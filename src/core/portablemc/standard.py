"""Definition of the standard tasks for vanilla Minecraft, versions
are provided by Mojang through their version manifest (see associated
module).
"""

from json import JSONDecodeError
from pathlib import Path
import platform
import json
import re

from portablemc.task import State, Watcher

from .task import Sequence, Task, State, Watcher
from .download import DownloadList, DownloadEntry, DownloadTask
from .manifest import VersionManifest
from .http import http_request, json_simple_request
from .util import LibrarySpecifier, calc_input_sha1, merge_dict

from typing import Optional, List, Iterator, Any, Dict


class Context:
    """Base class for storing global context of a standard Minecraft launch, such as main 
    and working directory.
    """

    def __init__(self, 
        main_dir: Optional[Path] = None, 
        work_dir: Optional[Path] = None
    ) -> None:
        """Construct a Minecraft installation context. This context is used by most of the
        installer's tasks to know where to install files and from where to launch the game.

        :param main_dir: The main directory where versions, assets, libraries and 
        optionally JVM are installed. If not specified this path will be set the usual 
        `.minecraft` (see https://minecraft.fandom.com/fr/wiki/.minecraft).
        :param work_dir: The working directory from where the game is run, the game stores
        thing like saves, resource packs, options and mods if relevant. This defaults to
        `main_dir` if not specified.
        """

        main_dir = get_minecraft_dir() if main_dir is None else main_dir
        self.work_dir = main_dir if work_dir is None else work_dir
        self.versions_dir = main_dir / "versions"
        self.assets_dir = main_dir / "assets"
        self.libraries_dir = main_dir / "libraries"
        self.jvm_dir = main_dir / "jvm"
        self.bin_dir = self.work_dir / "bin"

    def get_version(self, version_id: str) -> "Version":
        """Get an instance of the given version.
        """
        return Version(version_id, self.versions_dir / version_id)

    # def list_versions(self) -> Generator[Tuple[str, int], None, None]:
    #     """ A generator method that yields all versions (version, mtime) that have a version metadata file. """
    #     if path.isdir(self.versions_dir):
    #         for version in os.listdir(self.versions_dir):
    #             try:
    #                 yield version, path.getmtime(path.join(self.versions_dir, version, f"{version}.json"))
    #             except OSError:
    #                 pass


class VersionId:
    """Small class used to specify which Minecraft version to launch.
    """

    __slots__ = "id",

    def __init__(self, id: str) -> None:
        self.id = id


class FullMetadata:
    """Fully computed metadata, all the layers are merged together.
    """

    def __init__(self, data: dict) -> None:
        self.data = data


class Version:
    """This class holds version's metadata, as resolved by `MetadataTask`.
    """

    __slots__ = "id", "dir", "metadata", "parent"

    def __init__(self, id: str, dir: Path) -> None:
        self.id = id
        self.dir = dir
        self.metadata = {}
        self.parent: Optional[Version] = None
    
    def metadata_exists(self) -> bool:
        """This function returns true if the version's metadata file exists.
        """
        return self.metadata_file().is_file()
    
    def metadata_file(self) -> Path:
        """This function returns the computed path of the metadata file.
        """
        return self.dir / f"{self.id}.json"

    def jar_file(self) -> Path:
        """This function returns the computed path of the JAR file of the game.
        """
        return self.dir / f"{self.id}.jar"

    def write_metadata_file(self) -> None:
        """This function write the metadata file of the version with the internal data.
        """
        self.dir.mkdir(parents=True, exist_ok=True)
        with self.metadata_file().open("wt") as fp:
            json.dump(self.metadata, fp)

    def read_metadata_file(self) -> bool:
        """This function reads the metadata file and updates the internal data if found.

        :return: True if the data was actually updated from the file.
        """
        try:
            with self.metadata_file().open("rt") as fp:
                self.metadata = json.load(fp)
            return True
        except (OSError, JSONDecodeError):
            return False
    
    def recurse(self) -> "Iterator[Version]":
        """Walk through every version metadata in the hierarchy of the current one.
        """
        version_meta = self
        while version_meta is not None:
            yield version_meta
            version_meta = version_meta.parent
    
    def merge(self) -> FullMetadata:
        """Merge this version metadata and all of its parents into a `FullMetadata`.
        """
        result = {}
        for version_meta in self.recurse():
            merge_dict(result, version_meta.metadata)
        return FullMetadata(result)


