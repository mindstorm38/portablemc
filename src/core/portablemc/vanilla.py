"""Definition of the standard tasks for vanilla Minecraft, versions
are provided by Mojang through their version manifest (see associated
module).
"""

from json import JSONDecodeError
from pathlib import Path
import platform
import shutil
import json
import re
import os

from .util import LibrarySpecifier, calc_input_sha1, merge_dict, jvm_bin_filename
from .download import DownloadList, DownloadEntry, DownloadTask
from .auth import AuthSession, OfflineAuthSession
from .task import Sequence, Task, State, Watcher
from .http import HttpError, http_request

from . import LAUNCHER_NAME, LAUNCHER_VERSION

from typing import Optional, List, Iterator, Any, Dict, Tuple, Union


RESOURCES_URL = "https://resources.download.minecraft.net/"
LIBRARIES_URL = "https://libraries.minecraft.net/"
JVM_META_URL = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json"
VERSION_MANIFEST_URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


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

    def list_versions(self) -> "Iterator[Version]":
        """List versions in the context. 
        Only versions with existing metadata are returned.
        """
        if self.versions_dir.is_dir():
            for version_dir in self.versions_dir.iterdir():
                if version_dir.is_dir():
                    version = Version(version_dir.name, version_dir)
                    if version.metadata_exists():
                        yield version


class VersionManifest:
    """The Mojang's official version manifest. Providing officially
    available versions with optional cache file.
    """

    def __init__(self, cache_file: Optional[Path] = None) -> None:
        self.data: Optional[dict] = None
        self.cache_file = cache_file

    def _ensure_data(self) -> dict:
        """Internal method that ensure that the manifest data is up-to-date.

        :return: The full data of the manifest.
        :raises HttpError: Underlying HTTP error if manifest could not be requested.
        """

        if self.data is None:

            headers = {}
            cache_data = None

            # If a cache file should be used, try opening it and read the last modified
            # time that will be used for requesting the manifest, only if needed.
            if self.cache_file is not None:
                try:
                    with self.cache_file.open("rt") as cache_fp:
                        cache_data = json.load(cache_fp)
                    if "last_modified" in cache_data:
                        headers["If-Modified-Since"] = cache_data["last_modified"]
                except (OSError, json.JSONDecodeError):
                    pass
            
            try:

                res = http_request("GET", VERSION_MANIFEST_URL, 
                    headers=headers, 
                    accept="application/json")
                
                self.data = res.json()

                if "Last-Modified" in res.headers:
                    self.data["last_modified"] = res.headers["Last-Modified"]

                if self.cache_file is not None:
                    self.cache_file.parent.mkdir(parents=True, exist_ok=True)
                    with self.cache_file.open("wt") as cache_fp:
                        json.dump(self.data, cache_fp, indent=2)

            except HttpError as error:
                res = error.res
                if res.status == 304 and cache_data is not None:
                    self.data = cache_data
                else:
                    raise

        return self.data

    def filter_latest(self, version: str) -> Tuple[str, bool]:
        """Filter a version identifier if 'release' or 'snapshot' alias is used, then it's
        replaced by the full version identifier, like `1.19.3`.

        :param version: The version id or alias.
        :return: A tuple containing the full version id and a boolean indicating if the
        given version identifier is an alias.
        :raises HttpError: Underlying HTTP error if manifest could not be requested.
        """

        if version in ("release", "snapshot"):
            latest = self._ensure_data()["latest"].get(version)
            if latest is not None:
                return latest, True
        return version, False

    def get_version(self, version: str) -> Optional[dict]:
        """Get a manifest's version metadata. Containing the metadata's URL, its SHA1 and
        its type.

        :param version: The version identifier.
        :return: If found, the version is returned.
        :raises HttpError: Underlying HTTP error if manifest could not be requested.
        """
        version, _alias = self.filter_latest(version)
        for version_data in self._ensure_data()["versions"]:
            if version_data["id"] == version:
                return version_data
        return None

    def all_versions(self) -> list:
        return self._ensure_data()["versions"]


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
    
    def __init__(self, index_version: str, assets: Dict[str, Path], virtual_dir: Optional[Path], resources_dir: Optional[Path]) -> None:
        self.index_version = index_version
        self.assets = assets
        self.virtual_dir = virtual_dir
        self.resources_dir = resources_dir

