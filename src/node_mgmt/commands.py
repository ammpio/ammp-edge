import logging
import requests
import json
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
    upload_url = __get_upload_url(node)
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
        r = requests.put(
            upload_url,
            data=fh.read(),
            headers={'Content-Disposition': os.path.basename(zipped_logs)}
                )

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


def __get_upload_url(node):

    logger.debug('Obtaining upload URL from API')

    try:
        r = requests.get(
            f"https://{node.remote_api['host']}/api/{node.remote_api['apiver']}/nodes/{node.node_id}/upload_url",
            headers={'Authorization': node.access_key}
            )
        rtn = json.loads(r.text)

        if r.status_code == 200:
            if 'message' in rtn:
                logger.debug('API message: %s' % rtn['message'])

            if rtn.get('upload_url'):
                logger.info('Obtained upload URL from API: %s' % rtn['upload_url'])
                return rtn['upload_url']
            else:
                logger.error('API call successful but response did not include a URL payload')
                return None
        else:
            logger.error('Error %d requesting URL from API' % r.status_code)
            if rtn:
                logger.debug('API response: %s' % rtn)
            return None
    except Exception:
        logger.exception('Exception raised while requesting upload URL from API')
        return None


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

    try:
        r = requests.post(
            f"https://{node.remote_api['host']}/api/{node.remote_api['apiver']}/nodes/{node.node_id}/env_scan",
            json=scan_result,
            headers={'Authorization': node.access_key},
            timeout=node.config.get('push_timeout') or 120)
    except requests.exceptions.ConnectionError:
        logger.warning('Connection error while trying to submit environment scan to API.')
        return
    except requests.exceptions.ConnectionError:
        logger.warning('Timeout error while trying to submit environment scan to API.')
        return
    except Exception:
        logger.warning('Exception while trying to to submit environment scan to API.', exc_info=True)
        return

    if r.status_code != 200:
        logger.warning('Error code %d while trying to to submit environment scan to API.' % (r.status_code))
        return


def imt_sensor_address(node):
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

    # Change address from 1 to 2
    req = bytes.fromhex('01460402630c')
    ser.write(req)
    result['Address change response (expect 01460402630c'] = readall(ser).hex()

    # Restart communications
    req = bytes.fromhex('010800010000b1cb')
    ser.write(req)
    result['Comms restart response (expect 010800010000b1cb'] = readall(ser).hex()

    # Test irradiation sensor
    try:
        with Reader('/dev/ttyAMA0', 2, baudrate=9600, debug=True) as r:
            result['Data read test from address 2'] = r.read(0, 1, 4)
    except Exception as e:
        result['Error'] = f"Exception: {e}"

    return result