class VersionJar:
    """This state object contains the version JAR to use for launching the game.
    """

    def __init__(self, version_id: str, path: Path) -> None:
        self.version_id = version_id
        self.path = path


class VersionAssets:
    """Represent the loaded assets for the current version. This contains the index 
    version as well as all assets and if they need to copied to virtual or resources
    directory.
    """
    
    def __init__(self, index_version: str, assets: Dict[str, Path], virtual: bool, resources: bool) -> None:
        self.index_version = index_version
        self.assets = assets
        self.virtual = virtual
        self.resources = resources


class VersionLibraries:
    """Represent the loaded libraries for the current version. This contains both 
    classpath libraries that should be added to the classpath, and the native libraries
    that should be extracted in a temporary bin directory. These native libraries are no
    longer used in modern versions.
    """

    def __init__(self, class_libs: List[str], native_libs: List[str]) -> None:
        self.class_libs = class_libs
        self.native_libs = native_libs


class VersionLogging:
    """Represent the loaded logging configuration for the current version. It contain
    the logging file's path and the argument to add to the command line, this argument
    contains a format placeholder '${path}' that should be replaced with the file's path.
    """

    def __init__(self, file: Path, arg: str) -> None:
        self.file = file
        self.arg = arg


class TooMuchParentError(Exception):
    """Raised when a 
    """

class MetadataInvalidError(Exception):
    """A specialized error type when metadata is invalid. This errors contains the path
    of the value being problematic and the expected type. The value may be None, in such
    case it means that the value was not present as required.
    """

    def __init__(self, value: Any, expected_type: type, path: str) -> None:
        self.value = value
        self.expected_type = expected_type
        self.path = path


class JarNotFoundError(Exception):
    """Raised when no version's JAR file could be found from the metadata.
    """


EV_VERSION_RESOLVING = "version_resolving"
EV_VERSION_RESOLVED = "version_resolved"
EV_ASSETS_PROGRESS = "assets_progress"
EV_LIBRARY_RESOLVING = "library_resolving"
EV_LIBRARY_RESOLVED = "library_resolved"

RESOURCES_URL = "https://resources.download.minecraft.net/"
LIBRARIES_URL = "https://libraries.minecraft.net/"
JVM_META_URL = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json"