class VersionLibraries:
    """Represent the loaded libraries for the current version. This contains both 
    classpath libraries that should be added to the classpath, and the native libraries
    that should be extracted in a temporary bin directory. These native libraries are no
    longer used in modern versions.
    """

    def __init__(self, class_libs: List[Path], native_libs: List[Path]) -> None:
        self.class_libs = class_libs
        self.native_libs = native_libs

class VersionLogging:
    """Represent the loaded logging configuration for the current version. It contain
    the logging file's path and the argument to add to the command line, this argument
    contains a format placeholder '${path}' that should be replaced with the file's path.
    """

    def __init__(self, path: Path, arg: str) -> None:
        self.path = path
        self.arg = arg

class VersionJvm:
    """Indicates JVM to use for running the game. This state is automatically generated
    by the 'JvmTask' when used and successful. It specifies the executable file's path
    and the JVM version selected. The JVM version is optional because it may not be
    known at this time.
    """

    def __init__(self, executable_file: Path, version: Optional[str]) -> None:
        self.executable_file = executable_file
        self.version = version

class VersionArgsOptions:
    """Options used for the preparation of `VersionArgs` by `ArgsTask`. These options are
    optional for this task but can be used to specify various options such as 
    authentication session to use or server address to start Minecraft with.
    """

    def __init__(self):
        self.auth_session: Optional[AuthSession] = None
        self.demo: bool = False
        self.resolution: Optional[Tuple[int, int]] = None
        self.disable_multiplayer: bool = False
        self.disable_chat: bool = False
        self.server_address: Optional[str] = None
        self.server_port: Optional[int] = None
        self.fix_legacy: bool = True         # For legacy merge sort and betacraft proxy.
        self.features: Dict[str, bool] = {}  # Additional features for metadata args.

    @classmethod
    def with_online(cls, auth_session: AuthSession) -> "VersionArgsOptions":
        """Make arguments options with an online authentication session.
        """
        opts = cls()
        opts.auth_session = auth_session
        return opts

    @classmethod
    def with_offline(cls, username: Optional[str], uuid: Optional[str]) -> "VersionArgsOptions":
        """Make arguments options with an offline username and UUID.
        """
        opts = cls()
        opts.auth_session = OfflineAuthSession(uuid, username)
        return opts

class VersionArgs:
    """Indicates java arguments, game arguments and main class required to run the game.
    This can later be used to run the game and is usually prepared by the `ArgsTask`.
    """

    def __init__(self, jvm_args: List[str], game_args: List[str], main_class: str, args_replacements: Dict[str, str]) -> None:
        self.jvm_args = jvm_args
        self.game_args = game_args
        self.main_class = main_class
        self.args_replacements = args_replacements


class VersionNotFoundError(Exception):
    """Raised when a version was not found. The version that was not found is given
    """

    def __init__(self, version: Version) -> None:
        self.version = version

class TooMuchParentsError(Exception):
    """Raised when a version hierarchy is too deep. The hierarchy of versions is given
    in property `versions`.
    """

    def __init__(self, versions: List[Version]) -> None:
        self.versions = versions

class JarNotFoundError(Exception):
    """Raised when no version's JAR file could be found from the metadata.
    """

class JvmNotFoundError(Exception):
    """Raised when the 
    """

    UNSUPPORTED_LIBC = "unsupported_libc"
    UNSUPPORTED_ARCH = "unsupported_arch"
    UNSUPPORTED_VERSION = "unsupported_version"

    def __init__(self, code: str) -> None:
        self.code = code


class VersionResolveEvent:
    __slots__ = "version_id", "done"
    def __init__(self, version_id: str, done: bool) -> None:
        self.version_id = version_id
        self.done = done

