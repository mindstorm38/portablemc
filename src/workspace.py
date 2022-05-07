"""
Development tool script for managing all modules at once using poetry.
Requiring Python 3.10
"""

import subprocess
import sys
import os

def for_each_module(all_args: list[str], core_args: list[str] | None = None):
    if core_args is None:
        core_args = all_args
    with os.scandir() as dirs:
        for entry in dirs:
            if entry.is_dir():
                args = core_args if entry.name == "core" else all_args
                print(f"{entry.name} $ {' '.join(args)}")
                subprocess.call(args, cwd=entry.path)

if __name__ == '__main__':

    if len(sys.argv) != 2 or sys.argv[1] not in ("install", "update"):
        print(f"usage: {sys.argv[0]} <install|update>")
        sys.exit(1)

    cmd = sys.argv[1]

    if cmd == "install":
        for_each_module(["poetry", "install"])
    elif cmd == "update":
        for_each_module(["poetry", "update"])

    sys.exit(0)
