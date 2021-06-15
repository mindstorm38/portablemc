from portablemc_core import LAUNCHER_VERSION
from zipfile import ZipFile
from os import path
import os


PACKAGES = {
    "standard": ("portablemc.py",),
    "richer": ("portablemc.py", "addons/richer/*"),
    # "scripting": ("portablemc.py", "addons/richer/*", "addons/scripting/*"),
    "modloaders": ("portablemc.py", "addons/richer/*", "addons/modloader_fabric/*")
}


def main():

    print("Generating PortableMC distribution packages...")

    this_dir = path.dirname(__file__)
    packages_dir = path.join(this_dir, "packages")

    if not path.isdir(packages_dir):
        os.mkdir(packages_dir)

    for pkg_name, files in PACKAGES.items():
        print(f"Generating package '{pkg_name}'... ")
        pkg_file = path.join(packages_dir, f"portablemc_{LAUNCHER_VERSION}_{pkg_name}.zip")
        with ZipFile(pkg_file, "w") as zf:
            for file in files:
                is_dir = file.endswith("/*")
                if is_dir:
                    file = file[:-2]
                    for root, dirs, src_files in os.walk(path.join(this_dir, file)):
                        if not root.endswith("__pycache__"):
                            for src_file in src_files:
                                src_file = path.join(root, src_file)
                                dst_file = path.relpath(src_file, this_dir)
                                print(f"=> Writing {src_file}")
                                zf.write(src_file, dst_file)
                else:
                    src_file = path.join(this_dir, file)
                    print(f"=> Writing {src_file}")
                    zf.write(src_file, file)
        print(f"Done.")


if __name__ == '__main__':
    main()
