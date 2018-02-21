import logging
logger = logging.getLogger(__name__)

class Reader(object):
    def __init__(self):
        pass

    def __enter__(self):
        return self

    def __exit__(self, type, value, traceback):
        pass

    def read(self, module, method, args, keypath, **kwargs):
        """
        module defaults to "psutil" but can be "os" (or anything else that's useful)
        method can be something like "disk_usage"
        args are what's passed to the method (e.g. '/')
        keypath is a list of keys or indices (e.g. [0, 1]) that are used to traverse the returned result in order to retrieve the desired value
        """

        mod = __import__(module)

        val = getattr(mod, method)(**args)

        for key in keypath:
            val = val[key]

        return val
