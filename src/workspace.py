"""
Development tool script for managing all modules at once using poetry.
"""

import subprocess
import sys
import os

def iter_module():
    os.chdir(os.path.dirname(__file__))
    yield "portablemc", "core"
    with os.scandir() as dirs:
        for entry in dirs:
            if entry.is_dir() and entry.name != "core":
                yield f"portablemc-{entry.name}", entry.name

def for_each_module(args):
    for name, path in iter_module():
        print(f"{name} > {' '.join(args)}")
        subprocess.call(args, cwd=path)

if __name__ == '__main__':

    need_uninstall = sys.argv[1] == "install" if len(sys.argv) >= 2 else False

    if need_uninstall:
        print("Uninstalling previously installed modules...")
        subprocess.call(["pip", "uninstall", "-y", *map(lambda t: t[0], iter_module())])
    
    for_each_module(["poetry"] + sys.argv[1:])
    sys.exit(0)
