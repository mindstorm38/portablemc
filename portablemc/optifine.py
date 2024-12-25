import os
import re
import zipfile
from io import BytesIO
import struct
from bs4 import BeautifulSoup
from typing import List, Dict, Optional
from hashlib import md5
import requests
from portablemc.standard import Version, Watcher, Context
import json
import shutil
from datetime import datetime
from pathlib import Path

def wget(url,file,watcher=Watcher):
    """
    wget-like function to download a file from a given url.
    will support an event handler to be called when the download progress is updated.
    Args:
        url (str): The URL to download.
        file (str): The name of the file to be saved locally.

    Returns:
        str: The name of the file saved locally.
    """
    local_filename = file
    r = requests.get(url)
    with open(local_filename,"wb") as f:
        for chunk in r.iter_content(chunk_size=512 * 1024):
            if chunk: # filter out keep-alive new chunks
                f.write(chunk)
    return local_filename

def get_official_version_list():
    manifest_url = "https://launchermeta.mojang.com/mc/game/version_manifest.json"
    list_versions = requests.get(manifest_url).json()
    versions = {}
    for i in list_versions["versions"]:
        versions[i["id"]] = {"type": i["type"], "url": i["url"]}
    return versions

def get_versions_list():
    """
    Récupère la liste des versions d'OptiFine.
    Renvoie une liste de noms de fichiers correspondant aux versions d'OptiFine.
    """
    url = 'https://optifine.net/downloads'
    response = requests.get(url)
    # Vérifie si la requête a réussi
    if response.status_code == 200:
        # Analyse le contenu HTML de la page avec BeautifulSoup
        soup = BeautifulSoup(response.text, 'html.parser')

        # Recherche les balises <a> contenant les liens de téléchargement
        download_links = soup.find_all('a', {'href': True}, text=lambda text: text and '(mirror)' in text.lower())

        # Parcours des liens de téléchargement et récupération des noms de fichiers
        versions = list()
        for link in download_links:
            versions.append(str(link).split('"')[1].split('=')[1])
        #get(versions[0])
        return versions

    else:
        print('Erreur lors de la requête HTTP')


def get_compatible_versions():
    """
    Returns a dictionary with the keys being the Minecraft version,
    and the values being a list of compatible Optifine versions.

    Returns:
        dict: A dictionary with the Minecraft version as the key and
              a list of compatible Optifine versions as the value.
    """
    versions_compat = dict()
    for of_version in get_versions_list():
        mc_version = of_version.replace("preview_OptiFine", "OptiFine").split('_')[1]
        if not mc_version in versions_compat.keys():
            versions_compat[mc_version] = list()
        versions_compat[mc_version].append(of_version.replace(".jar", ""))
    return versions_compat


