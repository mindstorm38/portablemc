"""
Development tool script for managing all modules at once using poetry.
Requiring Python 3.10
"""

import subprocess
import sys
import os

def for_each_module(args: list[str]):
    with os.scandir() as dirs:
        for entry in dirs:
            if entry.is_dir():
                print(f"{entry.name} > {' '.join(args)}")
                subprocess.call(args, cwd=entry.path)

if __name__ == '__main__':
    for_each_module(["poetry"] + sys.argv[1:])
    sys.exit(0)