class JarFoundEvent:
    __slots__ = "version_id",
    def __init__(self, version_id: str) -> None:
        self.version_id = version_id

class AssetsResolveEvent:
    __slots__ = "index_version", "count"
    def __init__(self, index_version: str, count: Optional[int]) -> None:
        self.index_version = index_version
        self.count = count

class LibraryResolveEvent:
    __slots__ = "count",
    def __init__(self, count: Optional[int]) -> None:
        self.count = count

class LoggerFoundEvent:
    __slots__ = "version"
    def __init__(self, version: str) -> None:
        self.version = version

class JvmResolveEvent:
    __slots__ = "version", "count"
    def __init__(self, version: Optional[str], count: Optional[int]) -> None:
        self.version = version
        self.count = count


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
                raise TooMuchParentsError(versions)
            
            watcher.on_event(VersionResolveEvent(version_id, False))
            version = context.get_version(version_id)
            self.ensure_version_meta(version, manifest)
            watcher.on_event(VersionResolveEvent(version_id, True))

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

        try:
            version_super_meta = manifest.get_version(version.id)
        except HttpError:
            # Silently ignoring HTTP errors, we want to be able to launch offline.
            return True
        
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
        :raises VersionNotFoundError: In case of error finding the version.
        """

        version_super_meta = manifest.get_version(version.id)
        if version_super_meta is None:
            raise VersionNotFoundError(version)

        res = http_request("GET", version_super_meta["url"], accept="application/json")
        
        # First decode the data and set it to the version meta. Raising if invalid.
        version.metadata = res.json()
        
        # If successful, write the raw data directly to the file.
        version.dir.mkdir(parents=True, exist_ok=True)
        with version.metadata_file().open("wb") as fp:
            fp.write(res.data)


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
                    watcher.on_event(JarFoundEvent(version_meta.id))
                    return
            # If no download entry has been found, but the JAR exists, we use it.
            if jar_file.is_file():
                state.insert(VersionJar(version_meta.id, jar_file))
                watcher.on_event(JarFoundEvent(version_meta.id))
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
        
        watcher.on_event(AssetsResolveEvent(assets_index_version, None))

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
            
            assets_index = http_request("GET", assets_index_url, accept="application/json").json()
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
        
        virtual_dir = context.assets_dir.joinpath("virtual", assets_index_version) if assets_virtual else None
        resources_dir = context.work_dir / "resources" if assets_resources else None

        state.insert(VersionAssets(assets_index_version, assets, virtual_dir, resources_dir))
        watcher.on_event(AssetsResolveEvent(assets_index_version, len(assets)))


class AssetsFinalizeTask(Task):
    """This task finalize the installation of assets after being downloaded.
    """

    def execute(self, state: State, watcher: Watcher) -> None:
        
        assets = state[VersionAssets]

        if assets.resources_dir is not None:
            for asset_id, asset_file in assets.assets.items():
                dst_file = assets.resources_dir / asset_id
                dst_file.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(str(asset_file), str(dst_file))

        if assets.virtual_dir is not None:
            for asset_id, asset_file in assets.assets.items():
                dst_file = assets.virtual_dir / asset_id
                dst_file.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(str(asset_file), str(dst_file))


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
            
        watcher.on_event(LibraryResolveEvent(None))
        
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

            # TODO: Predicates??

            # Old metadata files provides a 'natives' mapping from OS to the classifier
            # specific for this OS.
            natives = library.get("natives")

            if natives is not None:

                if not isinstance(natives, dict):
                    raise ValueError(f"metadata: /libraries/{library_idx}/natives must be an object")
                
                # If natives object is present, the classifier associated to the
                # OS overrides the lib_spec classifier.
                spec.classifier = natives.get(minecraft_os)
                if spec.classifier is None:
                    continue

                if minecraft_arch_bits is not None:
                    spec.classifier = spec.classifier.replace("${arch}", minecraft_arch_bits)
                
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

        state.insert(VersionLibraries(class_libs, native_libs))
        watcher.on_event(LibraryResolveEvent(len(class_libs) + len(native_libs)))


class LoggerTask(Task):
    """Logger resolving task.

    This task resolves which logger configuration to use for the selected version.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out VersionLogging: Optional, the logging configuration if this version specifies it.
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
        watcher.on_event(LoggerFoundEvent(file_id.replace(".xml", "")))


