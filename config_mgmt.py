import logging
logger = logging.getLogger(__name__)

import os
import requests
import json, yaml

# Load base config from YAML file
with open("config.yml", 'r') as config_yml:
    config = yaml.load(config_yml)


def generate_node_id():
    # Get ID (ideally hardware MAC address) that is used to identify logger when pushing data

    # First try to get the address of the primary Ethernet adapter
    try:
        import netifaces as nif

        ifn_wanted = ['eth0', 'en0', 'eth1', 'en1', 'em0', 'em1', 'wlan0', 'wlan1']
        ifn_available = nif.interfaces()

        ifn = [i for i in ifn_wanted if i in ifn_available][0]

        if_mac = nif.ifaddresses(ifn)[nif.AF_LINK][0]['addr']
        node_id = if_mac.replace(':','')

    except Exception as ex:
        logger.exception('Cannot find primary network interface MAC; trying UUID MAC')

        # If that doesn't work, try doing it via the UUID method
        try:
            from uuid import getnode
            
            uuid_node = getnode()
            node_id = "{0:0{1}x}".format(uuid_node, 12)

        except Exception as ex:
            logger.exception('Cannot get MAC via UUID method; generating random node ID')

            # If that also doesn't work, generate a random 12-character hex string
            import random
            node_id = '%012x' % random.randrange(16**12)

    return node_id

def get_config():

    from db_model import NodeConfig

    try:
        nodeconfig = NodeConfig.get()

        node_id = nodeconfig.node_id
        access_key = nodeconfig.access_key

    except NodeConfig.DoesNotExist:
        logger.info('No node configuration found in internal database. Attempting node activation')
        node_id = generate_node_id()
        logger.info('Generated node ID %s' % node_id)

        access_key = do_node_activation(node_id)
        if not access_key:
            logger.error('Unable to obtain access key')
            return None

        nodeconfig = NodeConfig.create(node_id=node_id, access_key=access_key)
        nodeconfig.save()

    logger.info('Node ID: %s', node_id)

    if nodeconfig.config and not config['config_reset']:
        # Configuration is available in DB; use this
        logger.info('Using stored configuration from database')
        remote_config = nodeconfig.config
    else:
        # Need to request configuration from API
        logger.info('No stored configuration in database, or configuration reset requested')
        remote_config = get_config_from_api(node_id, access_key)

        nodeconfig.config = remote_config
        nodeconfig.save()

    return node_id, remote_config, access_key



def do_node_activation(node_id):

    # Initiate activation
    logger.info('Requesting activation for node %s' % node_id)
    
    try:
        r1 = requests.get('https://%s/api/%s/nodes/%s/activate' % (config['remote']['host'], config['remote']['apiver'], node_id))
        rtn = json.loads(r1.text)

        if r1.status_code == 200:
            access_key = rtn['access_key']
            logger.info('Obtained API key')
            if rtn:
                logger.debug('API response: %s' % rtn)
        else:
            logger.error('Error %d requesting activation from API' % r1.status_code)
            if rtn:
                logger.debug('API response: %s' % rtn)
            return None
    except Exception as ex:
        logger.exception('Exception raised while requesting activation from API')
        return None

    # Confirm activation
    logger.info('Confirming activation for node %s' % node_id)

    try:
        r2 = requests.post('https://%s/api/%s/nodes/%s/activate' % (config['remote']['host'], config['remote']['apiver'], node_id),
            headers={'Authorization': access_key})
        rtn = json.loads(r2.text)

        if r2.status_code == 200:
            logger.info('Confirmed activation')
            if rtn:
                logger.debug('API response: %s' % rtn)
        else:
            logger.error('Error %d confirming activation with API' % r2.status_code)
            if rtn:
                logger.debug('API response: %s' % rtn)
            return None
    except Exception as ex:
        logger.exception('Exception raised while confirming activation with API')
        return None

    return access_key


def get_config_from_api(node_id, access_key):

    logger.info('Obtaining configuration for node %s from API' % node_id)

    try:
        r = requests.get('https://%s/api/%s/nodes/%s' % (config['remote']['host'], config['remote']['apiver'], node_id),
            headers={'Authorization': access_key})
        rtn = json.loads(r.text)

        if r.status_code == 200:
            if 'message' in rtn:
                logger.debug('API message: %s' % rtn['message'])

            if rtn.get('config'):
                logger.info('Obtained config from API')
                logger.debug('Config payload: %s' % rtn['config'])
                return rtn['config']
            else:
                logger.error('API call successful but response did not include a config payload')
                return None
        else:
            logger.error('Error %d requesting configuration from API' % r.status_code)
            if rtn:
                logger.debug('API response: %s' % rtn)
            return None
    except Exception as ex:
        # Will need to add error-catch + retry criteria for when unit is offline

        logger.exception('Exception raised while requesting configuration from API')
        return None


def get_drivers():

    drivers = {}

    drvpath = os.path.join(os.getenv('SNAP_COMMON', './'), 'drivers')

    driver_files = [pos_json for pos_json in os.listdir(drvpath) if pos_json.endswith('.json')]
    for drv in driver_files:
        with open(os.path.join(drvpath, drv)) as driver_file:
            drivers[os.path.splitext(drv)[0]] = json.load(driver_file)
            logger.info('Loaded driver %s' % drv)

    return drivers


node_id, remote_config, access_key = get_config()

config.update(remote_config)

drivers = get_drivers()
