import re # needed to parse downloads from optifine website and parse the patch config
import zipfile
# Libs for binary files processing
from io import BytesIO
import struct
from typing import List, Dict, Optional
from datetime import datetime
from hashlib import md5 # check if the file is corrupted after the patch
from .standard import Version, Watcher, Context, VersionHandle, VersionNotFoundError
from .http import http_request
from .download import DownloadEntry
from .util import LibrarySpecifier

def get_versions_list():
    """
    Récupère la liste des versions d'OptiFine.
    Renvoie une liste de noms de fichiers correspondant aux versions d'OptiFine.
    """
    url = 'https://optifine.net/downloads'
    response = http_request("GET", url)
    # Check if the request was successful
    if response.status == 200:
        # Download link search
        pattern = r'<a href="([^"]*)"[^>]*>([^<]*)</a>'
        # use a regex instead of BS4 to avoid dependencies
        matches = re.findall(pattern, response.data.decode('utf-8'))
        download_links = [match for match in matches if '(mirror)' in match[1].lower()]

        # take file names (used as loader version names) by browsing download links
        versions = [match[0].split('=')[1][:-4] for match in download_links]
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


class OptifineStartInstallEvent:
    __slots__ = tuple()


class OptifinePatchEvent:
    __slots__ = "location","total","done"
    def __init__(self, location: str,total: int,done: int):
        self.location = location
        self.total=total
        self.done=done


class OptifineEndInstallEvent:
    __slots__ = tuple()


class Patcher:
    CONFIG_FILES = ["patch.cfg", "patch2.cfg", "patch3.cfg"]
    PREFIX_PATCH = "patch/"
    SUFFIX_DELTA = ".xdelta"
    SUFFIX_MD5 = ".md5"

    @staticmethod
    def process(base_file: str, diff_file: str, mod_file: str,watcher: Watcher) -> None:
        """Process the patching operation by applying xdelta patches from the diff file to the base file and writing the
        result to the mod file.

        :param base_file: The base file which is being patched (Minecraft version original jar).
        :param diff_file: The jarfile containing xdelta patches which are applied to the base file (Optifine Installer).
        :param mod_file: The resulting file containing the patched content of the base file (final library stored
        :param watcher: The watcher to send events to
        in libraries folder).
        """
        with zipfile.ZipFile(diff_file, 'r') as diff_zip, \
                zipfile.ZipFile(base_file, 'r') as base_zip, \
                zipfile.ZipFile(mod_file, 'w') as mod_zip:

            # Read the configuration from the diff file
            cfg_map = Patcher.get_configuration_map(diff_zip)
            # Compile regex patterns from the config
            patterns = Patcher.get_configuration_patterns(cfg_map)
            diff_entries=diff_zip.infolist()
            # Iterate over all the files in the diff zip
            for diff_entry in diff_entries:
                name = diff_entry.filename
                watcher.handle(OptifinePatchEvent(name[:-len(Patcher.SUFFIX_DELTA)],len(diff_entries),diff_entries.index(diff_entry)))
                # Read the contents of the file
                with diff_zip.open(diff_entry) as entry_stream:
                    entry_bytes = entry_stream.read()

                # If it's a xdelta diff file, apply the patch
                if name.startswith(Patcher.PREFIX_PATCH) and name.endswith(
                        Patcher.SUFFIX_DELTA):  # it's an xdelta diff file
                    name = name[len(Patcher.PREFIX_PATCH):-len(Patcher.SUFFIX_DELTA)]

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