class JvmTask(Task):
    """JVM resolving task, may not succeed on all platforms.

    This task resolves which official Mojang JVM should be used.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out VersionJvm: If JVM is found.
    """

    def execute(self, state: State, watcher: Watcher) -> None:

        # Don't do anything if JVM is already provided.
        if state.get(VersionJvm) is not None:
            return
        
        context = state[Context]
        metadata = state[FullMetadata].data
        dl = state[DownloadList]

        if platform.system() == "Linux" and platform.libc_ver()[0] != "glibc":
            raise JvmNotFoundError(JvmNotFoundError.UNSUPPORTED_LIBC)
        
        jvm_version_type = metadata.get("javaVersion", {}).get("component", "jre-legacy")
        if not isinstance(jvm_version_type, str):
            raise ValueError("metadata: /javaVersion/component must be a string")

        jvm_dir = context.jvm_dir / jvm_version_type
        jvm_manifest_file = context.jvm_dir / f"{jvm_version_type}.json"

        try:
            with jvm_manifest_file.open("rt") as jvm_manifest_fp:
                jvm_manifest = json.load(jvm_manifest_fp)
        except (OSError, JSONDecodeError):

            all_jvm_meta = http_request("GET", JVM_META_URL, accept="application/json").json()
            if not isinstance(all_jvm_meta, dict):
                raise ValueError("jvm metadata: / must be an object")
            
            jvm_arch_meta = all_jvm_meta.get(minecraft_jvm_os)
            if not isinstance(jvm_arch_meta, dict):
                raise JvmNotFoundError(JvmNotFoundError.UNSUPPORTED_ARCH)

            jvm_meta = jvm_arch_meta.get(jvm_version_type)
            if not isinstance(jvm_meta, list) or not len(jvm_meta):
                raise JvmNotFoundError(JvmNotFoundError.UNSUPPORTED_VERSION)

            jvm_meta_manifest = jvm_meta[0].get("manifest")
            if not isinstance(jvm_meta_manifest, dict):
                raise ValueError(f"jvm metadata: /{minecraft_jvm_os}/{jvm_version_type}/0/manifest must be an object")
            
            jvm_meta_manifest_url = jvm_meta_manifest.get("url")
            if not isinstance(jvm_meta_manifest_url, str):
                raise ValueError(f"jvm metadata: /{minecraft_jvm_os}/{jvm_version_type}/0/manifest/url must be a string")

            jvm_manifest = http_request("GET", jvm_meta_manifest_url, accept="application/json").json()

            if not isinstance(jvm_manifest, dict):
                raise ValueError("jvm manifest: / must be an object")

            jvm_manifest["version"] = jvm_meta[0].get("version", {}).get("name")

            jvm_manifest_file.parent.mkdir(parents=True, exist_ok=True)
            with jvm_manifest_file.open("wt") as jvm_manifest_fp:
                json.dump(jvm_manifest, jvm_manifest_fp)
        
        jvm_exec = jvm_dir.joinpath("bin", jvm_bin_filename)
        jvm_version = jvm_manifest.get("version")

        watcher.on_event(JvmResolveEvent(jvm_version, None))

        jvm_files = jvm_manifest.get("files")
        if not isinstance(jvm_files, dict):
            raise ValueError("jvm manifest: /files must be an object")

        for jvm_file_path_prefix, jvm_file in jvm_files.items():
            if jvm_file.get("type") == "file":

                jvm_file_path = jvm_dir / jvm_file_path_prefix
                jvm_download_raw = jvm_file.get("downloads", {}).get("raw")
                jvm_download_entry = parse_download_entry(jvm_download_raw, jvm_file_path, f"jvm manifest: /files/{jvm_file_path_prefix}/downloads/raw")
                jvm_download_entry.executable = jvm_file.get("executable", False)

                dl.add(jvm_download_entry, verify=True)
        
        state.insert(VersionJvm(jvm_exec, jvm_version))
        watcher.on_event(JvmResolveEvent(jvm_version, len(jvm_files)))


