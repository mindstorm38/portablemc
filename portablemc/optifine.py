import re # needed to parse downloads from optifine website and parse the patch config
import zipfile
# Libs for binary files processing
from io import BytesIO
import struct
from typing import List, Dict, Optional
from datetime import datetime
from hashlib import md5 ,sha1# check if the file is corrupted after the patch
from .standard import Version, Watcher, Context, VersionHandle, VersionNotFoundError
from .http import http_request, HttpError
from .download import DownloadEntry
from .util import LibrarySpecifier
from pathlib import Path
INSTALLER_FILENAME="of_installer.jar"

class OptifineEntryObject:
    def __init__(self, filename: str, edition: str, mc_version: str, preview: bool =False, date: Optional[str] | None =None, forge: Optional[str] | None =None):
        self.edition=edition
        self.mc_version=mc_version
        self.preview=preview
        self.date=date
        self.forge=forge
        self.filename=filename

    @classmethod
    def from_dir_name(cls, name: str):
        n = name.split("-OptiFine_")
        mcver = n[0]
        edition = n[1]
        filename = ""
        preview = False
        if re.match(r"pre\d",edition.split("_")[-1]):
            filename += "preview_"
            preview = True
        filename += f"OptiFine_{mcver}_{edition}"
        return cls(filename,edition,mcver,preview)

    @classmethod
    def from_filename(cls, filename: str):
        split_filename = filename.replace(".jar", "").split("_")
        preview = (split_filename[0] == "preview")
        if preview:
            split_filename.pop(0)  # remove preview prefix in order to parse the rest of the filename
        edition = "_".join(split_filename[2:])
        mc_version = split_filename[1]
        preview = preview
        return cls(filename, edition, mc_version, preview)

    @classmethod
    def from_tuple(cls, match):
        entry=cls.from_filename(match[0].split("=")[1])
        entry.date = match[2]
        entry.forge = match[1]
        return entry

    def __repr__(self):
        return f"<{self.mc_version}-OptiFine_{self.edition}>"
    
def get_offline_versions(versions_dir:Optional[Path] | None=None):
    """
    When there is no Internet, this list available optifine versions in the current context
    """
    versions=[OptifineEntryObject.from_dir_name(dirpath.name) for dirpath in versions_dir.iterdir() if re.match(r"^\d+\.\d+\.\d+-OptiFine_HD_U_[A-Z][0-9](_pre\d)?$",dirpath.name)]
    out_dict = {}
    for v in versions:
        if not v.mc_version in out_dict:
            out_dict[v.mc_version] = []
        out_dict[v.mc_version].append(v)
    return out_dict
        
def get_versions_list(work_dir:Optional[Path] | None=None):
    """
    Récupère la liste des versions d'OptiFine.
    Renvoie une liste de noms de fichiers correspondant aux versions d'OptiFine.
    """
    url = 'https://optifine.net/downloads'
    try:
        response = http_request("GET", url, accept="text/html")
    except HttpError:
        response = None
    # Check if the request was successful
    # Download link search
    pattern = r"<tr class='downloadLine [^']*'>\s*<td class='colFile'>[^<]+</td>\s*<td class='colDownload'><a href=\"[^\"]+\">Download</a></td>\s*<td class='colMirror'><a href=\"([^\"]+)\">\(Mirror\)</a></td>\s*<td class='colChangelog'><a href='[^']+'>Changelog</a></td>\s*<td class='colForge'>([^<>]+)</td>\s*<td class='colDate'>([0-9.]+)</td>\s*</tr>"
    if response and response.status == 200:
        data = response.data.decode('utf-8')
        if work_dir:
            with open(work_dir/"net.optifine.downloads.html","w") as f:
                f.write(data) # backup data
    else: # unable to fetch the web ressource
        if work_dir:
            try:
                with open(work_dir/"net.optifine.downloads.html","rb") as f:
                    data = f.read().decode("utf-8")
            except Exception:
                raise VersionNotFoundError("Unable to fetch optifine version list")
        else : raise VersionNotFoundError("Unable to fetch optifine version list")
    # use a regex instead of BS4 to avoid dependencies
    matches = re.findall(pattern, data)

    # take file names (used as loader version names) by browsing download links
    versions = [OptifineEntryObject.from_tuple(match) for match in matches]
    return versions

def get_compatible_versions(work_dir:Optional[Path] | None=None):
    """
    Returns a dictionary with the keys being the Minecraft version,
    and the values being a list of compatible Optifine versions.

    Returns:
        dict: A dictionary with the Minecraft version as the key and
              a list of compatible Optifine versions as the value.
    """
    versions_compat = dict()
    for of_version in get_versions_list(work_dir):
        if not of_version.mc_version in versions_compat.keys():
            versions_compat[of_version.mc_version] = list()
        versions_compat[of_version.mc_version].append(of_version)
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
        self.total = total
        self.done = done