class MetadataTask(Task):
    """Version metadata resolving.

    This task resolves the current version's metadata and inherited 
    versions.

    :in Context: The installation context.
    :in VersionId: Describe which version to resolve.
    :in VersionManifest: Version manifest to use for fetching online official versions.
    :out VersionMetadata: The resolved version metadata.
    """

    def __init__(self, *, max_parents: int = 10) -> None:
        self.max_parents = max_parents

    def execute(self, state: State, watcher: Watcher) -> None:

        context = state[Context]
        manifest = state[VersionManifest]

        version_id: Optional[str] = state[VersionId].id
        versions: List[Version] = []

        while version_id is not None:

            if len(versions) > self.max_parents:
                raise TooMuchParentError()
            
            watcher.on_event(EV_VERSION_RESOLVING, id=version_id)
            version = context.get_version(version_id)
            self.ensure_version_meta(version, manifest)
            watcher.on_event(EV_VERSION_RESOLVED, id=version_id)

            # Set the parent of the last version to the version being resolved.
            if len(versions):
                versions[-1].parent = version
            
            versions.append(version)
            version_id = version.metadata.pop("inheritsFrom", None)
            
            if version_id is not None and not isinstance(version_id, str):
                raise ValueError("metadata: /inheritsFrom must be a string")

        # The metadata is included to the state.
        state.insert(versions[0])
        state.insert(versions[0].merge())

    def ensure_version_meta(self, version: Version, manifest: VersionManifest) -> None:
        """This function tries to load and get the directory path of a given version id.
        This function proceeds in multiple steps: it tries to load the version's metadata,
        if the metadata is found then it's validated with the `validate_version_meta`
        method. If the version was not found or is not valid, then the metadata is fetched
        by `fetch_version_meta` method.

        :param version: The version metadata to fill with 
        """

        if version.read_metadata_file():
            if self.validate_version_meta(version, manifest):
                # If the version is successfully loaded and valid, return as-is.
                return
        
        # If not loadable or not validated, fetch metadata.
        self.fetch_version_meta(version, manifest)

    def validate_version_meta(self, version: Version, manifest: VersionManifest) -> bool:
        """This function checks that version metadata is correct.

        An internal method to check if a version's metadata is 
        up-to-date, returns `True` if it is. If `False`, the version 
        metadata is re-fetched, this is the default when version 
        metadata doesn't exist.

        The default implementation check official versions against the
        expected SHA1 hash of the metadata file.

        :param version: The version metadata to validate or not.
        :return: True if the given version.
        """

        version_super_meta = manifest.get_version(version.id)
        if version_super_meta is None:
            return True
        else:
            expected_sha1 = version_super_meta.get("sha1")
            if expected_sha1 is not None:
                try:
                    with version.metadata_file().open("rb") as version_meta_fp:
                        current_sha1 = calc_input_sha1(version_meta_fp)
                        return expected_sha1 == current_sha1
                except OSError:
                    return False
            return True

    def fetch_version_meta(self, version: Version, manifest: VersionManifest) -> None:
        """Internal method to fetch the data of the given version.

        :param version: The version meta to fetch data into.
        :raises VersionError: TODO
        """

        version_super_meta = manifest.get_version(version.id)
        if version_super_meta is None:
            raise ValueError("Version not found")  # TODO: Specialized error
        
        code, raw_data = http_request(version_super_meta["url"], "GET")
        if code != 200:
            raise ValueError("HTTP error: {raw_data}")  # TODO: Specialized error
        
        # First decode the data and set it to the version meta. Raising if invalid.
        version.metadata = json.loads(raw_data)
        
        # If successful, write the raw data directly to the file.
        version.dir.mkdir(parents=True, exist_ok=True)
        with version.metadata_file().open("wb") as fp:
            fp.write(raw_data)


class JarTask(Task):
    """Version JAR file resolving task.

    This task resolve which JAR file should be used to run the current version. This can
    be any valid JAR file present in the version's hierarchy.

    :in VersionMetadata: The version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out VersionJar: The resolved version JAR with associated version ID providing it.
    """

    def execute(self, state: State, watcher: Watcher) -> None:

        # Try finding a JAR to use in the hierarchy of versions.
        for version_meta in state[Version].recurse():
            jar_file = version_meta.jar_file()
            # First try to find a /downloads/client download entry.
            version_dls = version_meta.metadata.get("downloads")
            if version_dls is not None:
                if not isinstance(version_dls, dict):
                    raise ValueError("metadata: /downloads must be an object")
                client_dl = version_dls.get("client")
                if client_dl is not None:
                    entry = parse_download_entry(client_dl, jar_file, "metadata: /downloads/client")
                    state[DownloadList].add(entry, verify=True)
                    state.insert(VersionJar(version_meta.id, jar_file))
                    return
            # If no download entry has been found, but the JAR exists, we use it.
            if jar_file.is_file():
                state.insert(VersionJar(version_meta.id, jar_file))
                return
        
        raise JarNotFoundError()


