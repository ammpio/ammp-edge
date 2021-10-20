import logging
import requests
import os
import zipfile
import datetime

import requests_unixsocket

from node_mgmt import EnvScanner

logger = logging.getLogger(__name__)


def send_log(node):
    """ Upload system logs to S3 """

    # Package logs in zipped archive
    zipped_logs = __create_log_archive(node)
    if not zipped_logs:
        logger.warn('No log archive available. Exiting log upload.')
        return

    # Obtain S3 location for file upload
    upload_url = node.api.get_upload_url()
    if not upload_url:
        logger.warn('No upload URL available. Exiting log upload.')
        return

    # Send logs
    try:
        fh = open(zipped_logs, 'rb')
    except Exception:
        logger.exception('Cannot open log archive. Exiting log upload.')
        return

    try:
        r = requests.put(upload_url, data=fh.read(), headers={'Content-Disposition': os.path.basename(zipped_logs)})

        if r.status_code == 200:
            logger.info('Upload successful')
            logger.debug(r.text)
        else:
            logger.warn('Upload not successful')
            logger.info(r.text)
    except Exception:
        logger.exception('Exception while uploading log archive')
    finally:
        fh.close()

    # Delete temporary file
    try:
        os.remove(zipped_logs)
    except Exception:
        logger.warn('Cannot delete local log archive', exc_info=True)


def __create_log_archive(node):
    """ Find the systemd logs and create a zipped archive of them """

    # List of directories where to look for logs.
    # The function will stop and try to get logs from the first one that exists
    LOG_DIRS_TO_CHECK = ['/run/log/journal/']
    output_path = None

    for log_dir in LOG_DIRS_TO_CHECK:
        if os.path.isdir(log_dir):
            filename = f"logs_{node.node_id}_{datetime.datetime.utcnow().strftime('%Y%m%dT%H%M%SZ')}.zip"
            output_path = os.path.join(os.environ['SNAP_DATA'], filename)

            logger.info('Zipping logs in %s into %s' % (log_dir, output_path))
            __zip_directory(log_dir, output_path)

            break

    return output_path


def __zip_directory(dir_path, output_path):
    """Zip the contents of an entire folder (with that folder included
    in the archive). Empty subfolders will be included in the archive
    as well.
    """
    parent_folder = os.path.dirname(dir_path)
    # Retrieve the paths of the folder contents.
    contents = os.walk(dir_path)
    try:
        zip_file = zipfile.ZipFile(output_path, 'w', zipfile.ZIP_DEFLATED)
        for root, folders, files in contents:
            # Include all subfolders, including empty ones.
            for folder_name in folders:
                absolute_path = os.path.join(root, folder_name)
                relative_path = absolute_path.replace(parent_folder + '\\', '')
                logger.debug('Adding %s to archive.' % absolute_path)
                zip_file.write(absolute_path, relative_path)
            for file_name in files:
                absolute_path = os.path.join(root, file_name)
                relative_path = absolute_path.replace(parent_folder + '\\', '')
                logger.debug('Adding %s to archive.' % absolute_path)
                zip_file.write(absolute_path, relative_path)
        logger.debug('%s created successfully.' % output_path)
    except IOError:
        logger.exception('I/O Error')
        return None
    except OSError:
        logger.exception('OS Error')
        return None
    except zipfile.BadZipfile:
        logger.exception('"Bad ZIP file" error')
        return None
    finally:
        zip_file.close()
        return True


def snap_refresh(node):
    __snapd_socket_post({'action': 'refresh'})


def snap_switch_stable(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'stable'})


def snap_switch_candidate(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'candidate'})


def snap_switch_beta(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'beta'})


def snap_switch_edge(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'edge'})


def __snapd_socket_post(payload):
    try:
        with requests_unixsocket.Session() as s:
            res = s.post('http+unix://%2Frun%2Fsnapd.socket/v2/snaps/ammp-edge', json=payload)
        logger.info(f"Response from snapd API: Status {res.status_code} / {res.text}")
    except Exception:
        logger.exception('Exception while doing snapd API socket request')


def env_scan(node):
    logger.info('Starting environment scan')
    scanner = EnvScanner()
    scan_result = scanner.do_scan()
    logger.info('Completed environment scan. Submitting results to API.')
    node.api.post_env_scan(scan_result)
    logger.info('Completed environment scan. Submitting results to MQTT Broker.')
    if node.mqtt_client.publish(scan_result, topic='state'):
        logger.info(f"Scan results: Successfully pushed to mqtt")
    else:
        # For some reason the env_state wasn't pushed successfully,
        logger.warning(f"NEW CONFIG TRIGGER: Push failed")


def trigger_config_generation(node):
    logger.info('Starting environment scan')
    scanner = EnvScanner(generate_new_config=True)
    scan_result = scanner.do_scan()
    logger.info('Completed environment scan. Submitting results to MQTT Broker.')
    if node.mqtt_client.publish(scan_result, topic='state'):
        logger.info(f"NEW CONFIG TRIGGERED: Successfully pushed to mqtt")
    else:
        # For some reason the env_state wasn't pushed successfully,
        logger.warning(f"NEW CONFIG TRIGGER: Push failed")


def imt_sensor_address(node):
    # Note: unlike the other commands here, this one is normally triggered from the Web UI
    # Ideally this will be placed elsewhere in future, within a more systematic
    # action/command framework
    import serial
    from reader.modbusrtu_reader import Reader

    def readall(s):
        resp = b''
        while True:
            r = s.read()
            if len(r) > 0:
                resp += r
            else:
                break
        return resp

    ser = serial.Serial('/dev/ttyAMA0', baudrate=9600, timeout=5)
    result = {}

    logger.info("Changing address of IMT sensor from 1 to 2 (command 0x01460402630c")
    req = bytes.fromhex('01460402630c')
    ser.write(req)
    result['Address change response (expect 01460402630c'] = readall(ser).hex()

    logger.info("Restarting IMT sensor communications (command 0x010800010000b1cb")
    req = bytes.fromhex('010800010000b1cb')
    ser.write(req)
    result['Comms restart response (expect 010800010000b1cb'] = readall(ser).hex()

    logger.info("Testing sensor reading")
    try:
        with Reader('/dev/ttyAMA0', 2, baudrate=9600, debug=True) as r:
            result['Data read test from address 2'] = r.read(0, 1, 4)
    except Exception as e:
        result['Error'] = f"Exception: {e}"

    logger.info(f"Result: {result}")

    return result


def sys_reboot(node):
    os.system(
        'busctl call org.freedesktop.login1 /org/freedesktop/login1 org.freedesktop.login1.Manager Reboot b "false"')


def sys_start_snapd(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StartUnit ss "snapd.service" "replace"'
    )


def sys_stop_snapd(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StopUnit ss "snapd.service" "replace"'
    )


def sys_remount_rw(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StartUnit ss "remount-rw.service" "replace"'
    )


def sys_remount_ro(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StopUnit ss "remount-rw.service" "replace"'
    )
