#!/usr/bin/env python3
# coding=utf-8

from setuptools import setup


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
    description='Secure Telemetry, Remote Operation and Monitoring for Mini-Grids',
    url='https://www.ammp.io/',
    long_description=get_long_description(),
    packages=[
        'node_mgmt',
        'data_mgmt',
        'reader',
        'processor'
        ],
    py_modules=[
        package_name,
        'db_model'
        ],
    entry_points={
        'console_scripts': [
            'ammp_edge = ammp_edge:main'
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
        'xmltodict'
    ]
)