class LwjglFixTask(Task):
    """This special task can be optionally used to 
    """


class ArgsTask(Task):
    """This task compute the `VersionArgs` 
    """

    def execute(self, state: State, watcher: Watcher) -> None:
        
        context = state[Context]
        version = state[Version]
        metadata = state[FullMetadata].data
        jar = state[VersionJar]
        libraries = state[VersionLibraries]
        assets = state[VersionAssets]
        logging = state.get(VersionLogging)
        jvm = state[VersionJvm]
        opts = state.get(VersionArgsOptions) or VersionArgsOptions()

        # Main class
        main_class = metadata.get("mainClass")
        if not isinstance(main_class, str):
            raise ValueError("metadata: /mainClass must be a string")
        
        # Features
        features = {
            "is_demo_user": opts.demo,
            "has_custom_resolution": opts.resolution is not None,
            **opts.features
        }

        # Get authentication of create a random offline.
        auth_session = opts.auth_session or OfflineAuthSession(None, None)
        uuid = auth_session.uuid
        username = auth_session.username

        # Arguments replacements
        args_replacements: Dict[str, str] = {
            # Game
            "auth_player_name": username,
            "version_name": version.id,
            "library_directory": str(context.libraries_dir),
            "game_directory": str(context.work_dir),
            "assets_root": str(context.assets_dir),
            "assets_index_name": assets.index_version,
            "auth_uuid": uuid,
            "auth_access_token": auth_session.format_token_argument(False),
            "auth_xuid": auth_session.get_xuid(),
            "clientid": auth_session.client_id,
            "user_type": auth_session.user_type,
            "version_type": metadata.get("type", ""),
            # Game (legacy)
            "auth_session": auth_session.format_token_argument(True),
            "game_assets": str(assets.virtual_dir or ""),
            "user_properties": "{}",
            # JVM
            "natives_directory": "",
            "launcher_name": LAUNCHER_NAME,
            "launcher_version": LAUNCHER_VERSION,
            "classpath_separator": os.pathsep,
            "classpath": os.pathsep.join(map(str, libraries.class_libs))
        }

        if opts.resolution is not None:
            args_replacements["resolution_width"] = str(opts.resolution[0])
            args_replacements["resolution_height"] = str(opts.resolution[1])
        
        # Arguments
        modern_args = metadata.get("arguments", {})
        modern_jvm_args = modern_args.get("jvm")
        modern_game_args = modern_args.get("game")

        jvm_args = [str(jvm.executable_file)]
        game_args = []

        # Interpret JVM arguments.
        interpret_args(legacy_jvm_args if modern_jvm_args is None else modern_jvm_args, features, jvm_args)
        
        # JVM argument for logging config
        if logging is not None:
            jvm_args.append(logging.arg.replace("${path}", str(logging.path)))

        # JVM argument for launch wrapper JAR path
        if main_class == "net.minecraft.launchwrapper.Launch":
            jvm_args.append(f"-Dminecraft.client.jar={str(jar.path)}")

        # Add old fix JVM args
        if opts.fix_legacy:
            version_split = version.id.split(".", 2)
            if version_split[0].endswith("b1") or version_split[0].endswith("a1"):
                jvm_args.append("-Djava.util.Arrays.useLegacyMergeSort=true")
                jvm_args.append("-Dhttp.proxyHost=betacraft.pl")
            elif len(version_split) >= 2 and len(version_split[1]) == 1 and ord("0") <= ord(version_split[1][0]) <= ord("5"):
                jvm_args.append("-Dhttp.proxyHost=betacraft.pl")
        
        # Game arguments
        if modern_game_args is None:
            # If no modern arguments were found, we try finding legacy game arguments.
            game_args.extend(metadata.get("minecraftArguments", "").split(" "))
            # If the resolution is set but we are using legacy arguments, we
            # manually add resolution arguments to the game args.
            if opts.resolution is not None:
                game_args.extend((
                    "--width", str(opts.resolution[0]),
                    "--height", str(opts.resolution[1]),
                ))
        else:
            interpret_args(modern_game_args, features, game_args)

        # Global options.        
        if opts.disable_multiplayer:
            game_args.append("--disableMultiplayer")
        if opts.disable_chat:
            game_args.append("--disableChat")
        if opts.server_address is not None:
            game_args.extend(("--server", opts.server_address))
        if opts.server_port is not None:
            game_args.extend(("--port", str(opts.server_port)))

        state.insert(VersionArgs(jvm_args, game_args, main_class, args_replacements))


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
    if os_name is None or os_name == minecraft_os:
        os_arch = rule_os.get("arch")
        if os_arch is None or os_arch == minecraft_arch:
            os_version = rule_os.get("version")
            if os_version is None or re.search(os_version, platform.version()) is not None:
                return True
    return False