class OptifineEndInstallEvent:
    __slots__ = "version",
    def __init__(self,version: str):
        self.version = version


class Patcher:
    CONFIG_FILES = ["patch.cfg", "patch2.cfg", "patch3.cfg"]
    PREFIX_PATCH = "patch/"
    SUFFIX_DELTA = ".xdelta"
    SUFFIX_MD5 = ".md5"

    @staticmethod
    def process(base_file: Path | str, diff_file: Path | str, mod_file: Path | str, watcher: Watcher) -> None:
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

                # If it's not a xdelta diff file or an .md5 file, just copy it
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
    def __init__(self, version: str = None, *args, context: Optional[Context] = None):
        """
            This class is a basic implementation of a version class for Optifine.
            it takes two arguments: version and context.
            the version can be a string of one of these formats:
            - "latest"
            - "recommended"
            - "<minecraft version>"
            - "<minecraft version>-<recommended|latest>"
            - "<minecraft version>:<optifine edition>"
            - "<minecraft version>-OptiFine_<optifine edition>"
            - optifine filename without the .jar suffix EG preview_Optifine_1.19.2-HD_U_J6.jar
            The optifine edition is a string looking like that:
            - HD_U_J6
            _ HD_U_I6_pre3
        """
        version = version or "recommended"
        super().__init__(version, context = context)

    def _resolve_version(self, watcher: Watcher) -> None:
        """
        Resolve the Optifine version based on the provided version and loader
        information.

        This method attempts to determine the most suitable version of Optifine
        based on the provided version and loader information. If the version is
        set to "latest", it will attempt to retrieve the latest compatible
        version. If the loader is set to "latest", it will attempt to retrieve
        the latest compatible loader for the determined version.

        Args:
            watcher (Watcher): The watcher instance to handle events during
                version resolution.

        Raises:
            VersionNotFoundError: If no compatible version is found.
        """
        try:
            available_of_vers = get_compatible_versions(self.context.work_dir)
        except VersionNotFoundError:
            available_of_vers = get_offline_versions(self.context.versions_dir)

        try:
            if self.version == "recommended":  # when the version is "recommended"
                mcver = [
                    v for v in available_of_vers.keys()
                    if any(not of.preview for of in available_of_vers[v])
                ][0]  # remove preview versions
                loader = [
                    of.edition for of in available_of_vers[mcver] if not of.preview
                ][0]

            elif self.version == "latest":  # when the version is "latest"
                mcver = list(available_of_vers.keys())[0]
                loader = available_of_vers[mcver][0].edition

            elif self.version == "latest:recommended":  # when the version is
                # latest recommended
                mcver = list(available_of_vers.keys())[0]
                loaders = (
                    [of.edition for of in available_of_vers[mcver] if not of.preview]
                    or [of.edition for of in available_of_vers[mcver]]
                )
                loader = loaders[0]

            elif re.match(
                r"^\d+\.\d+(\.\d+)?$", string = self.version
            ):  # in case only the minecraft version is provided
                mcver = self.version
                loaders = (
                    [of.edition for of in available_of_vers[mcver] if not of.preview]
                    or [of.edition for of in available_of_vers[mcver]]
                )
                loader = loaders[0]

            elif re.match(
                pattern = r"^\d+\.\d+(\.\d+)?:[a-zA-Z0-9_]+$", string = self.version
            ):  # when the version is of the form {minecraft version}:{optifine edition}
                mcver, loader = self.version.split(":")
                if loader == "latest":
                    loader = available_of_vers[mcver][0].edition
                elif loader == "recommended":  # takes the latest non-preview
                    # loader, if available, or fallback to a preview
                    loaders = (
                        [of.edition for of in available_of_vers[mcver] if not of.preview]
                        or [of.edition for of in available_of_vers[mcver]]
                    )
                    loader = loaders[0]

            elif re.match(
                pattern = r"^\d+\.\d+\.\d+-OptiFine_HD_U_[A-Z][0-9](_pre\d)?$",
                string = self.version,
            ):  # the version is already in the correct format
                mcver, loader = self.version.split("-OptiFine_")

            elif re.match(
                pattern = r"^(preview_)?OptiFine_\d+\.\d+\.\d+_[A-Z0-9_]+",
                string = self.version,
            ):  # in this case, the user directly chosen a filename on the
                # optifine website
                split_filename = self.version.split("_")
                if split_filename[0] == "preview":
                    split_filename.pop(0)
                mcver = split_filename[1]
                loader = "_".join(split_filename[2:])

            else:
                # no regex matched
                raise VersionNotFoundError(self.version)

        except Exception:
            raise VersionNotFoundError(self.version)
        if (not mcver in available_of_vers.keys() or
                not any(loader == entry.edition for entry in available_of_vers[mcver])):
            raise VersionNotFoundError(self.version)
        self.version = mcver + "-OptiFine_" + loader

    def mcver(self):
        return self.version.split("-OptiFine_")[0]

    def loader(self):
        return self.version.split("-OptiFine_")[1]

    def dl_url(self):
        mcver, edition = self.version.split("-OptiFine_")
        filename = ""
        if re.match(r"pre\d", edition.split("_")[-1]):
            filename += "preview_"
        filename += f"OptiFine_{mcver}_{edition}"
        return f"http://optifine.net/download?f={filename}.jar"

    def _load_version(self, version: VersionHandle, watcher: Watcher) -> bool:
        """Same as in ForgeVersion"""
        if version.id == self.version:
            return version.read_metadata_file()
        else:
            return super()._load_version(version, watcher)

    def _resolve_jar(self, watcher: Watcher) -> None:
        """Resolves the Optifine installer jar and the client jar"""
        super()._resolve_jar(watcher)
        if not (self._hierarchy[0].dir / INSTALLER_FILENAME).exists():
            install_jar_url = self.dl_url()
            self._dl.add(DownloadEntry(install_jar_url, self._hierarchy[0].dir / INSTALLER_FILENAME))

        self._download(watcher) # download needed ressources (vanilla client jar, jvms, and optifine installer jar)
        self._finalize_optifine(watcher)

    def _fetch_version(self, version: VersionHandle, watcher: Watcher) -> None:
        """
        builds the optifine core metadata if it doesn't exist
        """
        if version.id != self.version:
            return super()._fetch_version(version, watcher)
        if version.read_metadata_file():
            for key in self._of_base_json().keys():
                if key not in version.metadata:
                    version.metadata[key] = self._of_base_json()[key]

        else:
            version.metadata = self._of_base_json() # if the version metadata is not found, create it
        version.write_metadata_file()

    def _of_base_json(self):
        """
        Returns the base json for the optifine metadata file.
        This generates a very basic json to allow portablemc to download dependencies before installing optifine
        """
        return {
            "id": self.version,
            "inheritsFrom": self.mcver(),
            "time": datetime.now().strftime("%Y-%m-%dT%H:%M:%S%z"),
            "releaseTime": datetime.now().strftime("%Y-%m-%dT%H:%M:%S%z"),
            "type": "release"
        }

    def _build_optifine_json(self, launchwrapper_version: str, parent_data: dict, ofedition: str, ofchecksum: str = None,launchwrapper_checksum: str = None) -> dict:
        """Update the JSON configuration."""
        new_json = self._of_base_json()
        # Update the JSON with the optifine library name and main class
        of_lib_entry=({"name": f"optifine:OptiFine:{self.mcver()}_{ofedition}", "sha1": ofchecksum} if ofchecksum is not None
                 else {"name": f"optifine:OptiFine:{self.mcver()}_{ofedition}"})
        new_json.update( **{
            "libraries": [
                of_lib_entry
            ],
            "mainClass": "net.minecraft.launchwrapper.Launch"
        })

        if launchwrapper_version == "net.minecraft:launchwrapper:1.12":
            new_json["libraries"].append({"name": "net.minecraft:launchwrapper:1.12", "size": 32999,
                                              "url": "https://repo.papermc.io/repository/maven-public/"})
        else:
            launchwrapper_entry=({"name": launchwrapper_version, "sha1": launchwrapper_checksum} if launchwrapper_checksum is not None
                                 else {"name": launchwrapper_version})
            new_json["libraries"].append(launchwrapper_entry)
        if "minecraftArguments" in parent_data:
            new_json["minecraftArguments"] = parent_data["minecraftArguments"] + " --tweakClass optifine.OptiFineTweaker"
        else:
            new_json["minimumLauncherVersion"] = 21
            new_json["arguments"] = {
                "game": ["--tweakClass", "optifine.OptiFineTweaker"]
            }
        return new_json

    def _finalize_optifine(self, watcher: Watcher) -> None:
        try:
            self._finalize_optifine_internal(watcher)
            # _finalize_optifine is updating the version metadata, so reload it
            self._resolve_metadata(watcher)
        except Exception as e:
            version = self._hierarchy[0]
            version.metadata = self._of_base_json() # put back a basic metadata in the json to make sure it isn't broken
            version.write_metadata_file()
            jar_path = self._hierarchy[0].dir / INSTALLER_FILENAME
            jar_path.unlink()
            raise e # finnaly raise the exeption that occured, after fixing directory

    def check_of_install(self, version: VersionHandle) -> bool:
        """
        Check if all needed ressources are properly installed to allow standard installation.
        """
        if not version.read_metadata_file():
            return False
        if "inheritsFrom" in version.metadata and version.metadata["inheritsFrom"] == self.mcver():
            if "libraries" in version.metadata:
                launchwrapperseemscorrect = False # This variable is used to check if the launchwrapper is correct
                oflibseemscorrect = False
                for lib in version.metadata["libraries"]:
                    libpath = LibrarySpecifier.from_str(lib["name"])
                    if not (self.context.libraries_dir / libpath.file_path()).exists() and not "url" in lib.keys():
                        return False

                    else:
                        if "sha1" in lib.keys(): # A functionality that check if libs remains the same
                            with open(self.context.libraries_dir / libpath.file_path(), "rb") as f:
                                data = f.read()
                            if not sha1(data).hexdigest() == lib["sha1"]:
                                return False

                    if lib["name"].startswith("optifine:launchwrapper-of:") or "net.minecraft:launchwrapper:1.12" == lib["name"]:
                        launchwrapperseemscorrect = True

                    if lib["name"] == f"optifine:OptiFine:{self.mcver()}_{self.loader()}":
                        oflibseemscorrect = True


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
                    return launchwrapperseemscorrect and oflibseemscorrect

                elif ("arguments" in version.metadata and
                    "game" in version.metadata["arguments"] and
                    "--tweakClass" in version.metadata["arguments"]["game"] and
                    "optifine.OptiFineTweaker" in version.metadata["arguments"]["game"]):
                    return launchwrapperseemscorrect and oflibseemscorrect

                else:
                    return False

        return False

    def _finalize_optifine_internal(self, watcher: Watcher) -> None:
        version = self._hierarchy[0]

        if not self.check_of_install(version): # if the version is not installed, install it
            watcher.handle(OptifineStartInstallEvent())
            jar_path=self._hierarchy[0].dir / INSTALLER_FILENAME
            with zipfile.ZipFile(jar_path, "r") as jar:
                try:
                    launchwrapper_version=jar.open("launchwrapper-of.txt").read().decode("utf-8").strip()
                    launchwrapper = f"optifine:launchwrapper-of:{launchwrapper_version}"
                except KeyError:
                    launchwrapper = "net.minecraft:launchwrapper:1.12"

                minecraft_ver = self._hierarchy[1]
                if minecraft_ver.read_metadata_file():
                    parent_data = minecraft_ver.metadata
                else:
                    mcver_url=self.manifest.get_version(self.mcver)["url"]
                    res = http_request("GET", mcver_url, accept = "application/json")
                    parent_data = res.json()
                    minecraft_ver.metadata = parent_data
                    minecraft_ver.write_metadata_file()

                # patch minecraft version jar to build Optifine library
                of_lib_dir=self.context.libraries_dir / "optifine" / "OptiFine"/ f"{self.mcver()}_{self.loader()}"
                if not of_lib_dir.exists(): # makes the library directory
                    of_lib_dir.mkdir(parents = True,exist_ok = True)

                Patcher.process(version.dir / f"{self.version}.jar",
                                version.dir / INSTALLER_FILENAME,
                                self.context.libraries_dir / "optifine" / "OptiFine" / f"{self.mcver()}_{self.loader()}" / f"OptiFine-{self.mcver()}_{self.loader()}.jar",
                                watcher = watcher)

                launchwrapper_sha1 = None
                if not launchwrapper == "net.minecraft:launchwrapper:1.12":
                    # install launchwrapper library if it is not default launchwrapper
                    file_name = f"launchwrapper-of-{launchwrapper_version}.jar"
                    dir_dest = self.context.libraries_dir / "optifine" / "launchwrapper-of" / launchwrapper_version
                    file_dest = dir_dest / file_name
                    dir_dest.mkdir(parents=True, exist_ok=True)
                    with jar.open(file_name) as raw_launchwrapper:
                        launchwrapper_data = raw_launchwrapper.read()
                        with open(file_dest, "wb") as launchwrapper_f:
                            launchwrapper_f.write(launchwrapper_data)
                        launchwrapper_sha1 = sha1(launchwrapper_data).hexdigest()

            with open(self.context.libraries_dir / "optifine" / "OptiFine" / f"{self.mcver()}_{self.loader()}" / f"OptiFine-{self.mcver()}_{self.loader()}.jar", "rb") as f:
                libdata = f.read()

            oflibdigest = sha1(libdata).hexdigest()
            version.metadata = self._build_optifine_json(launchwrapper, parent_data, self.loader(), ofchecksum = oflibdigest, launchwrapper_checksum = launchwrapper_sha1)
            version.write_metadata_file()
            watcher.handle(OptifineEndInstallEvent(self.version))