def apply_xdelta_patch(source_data: bytes, patch_data: bytes) -> bytes:
    """
    Applies a binary patch to the source data and returns the patched file content.

    Args:
        source_data (bytes): The binary content of the file to be patched.
        patch_data (bytes): The binary content of the patch.

    Returns:
        bytes: The binary content of the patched file.
    """

    # Helper to copy data from the source
    def copy(offset: int, length: int):
        if offset + length > len(source_data):
            raise ValueError("Invalid copy range: exceeds source data size")
        return source_data[offset:offset + length]

    # Helper to append data from the patch
    def append(length: int, patch_stream: BytesIO):
        return patch_stream.read(length)

    # Open the patch stream
    patch_stream = BytesIO(patch_data)
    output_stream = BytesIO()

    # Verify the magic string for patch compatibility
    magic = patch_stream.read(5)
    if magic != b'\xd1\xff\xd1\xff\x04':
        raise ValueError("Invalid patch file: magic string not found")

    # Read commands from the patch file
    while patch_stream.tell() < len(patch_data):
        command = struct.unpack("B", patch_stream.read(1))[0]  # Read 1 byte as unsigned integer

        if command == 0:
            # No operation, skip
            continue
        elif 1 <= command <= 246:
            # Append `command` bytes of data from the patch
            output_stream.write(append(command, patch_stream))
        elif command == 247:
            # Append `unsigned short` bytes of data
            length = struct.unpack(">H", patch_stream.read(2))[0]
            output_stream.write(append(length, patch_stream))
        elif command == 248:
            # Append `unsigned int` bytes of data
            length = struct.unpack(">I", patch_stream.read(4))[0]
            output_stream.write(append(length, patch_stream))
        elif command == 249:
            # Copy `unsigned byte` bytes from `unsigned short` offset
            offset = struct.unpack(">H", patch_stream.read(2))[0]
            length = struct.unpack("B", patch_stream.read(1))[0]
            output_stream.write(copy(offset, length))
        elif command == 250:
            # Copy `unsigned short` bytes from `unsigned short` offset
            offset = struct.unpack(">H", patch_stream.read(2))[0]
            length = struct.unpack(">H", patch_stream.read(2))[0]
            output_stream.write(copy(offset, length))
        elif command == 251:
            # Copy `unsigned int` bytes from `unsigned short` offset
            offset = struct.unpack(">H", patch_stream.read(2))[0]
            length = struct.unpack(">I", patch_stream.read(4))[0]
            output_stream.write(copy(offset, length))
        elif command == 252:
            # Copy `unsigned byte` bytes from `unsigned int` offset
            offset = struct.unpack(">I", patch_stream.read(4))[0]
            length = struct.unpack("B", patch_stream.read(1))[0]
            output_stream.write(copy(offset, length))
        elif command == 253:
            # Copy `unsigned short` bytes from `unsigned int` offset
            offset = struct.unpack(">I", patch_stream.read(4))[0]
            length = struct.unpack(">H", patch_stream.read(2))[0]
            output_stream.write(copy(offset, length))
        elif command == 254:
            # Copy `unsigned int` bytes from `unsigned int` offset
            offset = struct.unpack(">I", patch_stream.read(4))[0]
            length = struct.unpack(">I", patch_stream.read(4))[0]
            output_stream.write(copy(offset, length))
        elif command == 255:
            # Copy `unsigned int` bytes from `long` offset
            offset = struct.unpack(">Q", patch_stream.read(8))[0]
            length = struct.unpack(">I", patch_stream.read(4))[0]
            output_stream.write(copy(offset, length))
        else:
            # Unrecognized command; append the command byte itself
            output_stream.write(bytes([command]))

    return output_stream.getvalue()


