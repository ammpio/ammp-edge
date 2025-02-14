import logging
import os

from do_network_scan import main
from dotenv import load_dotenv

# Set up logging
logging.basicConfig(format="%(name)s [%(levelname)s] %(message)s", level="INFO")
logger = logging.getLogger(__name__)

# Load additional environment variables from env file (set by snap configuration)
dotenv_path = os.path.join(os.environ.get("SNAP_COMMON", "."), ".env")
load_dotenv(dotenv_path)

if os.environ.get("LOG_LEVEL"):
    try:
        logging.getLogger().setLevel(os.environ["LOG_LEVEL"])
    except Exception:
        logger.warn(f"Failed to set log level to {os.environ['LOG_LEVEL']}", exc_info=True)

__all__ = ["main"]
