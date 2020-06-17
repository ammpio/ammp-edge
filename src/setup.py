#!/usr/bin/env python3
# coding=utf-8

from setuptools import setup, find_packages


package_name = 'ammp_edge'
filename = package_name + '.py'


def get_version():
    import ast

    with open(filename) as input_file:
        for line in input_file:
            if line.startswith('__version__'):
                return ast.parse(line).body[0].value.s


def get_long_description():
    try:
        with open('README.md', 'r') as f:
            return f.read()
    except IOError:
        return ''


setup(
    name=package_name,
    version=get_version(),
    author='AMMP Technologies',
    author_email='contact@ammp.io',
    description='Edge application for AMMP',
    url='https://www.ammp.io/',
    long_description=get_long_description(),
    packages=find_packages(),
    py_modules=[
        package_name,
        'db_model',
        'kvstore',
        'edge_api'
        ],
    include_package_data=True,
    entry_points={
        'console_scripts': [
            'ammp_edge = ammp_edge:main',
            'wifi_ap_control = wifi_ap_control:main',
            'env_scan_svc = env_scan_svc:main'
        ]
    },
    install_requires=[
        'pyyaml',
        'python-dotenv',
        'pyModbusTCP',
        'minimalmodbus',
        'pyserial',
        'easysnmp',
        'arrow',
        'peewee',
        'psutil',
        'requests',
        'influxdb',
        'xmltodict',
        'requests-unixsocket',
        'flask',
        'redis',
        'paho-mqtt'
    ]
)