class Patcher:
    CONFIG_FILES = ["patch.cfg", "patch2.cfg", "patch3.cfg"]
    PREFIX_PATCH = "patch/"
    SUFFIX_DELTA = ".xdelta"
    SUFFIX_MD5 = ".md5"

    @staticmethod
    def process(base_file: str, diff_file: str, mod_file: str) -> None:
        """Process the patching operation by applying xdelta patches from the diff file to the base file and writing the
        result to the mod file.

        :param base_file: The base file which is being patched (Minecraft version original jar).
        :param diff_file: The jarfile containing xdelta patches which are applied to the base file (Optifine Installer).
        :param mod_file: The resulting file containing the patched content of the base file (final library stored
        in libraries folder).
        """
        with zipfile.ZipFile(diff_file, 'r') as diff_zip, \
                zipfile.ZipFile(base_file, 'r') as base_zip, \
                zipfile.ZipFile(mod_file, 'w') as mod_zip:

            # Read the configuration from the diff file
            cfg_map = Patcher.get_configuration_map(diff_zip)
            # Compile regex patterns from the config
            patterns = Patcher.get_configuration_patterns(cfg_map)

            # Iterate over all the files in the diff zip
            for diff_entry in diff_zip.infolist():
                name = diff_entry.filename
                # Read the contents of the file
                with diff_zip.open(diff_entry) as entry_stream:
                    entry_bytes = entry_stream.read()

                # If it's a xdelta diff file, apply the patch
                if name.startswith(Patcher.PREFIX_PATCH) and name.endswith(
                        Patcher.SUFFIX_DELTA):  # it's an xdelta diff file
                    name = name[len(Patcher.PREFIX_PATCH):-len(Patcher.SUFFIX_DELTA)]
                    print(name, "patched")
                    # Apply the patch
                    patched_bytes = Patcher.apply_patch(name, entry_bytes, patterns, cfg_map, base_zip)
                    # Check for an accompanying .md5 file and verify the hash
                    md5_name = f"{Patcher.PREFIX_PATCH}{name}{Patcher.SUFFIX_MD5}"
                    if md5_name in diff_zip.namelist():
                        with diff_zip.open(md5_name) as md5_stream:
                            expected_md5 = md5_stream.read().decode("ascii")
                        actual_md5 = md5(patched_bytes).hexdigest()
                        if expected_md5 != actual_md5:
                            raise ValueError(f"MD5 mismatch for {name}. Expected {expected_md5}, got {actual_md5}")

                    # Write the patched content to the output file
                    mod_zip.writestr(name, patched_bytes)

                # If it's not an xdelta diff file or an .md5 file, just copy it
                elif not (name.startswith(Patcher.PREFIX_PATCH) and name.endswith(Patcher.SUFFIX_MD5)):
                    mod_zip.writestr(name, entry_bytes)

    @staticmethod
    def apply_patch(name: str, diff_bytes: bytes, patterns: List[re.Pattern],
                    cfg_map: Dict[str, str], base_zip: zipfile.ZipFile) -> bytes:
        """
        Apply a patch to a base resource.

        Args:
            name (str): The name of the resource being patched.
            diff_bytes (bytes): The binary content of the patch.
            patterns (List[re.Pattern]): A list of compiled regex patterns.
            cfg_map (Dict[str, str]): A dictionary mapping patterns to base strings.
            base_zip (zipfile.ZipFile): The zip file containing the base resource.

        Returns:
            bytes: The patched binary content.

        Raises:
            ValueError: If no patch base is found for the given name.
        """
        # Remove any leading slash from the resource name
        name = name.lstrip("/")

        # Get the base resource name using the configuration map and patterns
        base_name = Patcher.get_patch_base(name, patterns, cfg_map)
        if not base_name:
            raise ValueError(f"No patch base found for {name}")

        # Read the base resource content from the zip file
        with base_zip.open(base_name) as base_stream:
            base_bytes = base_stream.read()

        # Apply the xdelta patch to the base resource
        patched_bytes = apply_xdelta_patch(base_bytes, diff_bytes)

        return patched_bytes

    @staticmethod
    def get_configuration_patterns(cfg_map: Dict[str, str]) -> List[re.Pattern]:
        """Generate regex patterns from the configuration map."""
        return [re.compile(key) for key in cfg_map.keys()]

    @staticmethod
    def get_configuration_map(zip_file: zipfile.ZipFile) -> Dict[str, str]:
        """Aggregate configuration from multiple config files."""
        cfg_map = {}
        for config_file in Patcher.CONFIG_FILES:
            if config_file in zip_file.namelist():
                with zip_file.open(config_file) as config_stream:
                    cfg_map.update(Patcher.parse_config_file(config_stream))
        return cfg_map

    @staticmethod
    def parse_config_file(config_stream) -> Dict[str, str]:
        """Parse a single config file into a dictionary."""
        cfg_map = {}
        for line in config_stream.read().decode("ascii").splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                parts = line.split("=", 1)
                if len(parts) != 2:
                    raise ValueError(f"Invalid config line: {line}")
                cfg_map[parts[0].strip()] = parts[1].strip()
        return cfg_map

    @staticmethod
    def get_patch_base(name: str, patterns: List[re.Pattern], cfg_map: Dict[str, str]) -> Optional[str]:
        """
        Find the base resource for a given patch.

        Args:
            name (str): The name of the resource being patched.
            patterns (List[re.Pattern]): A list of compiled regex patterns.
            cfg_map (Dict[str, str]): A dictionary mapping patterns to base strings.

        Returns:
            Optional[str]: The base resource name, or None if no match is found.
        """
        name = name.lstrip("/")  # Equivalent to Utils.removePrefix(name, "/")

        for pattern in patterns:
            matcher = pattern.match(name)
            if not matcher:
                continue

            base = cfg_map.get(pattern.pattern, None)  # Get the base from cfg_map
            if base is None:
                continue

            # Handle the "*" case
            if base.strip() == "*":
                return name

            # Replace placeholders in `base` with regex group matches
            for g in range(1, len(matcher.groups()) + 1):
                base = base.replace(f"${g}", matcher.group(g))

            return base

        return None

    @staticmethod
    def install_optifine_library(mc_version: str, target_version_name:Path | str, of_edition: str, mc_lib_dir: str, source_file: str, watcher:Watcher,context:Context):
        """
        Install the OptiFine library.

        Args:
            mc_version (str): The Minecraft version to target.
            of_edition (str): The OptiFine edition to install.
            mc_lib_dir (str): The directory to install the library to.
            source_file (str): The path to the OptiFine library JAR file.
            watcher (Watcher, optional): The progress watcher. Defaults to Watcher.
        """
        # source_file = "path_to_optifine.zip"  # Adjust path as needed
        destination_dir = os.path.join(mc_lib_dir, f"optifine/OptiFine/{mc_version}_{of_edition}")
        destination_file = os.path.join(destination_dir, f"OptiFine-{mc_version}_{of_edition}.jar")

        # Find the base Minecraft JAR file
        base_file = context.versions_dir / f"{mc_version}" / f"{mc_version}.jar"
        if not os.path.exists(base_file):
            base_file = context.versions_dir / target_version_name / f"{target_version_name}.jar"
            os.makedirs(os.path.dirname(base_file), exist_ok=True)
            if not os.path.exists(base_file):
                # Download the base Minecraft JAR file if it doesn't exist
                mc_version_url = get_version_list()[mc_version]["url"]
                mc_jar_url = requests.get(mc_version_url).json()["downloads"]["client"]["url"]
                wget(mc_jar_url, base_file, watcher)
            # raise FileNotFoundError(f"Base file not found: {base_file}")

        # Create the destination directory if it doesn't exist
        os.makedirs(os.path.dirname(destination_file), exist_ok=True)

        # Patch the base Minecraft JAR file using the OptiFine library
        Patcher.process(base_file, source_file, destination_file)


