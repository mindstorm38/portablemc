"""
Development tool script for managing all modules at once using poetry.
"""

import subprocess
import sys
import os

def for_each_module(args, *, uninstall_first = False):

    os.chdir(os.path.dirname(__file__))

    def inner(name: str, path: str):

        print(f"{name} > {' '.join(args)}")

        if uninstall_first:
            print("=> uninstalling the package before installing it in dev mode")
            subprocess.call(["pip", "uninstall", "-y", name])
        
        subprocess.call(args, cwd=path)

    inner("portablemc", "core")

    with os.scandir() as dirs:
        for entry in dirs:
            if entry.is_dir() and entry.name != "core":
                inner(f"portablemc-{entry.name}", entry.path)

if __name__ == '__main__':
    is_install = sys.argv[1] == "install" if len(sys.argv) >= 2 else False
    for_each_module(["poetry"] + sys.argv[1:], uninstall_first=is_install)
    sys.exit(0)