class AssetsTask(Task):
    """Version assets resolving task.

    This task resolves which asset index to use for the version.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out VersionAssets: Optional, present if the assets are specified.
    """

    def execute(self, state: State, watcher: Watcher) -> None:
        
        context = state[Context]
        metadata = state[FullMetadata].data
        dl = state[DownloadList]

        assets_index_info = metadata.get("assetIndex")
        if assets_index_info is None:
            # Asset info may not be present, it's not required because some custom 
            # versions may want to use there own internal assets.
            return
        
        if not isinstance(assets_index_info, dict):
            raise ValueError("metadata: /assetIndex must be an object")
        
        assets_index_version = metadata.get("assets", assets_index_info.get("id"))
        if assets_index_version is None:
            # Same as above.
            return
        
        if not isinstance(assets_index_version, str):
            raise ValueError("metadata: /assets or /assetIndex/id must be a string")

        assets_indexes_dir = context.assets_dir / "indexes"
        assets_index_file = assets_indexes_dir / f"{assets_index_version}.json"

        try:
            with open(assets_index_file, "rb") as assets_index_fp:
                assets_index = json.load(assets_index_fp)
        except (OSError, JSONDecodeError):

            # If for some reason we can't read an assets index, try downloading it.
            assets_index_url = assets_index_info.get("url")
            if not isinstance(assets_index_url, str):
                raise ValueError("metadata: /assetIndex/url must be a string")
            
            # TODO: Handle non-200 codes.
            assets_index = json_simple_request(assets_index_url)
            assets_indexes_dir.mkdir(parents=True, exist_ok=True)
            with assets_index_file.open("wt") as assets_index_fp:
                json.dump(assets_index, assets_index_fp)

        assets_objects_dir = context.assets_dir / "objects"
        assets_resources = assets_index.get("map_to_resources", False)  # For version <= 13w23b
        assets_virtual = assets_index.get("virtual", False)  # For 13w23b < version <= 13w48b (1.7.2)

        if not isinstance(assets_resources, bool):
            raise ValueError("assets index: /map_to_resources must be a boolean")
        if not isinstance(assets_virtual, bool):
            raise ValueError("assets index: /virtual must be a boolean")

        assets_objects = assets_index.get("objects")
        if not isinstance(assets_objects, dict):
            raise ValueError("assets index: /objects must be an object")

        assets = {}
        for asset_id, asset_obj in assets_objects.items():

            if not isinstance(asset_obj, dict):
                raise ValueError(f"assets index: /objects/{asset_id} must be an object")

            asset_hash = asset_obj.get("hash")
            if not isinstance(asset_hash, str):
                raise ValueError(f"assets index: /objects/{asset_id}/hash must be a string")
            
            asset_size = asset_obj.get("size")
            if not isinstance(asset_size, int):
                raise ValueError(f"assets index: /objects/{asset_id}/size must be an integer")

            asset_hash_prefix = asset_hash[:2]
            asset_file = assets_objects_dir.joinpath(asset_hash_prefix, asset_hash)
            assets[asset_id] = asset_file
            if not asset_file.is_file() or asset_file.stat().st_size != asset_size:
                asset_url = f"{RESOURCES_URL}{asset_hash_prefix}/{asset_hash}"
                dl.add(DownloadEntry(asset_url, asset_file, size=asset_size, sha1=asset_hash, name=asset_id))
        
        state.insert(VersionAssets(assets_index_version, assets, assets_virtual, assets_resources))