class Installer:
    @staticmethod
    def do_install(online_name, target_version_name=None, watcher=Watcher, context=None):
        context = context or Context()
        url = f'http://optifine.net/download?f={online_name}.jar'
        tmp = wget(url, online_name, watcher)
        optifine_zip_path = tmp
        """Perform the OptiFine installation."""

        # optifine_zip_path = Path(__file__).parent / "OptiFine.zip"
        of_ver = Installer.get_optifine_version(optifine_zip_path)
        Installer.dbg(f"OptiFine Version: {of_ver}")
        of_vers = of_ver.split("_")
        mc_ver = of_vers[1]
        Installer.dbg(f"Minecraft Version: {mc_ver}")
        of_ed = "_".join(of_vers[2:])
        Installer.dbg(f"OptiFine Edition: {of_ed}")

        mc_ver_of = f"{mc_ver}-OptiFine_{of_ed}" if target_version_name is None else target_version_name
        Installer.dbg(f"Minecraft_OptiFine Version: {mc_ver_of}")
        Installer.copy_minecraft_version(mc_ver, mc_ver_of, context.versions_dir)
        launchwrapper = Installer.get_launchwrapper_version(optifine_zip_path)

        # The launchwrapper library is installed by the launcher for versions using the official one
        if not launchwrapper == "legacy":
            Installer.install_launchwrapper_library(context.libraries_dir, launchwrapper, optifine_zip_path)

        # Patch and install the library like the standard installer would do
        Patcher.install_optifine_library(mc_ver, mc_ver_of, of_ed, str(context.libraries_dir),
                                         optifine_zip_path, watcher=watcher, context=context)

        Installer.update_json(context.versions_dir, mc_ver_of, context.libraries_dir, mc_ver, of_ed, launchwrapper)
        os.remove(tmp)

    @staticmethod
    def dbg(message):
        """Debugging utility function."""
        print(f"DEBUG: {message}")

    @staticmethod
    def tokenize(string, delimiter="_"):
        """Tokenize a string by a delimiter."""
        return string.split(delimiter)

    @staticmethod
    def format_date(date):
        """Format a date for JSON."""
        return date.strftime("%Y-%m-%dT%H:%M:%S%z")

    @staticmethod
    def format_date_ms(date):
        """Format a date with milliseconds."""
        return date.strftime("%Y-%m-%dT%H:%M:%S.%fZ")

    @staticmethod
    def get_optifine_version_from_bytes(data):
        """Extract OptiFine version from byte data."""
        pattern = "OptiFine_".encode("ascii")
        pos = data.find(pattern)
        assert pos >= 0, "OptiFine version not found"
        end_pos = pos
        while end_pos < len(data) and 32 <= data[end_pos] <= 122:
            end_pos += 1
        return data[pos:end_pos].decode("ascii")

    @staticmethod
    def get_optifine_version(file_path):
        """Retrieve the OptiFine version from a JAR file."""
        with zipfile.ZipFile(file_path, "r") as jar:
            files_to_check = [
                "net/optifine/Config.class",
                "notch/net/optifine/Config.class",
                "Config.class",
                "VersionThread.class"
            ]
            for file_name in files_to_check:
                try:
                    with jar.open(file_name) as file:
                        return Installer.get_optifine_version_from_bytes(file.read())
                except KeyError as e:
                    print(e)
                    continue
        raise FileNotFoundError("OptiFine version not found in the provided JAR")

    @staticmethod
    def copy_minecraft_version(mc_ver, mc_ver_of, dir_mc_ver, watcher:Watcher):
        """Copy a Minecraft version to prepare for OptiFine."""
        dir_ver_mc = dir_mc_ver / mc_ver
        # assert dir_ver_mc.exists(), f"Cannot find Minecraft version {mc_ver}"
        file_jar_mc = dir_ver_mc / f"{mc_ver}.jar"
        dir_ver_mc_of = dir_mc_ver / mc_ver_of
        dir_ver_mc_of.mkdir(parents=True, exist_ok=True)
        file_jar_mc_of = dir_ver_mc_of / f"{mc_ver_of}.jar"
        if not file_jar_mc.exists():
            print(f"Cannot find Minecraft version {mc_ver}, so downloading it...")
            mc_jar_url = Installer.take_official_json(mc_ver,dir_mc_ver)["downloads"]["client"]["url"]
            os.makedirs(os.path.dirname(file_jar_mc_of), exist_ok=True)
            wget(mc_jar_url, file_jar_mc_of, watcher)
        else:
            shutil.copy(file_jar_mc, file_jar_mc_of)

    @staticmethod
    def get_launchwrapper_version(of_jar_path):
        try:
            with zipfile.ZipFile(of_jar_path, "r") as jar:
                return jar.open("launchwrapper-of.txt").read().decode("utf-8").strip()
        except KeyError:
            return "legacy"

    @staticmethod
    def install_launchwrapper_library(dir_mc_lib, launchwrapper_version, of_jar_path):
        """Install the LaunchWrapper library."""
        file_name = f"launchwrapper-of-{launchwrapper_version}.jar"
        dir_dest = dir_mc_lib / f"optifine/launchwrapper-of/{launchwrapper_version}"
        file_dest = dir_dest / file_name
        dir_dest.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(of_jar_path, "r") as jar:
            with open(file_dest, "wb") as file:
                file.write(jar.open(file_name).read())

    @staticmethod
    def take_official_json(version,mc_vers_dir):
        json_default_path = os.path.join(mc_vers_dir, version, version + ".json")
        if os.path.exists(json_default_path):
            with open(json_default_path, "r") as f:
                return json.load(f)
        else:
            json_url = get_official_version_list()[version]["url"]
            return requests.get(json_url).json()

    @staticmethod
    def update_json(dir_mc_vers, mc_ver_of, dir_mc_lib, mc_ver, of_ed, launcherwrapper_version):
        """Update the JSON configuration."""
        dir_mc_vers_of = dir_mc_vers / mc_ver_of
        file_json = dir_mc_vers_of / f"{mc_ver_of}.json"
        # json_data = json.loads(read_file(file_json))
        data = Installer.take_official_json(mc_ver)
        if not launcherwrapper_version == "legacy":
            new_json = {
                "id": mc_ver_of,
                "inheritsFrom": mc_ver,
                "time": Installer.format_date(datetime.now()),
                "releaseTime": Installer.format_date(datetime.now()),
                "type": "release",
                "libraries": [
                    {"name": f"optifine:OptiFine:{mc_ver}_{of_ed}"},
                    {"name": f"optifine:launchwrapper-of:{launcherwrapper_version}"}
                ],
                "mainClass": "net.minecraft.launchwrapper.Launch"
            }
            if "minecraftArguments" in data:
                new_json["minecraftArguments"] = data["minecraftArguments"] + " --tweakClass optifine.OptiFineTweaker"
            else:
                new_json["minimumLauncherVersion"] = 21
                if mc_ver in ["1.7.2", "1.7.10", "1.8.0", "1.8.8", "1.12.2", "1.12.1", "1.12"]:
                    new_json["minecraftArguments"] = "--tweakClass optifine.OptiFineTweaker"
                else:
                    new_json["arguments"] = {
                        "game": ["--tweakClass", "optifine.OptiFineTweaker"]
                    }
        else:
            if mc_ver in ["1.7.2", "1.7.10", "1.8.0", "1.8.8"]:
                new_json = data
            else:
                new_json = {}
            new_json["libraries"] = []
            if not data["mainClass"].startswith("net.minecraft.launchwrapper."):
                new_json["mainClass"] = "net.minecraft.launchwrapper.Launch"
                new_json["libraries"].append({"name": "net.minecraft:launchwrapper:1.12", "size": 32999,
                                              "url": "https://repo.papermc.io/repository/maven-public/"})
            new_json["inheritsFrom"] = mc_ver
            new_json["id"] = f"Optifine {mc_ver}_{of_ed}"

            if "minecraftArguments" in data:
                new_json["minecraftArguments"] = data["minecraftArguments"] + " --tweakClass optifine.OptiFineTweaker"
            else:
                new_json["minimumLauncherVersion"] = 21
                if mc_ver in ["1.7.2", "1.7.10", "1.8.0", "1.8.8", "1.12.2", "1.12.1", "1.12"]:
                    new_json["minecraftArguments"] = "--tweakClass optifine.OptiFineTweaker"
                else:
                    new_json["arguments"] = {
                        "game": ["--tweakClass", "optifine.OptiFineTweaker"]
                    }
            new_json["libraries"].append({"name": "optifine:OptiFine:" + mc_ver + "_" + of_ed})
        with open(file_json, "w", encoding="utf-8") as f:
            json.dump(new_json, f, indent=4)

    @staticmethod
    def check_install(installed_version_name, context: Context | None = None):
        context = context or Context()
        of_ver_dir = context.versions_dir / installed_version_name
        if os.path.exists(of_ver_dir / f"{installed_version_name}.json") and os.path.exists(
                of_ver_dir / f"{installed_version_name}.jar"):
            with open(os.path.join(of_ver_dir, f"{installed_version_name}.json"), 'r') as f:
                to_check = json.load(f)
                for lib in to_check["libraries"]:
                    libpath = lib["name"].split(":")
                    libpath[0] = libpath[0].replace(".", "/")
                    libpath.append(libpath[-2] + "-" + libpath[-1] + ".jar")
                    if not os.path.exists(context.libraries_dir / os.path.join(*libpath)):
                        if "url" not in lib.keys():
                            print(os.path.join(*libpath), "not found")
                            return False
            return True
        else:
            return False


class OptifineVersion(Version):
    def __init__(self, version="latest", loader="latest", installas=None,context: Context | None = None):
        context = context or Context()
        if version == "latest":
            version = list(get_compatible_versions().keys())[0]
        if loader == "latest":
            loader = get_compatible_versions()[version][0]
        super().__init__(version, context=context)
        self.mcver = version
        self.loader = loader
        self.version = self.get_installed_default_name() if installas is None else installas

    def install(self, *, watcher: Optional[Watcher] = None):
        watcher = watcher or Watcher()
        if not Installer.check_install(self.version):  # If the optifine libs are not already installed
            Installer.do_install(self.loader, target_version_name=self.version, watcher=watcher, context=self.context)
        return super().install(watcher=watcher)

    def get_installed_default_name(self):
        spl_v = self.loader.split("_")
        ver_index_name = spl_v.index(self.mcver)
        optifine_ed = "_".join(spl_v[ver_index_name + 1:])
        return self.version + "-OptiFine_" + optifine_ed


if __name__ == "__main__":
    v = OptifineVersion("1.18.1", "OptiFine_1.18.1_HD_U_H6")
    env = v.install()
    env.run()
