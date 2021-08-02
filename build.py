# encoding: utf8

# Copyright (C) 2021  Théo Rozier
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

"""
Further build utilities for PortableMC, actually used only for building the single-file portable version.
"""

from io import TextIOWrapper
from zipfile import ZipFile
from os import path
import toml
import sys
import os


dist_dir = path.join(path.dirname(__file__), "dist")
source_dir = path.join(path.dirname(__file__), "portablemc")


def main():

    if len(sys.argv) != 2:
        print_usage()
        sys.exit(1)

    subcommand = sys.argv[1]

    if subcommand == "portable":
        build_portable()
    else:
        print_usage()
        sys.exit(1)

    sys.exit(0)


def print_usage():
    print(f"usage: {sys.argv[0]} {{portable}}")


def get_version() -> str:
    data = toml.load(path.join(path.dirname(__file__), "pyproject.toml"))
    return data["tool"]["poetry"]["version"]


def build_portable():

    print("Building single-script version of PortableMC from raw source files...")

    version = get_version()
    archive_file = path.join(dist_dir, f"portablemc-single-{version}.zip")

    os.makedirs(dist_dir, exist_ok=True)

    with ZipFile(archive_file, "w") as zf:
        with zf.open("portablemc.py", "w") as wfb:
            with TextIOWrapper(wfb, encoding="utf-8") as wf:

                wf.write("#!/usr/bin/env python\n\n")

                with open(path.join(source_dir, "__init__.py"), "rt", encoding="utf-8") as rf:
                    wf.write(rf.read())

                wf.write("\n\nif __name__ == \"__main__\":\n\n")

                with open(path.join(source_dir, "cli.py"), "rt", encoding="utf-8") as rf:
                    first_import = False
                    for line in rf.readlines():
                        if not first_import:
                            if line.startswith("from"):
                                first_import = True
                        if first_import and not line.startswith("from . import *"):
                            wf.write("    ")
                            wf.write(line)

                wf.write("\n\n    main()\n")

    print(f"Built at {archive_file}")


if __name__ == '__main__':
    main()