class LibrariesTask(Task):
    """Version libraries resolving task.

    This task resolves which libraries should be used for running the selected version.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out VersionLibraries: Optional, present if the libraries are specified.
    """

    def execute(self, state: State, watcher: Watcher) -> None:
        
        context = state[Context]
        metadata = state[FullMetadata].data
        dl = state[DownloadList]

        class_libs = []
        native_libs = []

        libraries = metadata.get("libraries")
        if libraries is None:
            # Libraries are not inherently required.
            return
        
        if not isinstance(libraries, list):
            raise ValueError("metadata: /libraries must be a list")

        for library_idx, library in enumerate(libraries):

            if not isinstance(library, dict):
                raise ValueError(f"metadata: /libraries/{library_idx} must be an object")
            
            name = library.get("name")
            if not isinstance(name, str):
                raise ValueError(f"metadata: /libraries/{library_idx}/name must be a string")
            
            spec = LibrarySpecifier.from_str(name)

            rules = library.get("rules")
            if rules is not None:

                if not isinstance(rules, list):
                    raise ValueError(f"metadata: /libraries/{library_idx}/rules must be a list")
                
                if not interpret_rule(rules):
                    continue

            watcher.on_event(EV_LIBRARY_RESOLVING, spec=spec)

            # TODO: Predicates??

            # Old metadata files provides a 'natives' mapping from OS to the classifier
            # specific for this OS.
            natives = library.get("natives")

            if natives is not None:

                if not isinstance(natives, dict):
                    raise ValueError(f"metadata: /libraries/{library_idx}/natives must be an object")
                
                # If natives object is present, the classifier associated to the
                # OS overrides the lib_spec classifier.
                spec.classifier = natives.get(get_minecraft_os())
                if spec.classifier is None:
                    continue

                archbits = get_minecraft_archbits()
                if archbits is not None:
                    spec.classifier = spec.classifier.replace("${arch}", archbits)
                
                libs = native_libs

            else:
                libs = class_libs
            
            dl_entry: Optional[DownloadEntry] = None
            jar_path_rel = spec.jar_file_path()
            jar_path = context.libraries_dir / jar_path_rel
            
            downloads = library.get("downloads")
            if downloads is not None:

                if not isinstance(downloads, dict):
                    raise ValueError(f"metadata: /libraries/{library_idx}/downloads must be an object")

                if natives is not None:
                    # Only check classifiers if natives mapping is present.
                    lib_dl_classifiers = downloads.get("classifiers")
                    dl_meta = None if lib_dl_classifiers is None else lib_dl_classifiers.get(spec.classifier)
                else:
                    # If we are not dealing with natives, just take the artifact.
                    dl_meta = downloads.get("artifact")

                if dl_meta is not None:
                    dl_entry = parse_download_entry(dl_meta, jar_path, f"metadata: /libraries/{library_idx}/downloads/artifact")

            # If no download entry can be found, add a default one that points to official
            # library repository, this may not work.
            if dl_entry is None:

                # The official launcher seems to default to their repository, it will also
                # allows us to prevent launch if such lib cannot be found.
                repo_url = library.get("url", LIBRARIES_URL)
                if not isinstance(repo_url, str):
                    raise ValueError(f"metadata: /libraries/{library_idx}/url must be a string")
                
                # Let's be sure to have a '/' as last character.
                if repo_url[-1] != "/":
                    repo_url += "/"
                
                dl_entry = DownloadEntry(f"{repo_url}{jar_path_rel}", jar_path)

            libs.append(jar_path)

            if dl_entry is not None:
                dl_entry.name = str(spec)
                dl.add(dl_entry, verify=True)
            
            watcher.on_event(EV_LIBRARY_RESOLVED, spec=spec)

        state.insert(VersionLibraries(class_libs, native_libs))


class LoggerTask(Task):
    """Logger resolving task.

    This task resolves which logger configuration to use for the selected version.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    """

    def execute(self, state: State, watcher: Watcher) -> None:
        
        context = state[Context]
        metadata = state[FullMetadata].data
        dl = state[DownloadList]

        logging = metadata.get("logging")
        if logging is None:
            return
        
        if not isinstance(logging, dict):
            raise ValueError("metadata: /logging must be an object")
        
        client_logging = logging.get("client")
        if client_logging is None:
            return
        
        if not isinstance(client_logging, dict):
            raise ValueError("metadata: /logging/client must be an object")
        
        argument = client_logging.get("argument")
        if not isinstance(argument, str):
            raise ValueError("metadata: /logging/client/argument must be a string")

        file_info = client_logging.get("file")
        if not isinstance(file_info, dict):
            raise ValueError("metadata: /logging/client/file must be an object")
        
        file_id = file_info.get("id")
        if not isinstance(file_id, str):
            raise ValueError("metadata: /logging/client/file/id must be a string")

        file_path = context.assets_dir / "log_configs" / file_id
        dl_entry = parse_download_entry(file_info, file_path, "metadata: /logging/client/file")
        dl.add(dl_entry, verify=True)

        state.insert(VersionLogging(file_path, argument))


# TODO: class JvmTask(Task):


class RunTask(Task):

    def execute(self, state: State, watcher: Watcher) -> None:
        pass


def parse_download_entry(value: Any, dst: Path, path: str) -> DownloadEntry:

    if not isinstance(value, dict):
        raise ValueError(f"{path} must an object")

    url = value.get("url")
    if not isinstance(url, str):
        raise ValueError(f"{path}/url must be a string")

    size = value.get("size")
    if size is not None and not isinstance(size, int):
        raise ValueError(f"{path}/size must be an integer")

    sha1 = value.get("sha1")
    if sha1 is not None and not isinstance(sha1, str):
        raise ValueError(f"{path}/sha1 must be a string")

    return DownloadEntry(url, dst, size=size, sha1=sha1, name=dst.name)


