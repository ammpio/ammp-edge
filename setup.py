#!/usr/bin/env python
# coding=utf-8

from setuptools import setup


package_name = 'stromm'
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
    author='Svet Bajlekov',
    author_email='s.bajlekov@gmail.com',
    description='Secure Telemetry, Remote Operation and Monitoring for Mini-Grids',
    url='https://www.ammp.io',
    long_description=get_long_description(),
    packages=[
        'node_mgmt',
        'data_mgmt',
        'reader'
        ],
    py_modules=[
        package_name,
        'db_model'
        ],
    entry_points={
        'console_scripts': [
            'stromm = stromm:main'
        ]
    },
    install_requires=[
        'pyyaml',
        'pyModbusTCP',
        'minimalmodbus',
        'pyserial',
        'easysnmp',
        'arrow',
        'influxdb',
        'peewee',
        'systemd'
    ]
)