def interpret_args(args: List[Union[str, dict]], features: Dict[str, bool], dst: List[str]) -> None:
    for arg in args:
        if isinstance(arg, str):
            dst.append(arg)
        else:
            rules = arg.get("rules")
            if rules is not None:
                if not interpret_rule(rules, features):
                    continue
            arg_value = arg["value"]
            if isinstance(arg_value, list):
                dst.extend(arg_value)
            elif isinstance(arg_value, str):
                dst.append(arg_value)


# Name of the OS has used by Minecraft.
minecraft_os = {
    "Linux": "linux", 
    "Windows": "windows", 
    "Darwin": "osx",
    "FreeBSD": "freebsd"
}.get(platform.system())

# Name of the processor's architecture has used by Minecraft.
minecraft_arch = {
    "i386": "x86",
    "i686": "x86",
    "x86_64": "x86_64",
    "amd64": "x86_64",
    "arm64": "arm64",
    "aarch64": "arm64",
    "armv7l": "arm32",
    "armv6l": "arm32",
}.get(platform.machine().lower())

# Stores the bits length of pointers on the current system.
minecraft_arch_bits = {
    "64bit": "64",
    "32bit": "32"
}.get(platform.architecture()[0])

# Name of the OS has used by Mojang for officially distributed JVMs.
minecraft_jvm_os = None if minecraft_arch is None else {
    "Darwin": {"x86_64": "mac-os", "arm64": "mac-os-arm64"},
    "Linux": {"x86": "linux-i386", "x86_64": "linux"},
    "Windows": {"x86": "windows-x86", "x86_64": "windows-x64"}
}.get(platform.system(), {}).get(minecraft_arch)

# JVM arguments used if no arguments are specified.
legacy_jvm_args = [
    {
        "rules": [{"action": "allow", "os": {"name": "osx"}}],
        "value": ["-XstartOnFirstThread"]
    },
    {
        "rules": [{"action": "allow", "os": {"name": "windows"}}],
        "value": "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"
    },
    {
        "rules": [{"action": "allow", "os": {"name": "windows", "version": "^10\\."}}],
        "value": ["-Dos.name=Windows 10", "-Dos.version=10.0"]
    },
    "-Djava.library.path=${natives_directory}",
    "-Dminecraft.launcher.brand=${launcher_name}",
    "-Dminecraft.launcher.version=${launcher_version}",
    "-cp",
    "${classpath}"
]



def make_vanilla_sequence(version_id: str, *, 
    context: Optional[Context] = None,
    version_manifest: Optional[VersionManifest] = None,
) -> Sequence:
    """Make vanilla sequence for installing and running vanilla Minecraft versions.
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

    seq.append_task(JvmTask())

    seq.append_task(DownloadTask())
    seq.append_task(AssetsFinalizeTask())

    seq.append_task(ArgsTask())

    return seq