def get_minecraft_dir() -> Path:
    """Internal function to get the default directory for installing
    and running Minecraft.
    """
    home = Path.home()
    return {
        "Windows": home.joinpath("AppData", "Roaming", ".minecraft"),
        "Darwin": home.joinpath("Library", "Application Support", "minecraft"),
    }.get(platform.system(), home / ".minecraft")


def interpret_rule(rules: list, features: dict = {}) -> bool:
    """
    """
    # NOTE: Do not modify 'features' because of the default singleton.
    allowed = False
    for rule in rules:
        rule_os = rule.get("os")
        if rule_os is not None and not interpret_rule_os(rule_os):
            continue
        rule_features: Optional[dict] = rule.get("features")
        if rule_features is not None:
            feat_valid = True
            for feat_name, feat_expected in rule_features.items():
                if features.get(feat_name) != feat_expected:
                    feat_valid = False
                    break
            if not feat_valid:
                continue
        allowed = (rule["action"] == "allow")
    return allowed


def interpret_rule_os(rule_os: dict) -> bool:
    os_name = rule_os.get("name")
    if os_name is None or os_name == get_minecraft_os():
        os_arch = rule_os.get("arch")
        if os_arch is None or os_arch == get_minecraft_arch():
            os_version = rule_os.get("version")
            if os_version is None or re.search(os_version, platform.version()) is not None:
                return True
    return False


_minecraft_os: Optional[str] = None
def get_minecraft_os() -> Optional[str]:
    """Return the current OS identifier used in rules matching, 'linux', 'windows', 'osx',
    and 'freebsd'.
    """
    global _minecraft_os
    if _minecraft_os is None:
        _minecraft_os = {
            "Linux": "linux", 
            "Windows": "windows", 
            "Darwin": "osx",
            "FreeBSD": "freebsd"
        }.get(platform.system(), "")
    return _minecraft_os


_minecraft_arch: Optional[str] = None
def get_minecraft_arch() -> Optional[str]:
    """Return the architecture to use in rules matching, 'x86', 'x86_64', 'arm64' or 
    'arm32' if not found.
    """
    global _minecraft_arch
    if _minecraft_arch is None:
        _minecraft_arch = {
            "i386": "x86",
            "i686": "x86",
            "x86_64": "x86_64",
            "amd64": "x86_64",
            "arm64": "arm64",
            "aarch64": "arm64",
            "armv7l": "arm32",
            "armv6l": "arm32",
        }.get(platform.machine().lower(), "")
    return _minecraft_arch


_minecraft_archbits: Optional[str] = None
def get_minecraft_archbits() -> Optional[str]:
    """Return the address size of the architecture used for rules matching, '64', '32', 
    or '' if not found.
    """
    global _minecraft_archbits
    if _minecraft_archbits is None:
        raw_bits = platform.architecture()[0]
        _minecraft_archbits = "64" if raw_bits == "64bit" else "32" if raw_bits == "32bit" else ""
    return _minecraft_archbits


# _minecraft_jvm_os: Optional[str] = None
# def get_minecraft_jvm_os() -> Optional[str]:
#     """Return the OS identifier used to choose the right JVM to download.
#     """
#     global _minecraft_jvm_os
#     if _minecraft_jvm_os is None:
#         _minecraft_jvm_os = {
#             "osx": {"x86_64": "mac-os", "arm64": "mac-os-arm64"},
#             "linux": {"x86": "linux-i386", "x86_64": "linux"},
#             "windows": {"x86": "windows-x86", "x86_64": "windows-x64"}
#         }.get(get_minecraft_os(), {}).get(get_minecraft_arch())
#     return _minecraft_jvm_os


def make_standard_sequence(version_id: str, *, 
    context: Optional[Context] = None, 
    version_manifest: Optional[VersionManifest] = None) -> Sequence:
    """Make standard installer for installing standard Minecraft versions.

    :param context: The directory context of the game's installation.
    :return: The installer, ready to install a standard game.
    """

    seq = Sequence()
    seq.insert_state(VersionId(version_id))
    seq.insert_state(context or Context())
    seq.insert_state(version_manifest or VersionManifest())
    seq.append_task(MetadataTask())
    seq.append_task(JarTask())
    seq.append_task(AssetsTask())
    seq.append_task(LibrariesTask())
    seq.append_task(LoggerTask())
    seq.append_task(DownloadTask())
    return seq
