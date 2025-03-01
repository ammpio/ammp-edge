#!/usr/bin/env python3
# coding=utf-8

import sys

from setuptools import find_packages, setup

package_name = "ammp_edge"
filename = package_name + ".py"


def get_version():
    import ast

    with open(filename) as input_file:
        for line in input_file:
            if line.startswith("__version__"):
                return ast.parse(line).body[0].value.s


def get_long_description():
    try:
        with open("README.md", "r") as f:
            return f.read()
    except IOError:
        return ""


# Base requirements
install_requires = [
    "python-dotenv",
    "pyModbusTCP",
    "minimalmodbus",
    "pyserial",
    "psutil",
    "requests",
    "xmltodict",
    "requests-unixsocket2",
    "flask",
    "paho-mqtt",
    "jsonata-python",
]

# Add easysnmp only on Linux
if sys.platform.startswith("linux"):
    install_requires.append("easysnmp")

setup(
    name=package_name,
    version=get_version(),
    author="AMMP Technologies",
    author_email="contact@ammp.io",
    description="Edge application for AMMP",
    url="https://www.ammp.io/",
    long_description=get_long_description(),
    packages=find_packages(),
    py_modules=[
        package_name,
        "edge_api",
        "constants",
    ],
    include_package_data=True,
    entry_points={
        "console_scripts": [
            "ammp_edge = ammp_edge:main",
            "wifi_ap_control = wifi_ap_control:main",
            "env_scan_svc = env_scan_svc:main",
        ]
    },
    python_requires="~=3.12",
    install_requires=install_requires,
)