class OptifineVersion(Version):
    def __init__(self, version="latest", loader="latest",context: Optional[Context] | None = None):
        """Small override of the Version class to store the loader version"""
        context = context or Context()
        super().__init__(version, context=context)
        self.mcver = version # stores carefully the minecraft version
        self.loader = loader # and the loader

    def _resolve_version(self, watcher: Watcher) -> None:
        """
            Resolve the Optifine version based on the provided version and loader information.

            This method attempts to determine the most suitable version of Optifine based on the
            provided version and loader information. If the version is set to "latest", it will
            attempt to retrieve the latest compatible version. If the loader is set to "latest",
            it will attempt to retrieve the latest compatible loader for the determined version.

            Args:
                watcher (Watcher): The watcher instance to handle events during version resolution.

            Raises:
                VersionNotFoundError: If no compatible version is found.
        """
        available_of_vers = get_compatible_versions()
        if self.mcver == "latest" and self.loader == "latest":
            try:
                self.mcver = list(available_of_vers.keys())[0]
            except IndexError:
                raise VersionNotFoundError(self.mcver)
        elif self.mcver == "latest" and self.loader != "latest":
            self.mcver=self.parse_mcver_from_loader(self.loader)
        if self.loader == "latest":
            self.loader = available_of_vers[self.mcver][0]

        self.version = self.get_installed_default_name()
    @staticmethod
    def parse_mcver_from_loader(loader):
        """Takes the loader version (Install jar name)and returns the minecraft version"""
        spl_v = loader.split("_")
        spl_v.remove("preview") # in case it's a beta version
        return spl_v[1]
    def get_installed_default_name(self):
        """Get the default name the official Optifine installer would use for the installed version"""
        spl_v = self.loader.split("_")
        ver_index_name = spl_v.index(self.mcver)
        optifine_ed = "_".join(spl_v[ver_index_name + 1:])
        return self.version + "-OptiFine_" + optifine_ed

    def _load_version(self, version: VersionHandle, watcher: Watcher) -> bool:
        """Same as in ForgeVersion"""
        if version.id == self.version:
            return version.read_metadata_file()
        else:
            return super()._load_version(version, watcher)

    def _resolve_jar(self, watcher: Watcher) -> None:
        """Resolves the Optifine installer jar and the client jar"""
        super()._resolve_jar(watcher)
        if not (self.context.versions_dir / self.version / "ofInstaller.jar").exists():
            install_jar_url = f'http://optifine.net/download?f={self.loader}.jar'
            self._dl.add(DownloadEntry(install_jar_url, self.context.versions_dir / self.version / "ofInstaller.jar"))
        self._download(watcher) # download needed ressources (vanilla client jar, jvms, and optifine installer jar)
        self._finalize_optifine(watcher)

    def _fetch_version(self, version: VersionHandle, watcher: Watcher) -> None:
        """
        builds the optifine metadata if it doesn't exist
        """
        if version.id != self.version:
            return super()._fetch_version(version, watcher)
        if version.read_metadata_file():
            for key in self._of_base_json().keys():
                if key not in version.metadata:
                    version.metadata[key] = self._of_base_json()[key]
        else:
            version.metadata=self._of_base_json() # if the version metadata is not found, create it
        version.write_metadata_file()

    def _of_base_json(self):
        """
        Returns the base json for the optifine metadata file.
        This generates a very basic json to allow portablemc to download dependencies before installing optifine
        """
        return {
            "id": self.version,
            "inheritsFrom": self.mcver,
            "time": datetime.now().strftime("%Y-%m-%dT%H:%M:%S%z"),
            "releaseTime": datetime.now().strftime("%Y-%m-%dT%H:%M:%S%z"),
            "type": "release"
        }

    def _build_optifine_json(self, launchwrapper_version: str, parent_data: dict, ofedition: str) -> dict:
        """Update the JSON configuration."""
        new_json = self._of_base_json()
        # Update the JSON with the optifine library name and main class
        new_json.update( **{
            "libraries": [
                {"name": f"optifine:OptiFine:{self.mcver}_{ofedition}"}
            ],
            "mainClass": "net.minecraft.launchwrapper.Launch"
        })

        if launchwrapper_version == "net.minecraft:launchwrapper:1.12":
            new_json["libraries"].append({"name": "net.minecraft:launchwrapper:1.12", "size": 32999,
                                              "url": "https://repo.papermc.io/repository/maven-public/"})
        else:
            new_json["libraries"].append({"name": launchwrapper_version})
        if "minecraftArguments" in parent_data:
            new_json["minecraftArguments"] = parent_data["minecraftArguments"] + " --tweakClass optifine.OptiFineTweaker"
        else:
            if self.mcver in ["1.7.2", "1.7.10", "1.8.0", "1.8.8", "1.12.2", "1.12.1", "1.12"]:
                new_json["minecraftArguments"] = "--tweakClass optifine.OptiFineTweaker"
            else:
                new_json["minimumLauncherVersion"] = 21
                new_json["arguments"] = {
                    "game": ["--tweakClass", "optifine.OptiFineTweaker"]
                }
        return new_json
    def _finalize_optifine(self, watcher: Watcher) -> None:
        try:
            self._finalize_optifine_internal(watcher)
            self._resolve_metadata(watcher)
        except Exception as e:
            version = VersionHandle(self.version, self.context.versions_dir / self.version)
            version.metadata = self._of_base_json() # put back a basic metadata in the json to make sure it isn't broken
            version.write_metadata_file()
            jar_path=self.context.versions_dir / self.version / "ofInstaller.jar"
            jar_path.unlink()
            raise e
    def check_of_install(self, version: VersionHandle) -> bool:
        """
        Check if all needed ressources are properly installed to allow standard installation.
        """
        if not version.read_metadata_file():
            return False
        if "inheritsFrom" in version.metadata and version.metadata["inheritsFrom"] == self.mcver:
            if "libraries" in version.metadata:
                launchwrapperseemscorrect = False # This variable is used to check if the launchwrapper is correct
                for lib in version.metadata["libraries"]:
                    libpath=LibrarySpecifier.from_str(lib["name"])
                    if not (self.context.libraries_dir / libpath.file_path()).exists() and not "url" in lib.keys():
                        return False
                    if lib["name"].startswith("optifine:launchwrapper-of:") or "net.minecraft:launchwrapper:1.12" == lib["name"]:
                        launchwrapperseemscorrect = True
                if ("minecraftArguments" in version.metadata and
                    "--tweakClass optifine.OptiFineTweaker" not in version.metadata["minecraftArguments"]):
                    return False
                elif ("arguments" in version.metadata and
                    "game" in version.metadata["arguments"] and
                    "--tweakClass" not in version.metadata["arguments"]["game"] and
                    "optifine.OptiFineTweaker" not in version.metadata["arguments"]["game"]):
                    return False
                elif ("minecraftArguments" in version.metadata and
                    "--tweakClass optifine.OptiFineTweaker" in version.metadata["minecraftArguments"]):
                    return launchwrapperseemscorrect
                elif ("arguments" in version.metadata and
                    "game" in version.metadata["arguments"] and
                    "--tweakClass" in version.metadata["arguments"]["game"] and
                    "optifine.OptiFineTweaker" in version.metadata["arguments"]["game"]):
                    return launchwrapperseemscorrect
                else:
                    return False
        return False
    def _finalize_optifine_internal(self, watcher: Watcher) -> None:
        version = VersionHandle(self.version, self.context.versions_dir / self.version)

        if not self.check_of_install(version): # if the version is not installed, install it
            watcher.handle(OptifineStartInstallEvent())
            jar_path=self.context.versions_dir / self.version / "ofInstaller.jar"
            with zipfile.ZipFile(jar_path, "r") as jar:
                version=VersionHandle(self.version, self.context.versions_dir / self.version)
                try:
                    launchwrapper = f"optifine:launchwrapper-of:{jar.open("launchwrapper-of.txt").read().decode("utf-8").strip()}"
                    launchwrapper_version=jar.open("launchwrapper-of.txt").read().decode("utf-8").strip()
                except KeyError:
                    launchwrapper = "net.minecraft:launchwrapper:1.12"
                minecraft_ver = VersionHandle(self.mcver, self.context.versions_dir / self.mcver)
                if minecraft_ver.read_metadata_file():
                    parent_data = minecraft_ver.metadata
                else:
                    mcver_url=self.manifest.get_version(self.mcver)["url"]
                    res = http_request("GET", mcver_url, accept="application/json")
                    parent_data = res.json()
                    minecraft_ver.metadata = parent_data
                    minecraft_ver.write_metadata_file()
                ofver=self.get_optifine_version(jar)
                ofed="_".join(ofver.split("_")[2:]) # Get optifine edition for library path (EG: HD_U_J5)
                version.metadata = self._build_optifine_json(launchwrapper, parent_data, ofed)
                version.write_metadata_file()
                # patch minecraft version jar to build Optifine library
                of_lib_dir=self.context.libraries_dir/"optifine"/"OptiFine"/f"{self.mcver}_{ofed}"
                if not of_lib_dir.exists(): # makes the library directory
                    of_lib_dir.mkdir(parents=True,exist_ok=True)
                Patcher.process(self.context.versions_dir / self.version / f"{self.version}.jar",
                                self.context.versions_dir / self.version / "ofInstaller.jar",
                                self.context.libraries_dir/"optifine"/"OptiFine"/f"{self.mcver}_{ofed}"/f"OptiFine-{self.mcver}_{ofed}.jar",
                                watcher=watcher)
                if not launchwrapper=="net.minecraft:launchwrapper:1.12":
                    # install launchwrapper library if it is not default launchwrapper
                    file_name = f"launchwrapper-of-{launchwrapper_version}.jar"
                    dir_dest = self.context.libraries_dir / f"optifine/launchwrapper-of/{launchwrapper_version}"
                    file_dest = dir_dest / file_name
                    dir_dest.mkdir(parents=True, exist_ok=True)
                    with jar.open(file_name) as raw_launchwrapper:
                        with open(file_dest, "wb") as launchwrapper:
                            launchwrapper.write(raw_launchwrapper.read())

                watcher.handle(OptifineEndInstallEvent())
    @staticmethod
    def get_optifine_version(jar: zipfile.ZipFile) -> str:
        """Retrieve the OptiFine version from a optifine installer JAR file."""
        # there are several files in wich the version can be found, depending on the optifine version
        files_to_check = [
            "net/optifine/Config.class",
            "notch/net/optifine/Config.class",
            "Config.class",
            "VersionThread.class"
        ]
        for file_name in files_to_check:
            try:
                with jar.open(file_name) as file:
                    """Extract OptiFine version from byte data."""
                    data = file.read()
                    pattern = "OptiFine_".encode("ascii")
                    pos = data.find(pattern)
                    assert pos >= 0, "OptiFine version not found"
                    end_pos = pos
                    while end_pos < len(data) and 32 <= data[end_pos] <= 122:
                        end_pos += 1
                    return data[pos:end_pos].decode("ascii")
            except KeyError:
                continue
        #in case the version is not found
        raise FileNotFoundError("OptiFine version not found in the provided JAR")
