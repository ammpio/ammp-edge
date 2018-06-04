import logging
logger = logging.getLogger(__name__)

import requests
import json
import sys, os
import zipfile
import datetime

def send_log(node):
    """ Upload system logs to S3 """

    # Package logs in zipped archive
    zipped_logs = _create_log_archive(node)
    if not zipped_logs:
        logger.warn('No log archive available. Exiting log upload.')
        return

    # Obtain S3 location for file upload
    upload_url = _get_upload_url(node)
    if not upload_url:
        logger.warn('No upload URL available. Exiting log upload.')
        return

    # Send logs
    try:
        fh = open(zipped_logs, 'rb')
    except:
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
    except:
        logger.exception('Exception while uploading log archive')
    finally:
	    fh.close()    

    # Delete temporary file
    try:
        os.remove(zipped_logs)
    except:
        logger.warn('Cannot delete local log archive', exc_info=True)


def _create_log_archive(node):
    """ Find the systemd logs and create a zipped archive of them """

    # List of directories where to look for logs. The function will stop and try to get logs from the first one that exists
    LOG_DIRS_TO_CHECK = ['/run/log/journal/', '/tmp/test']
    output_path = None

    for log_dir in LOG_DIRS_TO_CHECK:
        if os.path.isdir(log_dir):
            filename = 'logs' + '_' + node.node_id + '_' + datetime.datetime.utcnow().strftime('%Y%m%dT%H%M%SZ') + '.zip'
            output_path = os.path.join(os.environ['SNAP_DATA'], filename)

            logger.info('Zipping logs in %s into %s' % (log_dir, output_path))
            _zip_directory(log_dir, output_path)

            break

    return output_path


def _zip_directory(dir_path, output_path):
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
        logger.exception('Bad ZIP file error')
        return None
    finally:
        zip_file.close()
        return True

def _get_upload_url(node):

    logger.debug('Obtaining upload URL from API')

    try:
        r = requests.get('https://%s/api/%s/nodes/%s/upload_url' % (node.remote['host'], node.remote['apiver'], node.node_id),
            headers={'Authorization': node.access_key})
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
    except:
        logger.exception('Exception raised while requesting upload URL from API')
        return None
