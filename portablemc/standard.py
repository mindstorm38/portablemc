"""Definition of standard version supporting the standard metadata format used by Mojang. 
This module also provide Mojang's version manifest which can be used as default version
repository, allowing resolution of what we call "vanilla" versions.
"""

from subprocess import Popen, TimeoutExpired, PIPE, STDOUT
from json import JSONDecodeError
from pathlib import Path
from uuid import uuid4
import platform
import shutil
import json
import re
import os

from .download import DownloadList, DownloadEntry, DownloadResultProgress, DownloadResultError
from .util import jvm_bin_filename, merge_dict, LibrarySpecifier
from .auth import AuthSession, OfflineAuthSession
from .http import http_request
from .task import Watcher
from . import LAUNCHER_NAME, LAUNCHER_VERSION

from typing import Optional, Iterator, Dict, List, Tuple, Any, Callable, Set


RESOURCES_URL = "https://resources.download.minecraft.net/"
LIBRARIES_URL = "https://libraries.minecraft.net/"
JVM_META_URL = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json"
VERSION_MANIFEST_URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

FIX_LEGACY_PROXY = object()
FIX_LEGACY_MERGE_SORT = object()
FIX_LEGACY_RESOLUTION = object()
FIX_LEGACY_QUICK_PLAY = object()


class Context:
    """Context of the game's installation and runtime. This defines various directories
    where versions, assets, libraries or JVM are stored, as well as a bin directory for
    temporary runtime files, and also a working directory from where the game will run.
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

    def get_version(self, version: str) -> "VersionHandle":
        """Get a version's handle.
        """
        return VersionHandle(version, self.versions_dir / version)

    def list_versions(self) -> "Iterator[VersionHandle]":
        """List installed versions given their handles.
        """
        if self.versions_dir.is_dir():
            for version_dir in self.versions_dir.iterdir():
                if version_dir.is_dir():
                    version = VersionHandle(version_dir.name, version_dir)
                    if version.metadata_exists():
                        yield version
    
    def gen_bin_dir(self) -> Path:
        """Generate a random named binary directory, may be used for any kind of temporary
        files and data. Usually for shared libraries used by the game. Note that this 
        directory isn't created by this method, only its path is returned.
        """
        return self.bin_dir / str(uuid4())


class VersionHandle:
    """This class holds a version handle that allows modifying its metadata, reading and
    writing it. The parents of this version can also be linked and then merged together
    to get a full metadata.

    This class cannot really be used as a complete version to prepare tasks on, this is
    rather a handle that defines its hierarchical chain of versions.
    """

    __slots__ = "id", "dir", "metadata", "parent"

    def __init__(self, id: str, dir: Path) -> None:
        self.id = id
        self.dir = dir
        self.metadata = {}
        self.parent: Optional[VersionHandle] = None
    
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
    
    def recurse(self) -> "Iterator[VersionHandle]":
        """Walk through every version metadata in the hierarchy of the current one.
        """
        version_meta = self
        while version_meta is not None:
            yield version_meta
            version_meta = version_meta.parent
    
    def merge(self) -> dict:
        """Merge this version metadata and all of its parents into a `FullMetadata`.
        """
        result = {}
        for version_meta in self.recurse():
            merge_dict(result, version_meta.metadata)
        return result


class Version:
    """Base class for basic version resolving, it handles metadata parsing and resources
    resolution. Note that this base class doesn't support vanilla version provided by
    Mojang by default.
    """
    
    def __init__(self, context: Context, root_version: str) -> None:
        """Construct a standard version installer and runner.

        :param context: The installation context of the game, used to know where to find
        its metadata and where to install resources.
        :param root_version: The root version to resolve at first, all of its parents will
        be loaded and then all the metadata will be merged together. This merged metadata
        will be used by this class to install its resources such as libraries and assets.
        :param version_max_parents: Optional parameter that specifies the hard limit of
        parents count when resolving the parents of the root version.
        """

        # Entry attributes
        self.context = context
        self.root_version = root_version
        
        # General options
        self.demo: bool = False
        self.auth_session: Optional[AuthSession] = None
        self.resolution: Optional[Tuple[int, int]] = None
        self.disable_multiplayer: bool = False
        self.disable_chat: bool = False
        self.quick_play: Optional[QuickPlay] = None
        self.fixes = { 
            FIX_LEGACY_PROXY, 
            FIX_LEGACY_MERGE_SORT,
            FIX_LEGACY_RESOLUTION,
            FIX_LEGACY_QUICK_PLAY,
        }

        # Resolved version metadata, root version and its hierarchy, and merged metadata
        self._version: Optional[VersionHandle] = None
        self._versions: List[VersionHandle] = []
        self._metadata: dict = {}

        # Features used for rule resolution across the metadata
        self._features: Dict[str, bool] = {}

        # Path to the JAR file to run the game
        self._jar_path: Optional[Path] = None

        # Assets dictionary and index version, with optional legacy directories
        self._assets_index_version: Optional[str] = None
        self._assets: Dict[str, Path] = {}
        self._assets_virtual_dir: Optional[Path] = None
        self._assets_resources_dir: Optional[Path] = None

        # Native and class libraries
        self._native_libs: List[Path] = []
        self._class_libs: List[Path] = []
        self._libs_predicates: List[Callable[[LibrarySpecifier], bool]] = []
        self._libs_version_fixes: Dict[LibrarySpecifier, str] = {
            LibrarySpecifier("com.mojang", "authlib", "2.1.28"): "2.2.30"
        }

        self._logger_path: Optional[Path] = None
        self._logger_arg: Optional[str] = None
        
        self._jvm_path: Optional[Path] = None
        self._jvm_version: Optional[str] = None

        self._dl = DownloadList()

        self._jvm_args: List[str] = []
        self._game_args: List[str] = []
        self._main_class: Optional[str] = None
        self._args_replacements: Dict[str, str] = {}

    def set_auth_offline(self, username: Optional[str], uuid: Optional[str]) -> None:
        """Shortcut for setting an offline session with the given username/uuid pair.
        """
        self.auth_session = OfflineAuthSession(uuid, username)

    def set_quick_play_singleplayer(self, level_name: str) -> None:
        """Configure quick play for entering a singleplayer level after game's launch.
        """
        self.quick_play = QuickPlaySingleplayer(level_name)
    
    def set_quick_play_multiplayer(self, host: str, port: int = 25565) -> None:
        """Configure quick play for connecting to a server after game's launch.
        """
        self.quick_play = QuickPlayMultiplayer(host, port)
    
    def set_quick_play_realms(self, realm: str) -> None:
        """Configure quick play for connection to a realm after game's launch.
        """
        self.quick_play = QuickPlayRealms(realm)

    def install(self, *, watcher: Optional[Watcher] = None) -> None:
        """This function ensures that this version is properly installed. You may give
        a watcher for listening at all the steps being executed to produce the binary.
        This function also ensures that 

        This function may produce a wide range of errors when resolving, and will also
        provides events to the given watcher. When an error happens the internal state
        is not guaranteed, but should be possible to fix after fixing the reported 
        problem and running this function again.
        """

        watcher = watcher or Watcher()

        self._resolve_metadata(watcher)
        self._resolve_features(watcher)
        self._resolve_jar(watcher)
        self._resolve_assets(watcher)
        self._resolve_libraries(watcher)
        self._resolve_logger(watcher)
        self._resolve_jvm(watcher)
        self._download(watcher)
        self._finalize_assets(watcher)

    def _resolve_metadata(self, watcher: Watcher) -> None:
        """This step resolves metadata of the root version and all of its parents.
        """

        versions = self._versions
        version: Optional[str] = self.root_version
        versions.clear()

        while version is not None:

            if len(versions) > 10:
                raise TooMuchParentsError(versions)
            
            watcher.handle(VersionLoadingEvent(version))

            # Get version instance and load/fetch is needed.
            handle = self.context.get_version(version)
            if not self._load_version(handle, watcher):
                watcher.handle(VersionLoadingEvent(version))
                self._fetch_version(handle, watcher)
            
            watcher.handle(VersionLoadedEvent(version))

            # Set the parent of the last version to the version being resolved.
            if len(versions):
                versions[-1].parent = handle
            
            versions.append(handle)
            version_id = handle.metadata.pop("inheritsFrom", None)
            
            if version_id is not None and not isinstance(version_id, str):
                raise ValueError("metadata: /inheritsFrom must be a string")

        self._version = versions[0]
        self._metadata = self._version.merge()

    def _load_version(self, version: VersionHandle, watcher: Watcher) -> bool:
        """This function is responsible for loading a version's metadata. Note that 
        implementations are free to load other things beside metadata.

        This function returns true if loading was successful, if this function returns
        false, the `fetch_version` function is then called to fetch the version. This can
        be used to check integrity of a version.

        De default implementation of this function just read metadata file and return 
        true if successful.

        :param version: The version metadata to validate or not.
        :param state: Sequence state when the MetadataTask execute.
        :return: True if the given version is valid and its metadata was properly loaded.
        """
        return version.read_metadata_file()

    def _fetch_version(self, version: VersionHandle, watcher: Watcher) -> None:
        """Fetch the data of the given version.

        The default implementation just raise not found with the version's id.

        :param version: The version meta to fetch data into.
        :param state: Sequence state when the MetadataTask execute.
        :raises VersionNotFoundError: In case of error finding the version.
        """
        raise VersionNotFoundError(version.id)

    def _resolve_features(self, watcher: Watcher) -> None:
        """Step resolving the version's features, whose are a mapping of string to 
        boolean, indicating if such feature is enabled or not. This is later used 
        to compute rules when needed.
        """

        self._features["is_demo_user"] = self.demo
        self._features["has_custom_resolution"] = self.resolution is not None
        if self.quick_play is not None:
            self._features[self.quick_play.feature] = True

    def _resolve_jar(self, watcher: Watcher) -> None:
        """This step resolves the JAR file to use for launcher the game.
        """

        assert self._version is not None

        self._jar_path = self._version.jar_file()

        # First try to find a /downloads/client download entry.
        version_dls = self._metadata.get("downloads")
        if version_dls is not None:

            if not isinstance(version_dls, dict):
                raise ValueError("metadata: /downloads must be an object")
            
            client_dl = version_dls.get("client")
            if client_dl is not None:
                self._dl.add(parse_download_entry(client_dl, self._jar_path, "metadata: /downloads/client"), verify=True)
                watcher.handle(JarFoundEvent())
                return
        
        # If no download entry has been found, but the JAR exists, we use it.
        if self._jar_path.is_file():
            watcher.handle(JarFoundEvent())
            return
        
        self._jar_path = None
        raise JarNotFoundError()

    def _resolve_assets(self, watcher: Watcher) -> None:
        """This step resolve assets from metadata and add missing entries to the download
        list for future download.
        """

        metadata = self._metadata
        context = self.context

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
        
        watcher.handle(AssetsResolveEvent(assets_index_version, None))

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

        self._assets.clear()
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
            self._assets[asset_id] = asset_file
            if not asset_file.is_file() or asset_file.stat().st_size != asset_size:
                asset_url = f"{RESOURCES_URL}{asset_hash_prefix}/{asset_hash}"
                self._dl.add(DownloadEntry(asset_url, asset_file, size=asset_size, sha1=asset_hash, name=asset_id))
        
        self._assets_index_version = assets_index_version
        self._assets_virtual_dir = context.assets_dir.joinpath("virtual", assets_index_version) if assets_virtual else None
        self._assets_resources_dir = context.work_dir / "resources" if assets_resources else None

        watcher.handle(AssetsResolveEvent(assets_index_version, len(self._assets)))

    def _finalize_assets(self, watcher: Watcher) -> None:
        """Step called after download to finalize installation of assets when 
        """

        if self._assets_resources_dir is not None:
            for asset_id, asset_file in self._assets.items():
                dst_file = self._assets_resources_dir / asset_id
                dst_file.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(str(asset_file), str(dst_file))

        if self._assets_virtual_dir is not None:
            for asset_id, asset_file in self._assets.items():
                dst_file = self._assets_virtual_dir / asset_id
                dst_file.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(str(asset_file), str(dst_file))

    def _resolve_libraries(self, watcher: Watcher) -> None:
        """Step resolving 
        """

        assert self._version is not None

        excluded_libs = []
        watcher.handle(LibrariesResolvingEvent())

        # Recursion order is important for libraries resolving, root libraries should
        # be placed first.
        for version in self._version.recurse():

            metadata_libraries = version.metadata.get("libraries")
            if metadata_libraries is None:
                continue

            if not isinstance(metadata_libraries, list):
                raise ValueError("metadata: /libraries must be a list")

            for library_idx, library in enumerate(metadata_libraries):

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
                    
                    if not interpret_rule(rules, self._features, f"metadata: /libraries/{library_idx}/rules"):
                        continue

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
                        spec.classifier = spec.classifier.replace("${arch}", str(minecraft_arch_bits))
                    
                    libs = self._native_libs

                else:
                    libs = self._class_libs
                
                # Apply predicates after the final classifier has been set, if relevant.
                if not all((pred(spec) for pred in self._libs_predicates)):
                    excluded_libs.append(spec)
                    continue

                # Check version fixes.
                version_fix = self._libs_version_fixes.get(spec)
                if version_fix is not None:
                    spec.version = version_fix

                dl_entry: Optional[DownloadEntry] = None
                jar_path_rel = spec.file_path()
                jar_path = self.context.libraries_dir / jar_path_rel
                
                # Avoids ready downloading if a fix is being used, in such case we'll use
                # the Mojang's libraries. TODO: Improve this fix system, it's limiting.
                downloads = library.get("downloads")
                if downloads is not None and version_fix is None:

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

                # If no download entry can be found, add a default one that points to 
                # official library repository, this may not work.
                # TODO: Maybe avoid trying this if the jar file already exists.
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

                # If URL is empty, just ignore the entry.
                if dl_entry is not None and len(dl_entry.url):
                    dl_entry.name = str(spec)
                    self._dl.add(dl_entry, verify=True)

        watcher.handle(LibrariesResolvedEvent(len(self._class_libs), len(self._native_libs), excluded_libs))

    def _resolve_logger(self, watcher: Watcher) -> None:
        """This step resolve the logger to use for launcher the game.
        """

        logging = self._metadata.get("logging")
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

        self._logger_arg = argument
        self._logger_path = self.context.assets_dir / "log_configs" / file_id
        dl_entry = parse_download_entry(file_info, self._logger_path, "metadata: /logging/client/file")
        self._dl.add(dl_entry, verify=True)

        watcher.handle(LoggerFoundEvent(file_id.replace(".xml", "")))

    def _resolve_jvm(self, watcher: Watcher) -> None:
        """Step resolving a JVM suitable for running the game.
        """

        # Don't do anything if JVM is already provided.
        if self._jvm_path is not None:
            return
        
        watcher.handle(JvmLoadingEvent())
        
        jvm_version_info = self._metadata.get("javaVersion", {})
        if not isinstance(jvm_version_info, dict):
            raise ValueError("metadata: /javaVersion must be a string")

        jvm_major_version = jvm_version_info.get("majorVersion")
        if jvm_major_version is not None and not isinstance(jvm_major_version, int):
            raise ValueError("metadata: /javaVersion/majorVersion must be an integer")

        if platform.system() == "Linux" and platform.libc_ver()[0] != "glibc":
            return self._resolve_builtin_jvm(watcher, JvmNotFoundError.UNSUPPORTED_LIBC, jvm_major_version)

        jvm_version_type = jvm_version_info.get("component", "jre-legacy")
        if not isinstance(jvm_version_type, str):
            raise ValueError("metadata: /javaVersion/component must be a string")

        jvm_dir = self.context.jvm_dir / jvm_version_type
        jvm_manifest_file = self.context.jvm_dir / f"{jvm_version_type}.json"

        try:
            with jvm_manifest_file.open("rt") as jvm_manifest_fp:
                jvm_manifest = json.load(jvm_manifest_fp)
        except (OSError, JSONDecodeError):

            all_jvm_meta = http_request("GET", JVM_META_URL, accept="application/json").json()
            if not isinstance(all_jvm_meta, dict):
                raise ValueError("jvm metadata: / must be an object")
            
            jvm_arch_meta = all_jvm_meta.get(minecraft_jvm_os)
            if not isinstance(jvm_arch_meta, dict):
                return self._resolve_builtin_jvm(watcher, JvmNotFoundError.UNSUPPORTED_ARCH, jvm_major_version)

            jvm_meta = jvm_arch_meta.get(jvm_version_type)
            if not isinstance(jvm_meta, list) or not len(jvm_meta):
                return self._resolve_builtin_jvm(watcher, JvmNotFoundError.UNSUPPORTED_VERSION, jvm_major_version)

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
        
        self._jvm_path = jvm_dir.joinpath("bin", jvm_bin_filename)
        self._jvm_version = jvm_manifest.get("version")

        jvm_files = jvm_manifest.get("files")
        if not isinstance(jvm_files, dict):
            raise ValueError("jvm manifest: /files must be an object")

        for jvm_file_path_prefix, jvm_file in jvm_files.items():
            if jvm_file.get("type") == "file":

                jvm_file_path = jvm_dir / jvm_file_path_prefix
                jvm_download_raw = jvm_file.get("downloads", {}).get("raw")
                jvm_download_entry = parse_download_entry(jvm_download_raw, jvm_file_path, f"jvm manifest: /files/{jvm_file_path_prefix}/downloads/raw")
                jvm_download_entry.executable = jvm_file.get("executable", False)

                self._dl.add(jvm_download_entry, verify=True)
        
        watcher.handle(JvmLoadedEvent(self._jvm_version, len(jvm_files)))

    def _resolve_builtin_jvm(self, watcher: Watcher, reason: str, major_version: Optional[int]) -> None:
        """Internal function to find the builtin Java executable, the reason why this is
        needed is given in parameter. The expected major version is also given, it should
        not be none because we cannot check builtin version.
        """

        if major_version is None:
            raise JvmNotFoundError(reason)

        builtin_path = shutil.which(jvm_bin_filename)
        if builtin_path is None:
            raise JvmNotFoundError(reason)
        
        try:
            
            # Get version of the JVM.
            process = Popen([builtin_path, "-version"], bufsize=1, stdout=PIPE, stderr=STDOUT, universal_newlines=True)
            stdout, _stderr = process.communicate(timeout=1)

            version_start = stdout.index(f"1.{major_version}" if major_version <= 8 else str(major_version))
            version = None
            
            # Parse version by getting all character that are numeric or '.'.
            for i, ch in enumerate(stdout[version_start:]):
                if not ch.isnumeric() and ch not in (".", "_"):
                    version = stdout[version_start:i]
                    break
            
            if version is None:
                raise ValueError()

        except (TimeoutExpired, ValueError):
            raise JvmNotFoundError(JvmNotFoundError.BUILTIN_INVALID_VERSION)

        self._jvm_path = Path(builtin_path)
        self._jvm_version = version
        watcher.handle(JvmLoadedEvent(version, None))

    def _download(self, watcher: Watcher) -> None:
        
        entries_count = len(self._dl.entries)
        if not entries_count:
            return
        
        # Note: do not create more thread than available entries.
        threads_count = min(entries_count, (os.cpu_count() or 1) * 4)
        errors = []

        watcher.handle(DownloadStartEvent(threads_count, entries_count, self._dl.size))

        for result_count, result in self._dl.download(threads_count):
            if isinstance(result, DownloadResultProgress):
                watcher.handle(DownloadProgressEvent(
                    result.thread_id,
                    result_count,
                    result.entry,
                    result.size,
                    result.speed
                ))
            elif isinstance(result, DownloadResultError):
                errors.append((result.entry, result.code))

        # If errors are present, raise an error.
        if len(errors):
            raise DownloadError(errors)
        
        # Clear entries if successful, therefore multiple calls can be chained if
        # needed, without re-downloading the same files.
        self._dl.clear()
        
        watcher.handle(DownloadCompleteEvent())

    def _resolve_args(self, watcher: Watcher) -> None:
        """Step for computing correct arguments to run the game as configured in this 
        class. This should be called after every other step in order to take everything
        into account.
        """

        assert self._version is not None
        assert self._assets_index_version is not None

        # Main class
        main_class = self._metadata.get("mainClass")
        if not isinstance(main_class, str):
            raise ValueError("metadata: /mainClass must be a string")

        # List of fixes for summary...
        fixes = []

        # Get authentication of create a random offline.
        auth_session = self.auth_session or OfflineAuthSession(None, None)

        # Class path, without main class (added later depending on arguments present).
        class_path = list(map(str, self._class_libs))
        
        # Arguments
        self._main_class = main_class
        self._jvm_args = jvm_args = [str(self._jvm_path)]
        self._game_args = game_args = []
        all_features = set()

        # Check if modern arguments are present (> 1.12.2).
        modern_args = self._metadata.get("arguments")
        if modern_args is not None:

            if not isinstance(modern_args, dict):
                raise ValueError("metadata: /arguments must be an object")
        
            # Interpret JVM arguments.
            modern_jvm_args = modern_args.get("jvm", [])
            interpret_args(modern_jvm_args, self._features, jvm_args, "metadata: /arguments/jvm", all_features=all_features)
            
            # Interpret Game arguments.
            modern_game_args = modern_args.get("game", [])
            interpret_args(modern_game_args, self._features, game_args, "metadata: /arguments/game", all_features=all_features)
        
        else:

            interpret_args(legacy_jvm_args, self._features, jvm_args, f"<legacy_jvm_args>", all_features=all_features)

            # Append legacy game arguments, if available.
            legacy_game_args = self._metadata.get("minecraftArguments")
            if legacy_game_args is not None:
                if not isinstance(legacy_game_args, str):
                    raise ValueError("metadata: /minecraftArguments must be a string")
                game_args.extend(legacy_game_args.split(" "))

        # JVM argument for logging config
        if self._logger_path is not None and self._logger_arg is not None:
            jvm_args.append(self._logger_arg.replace("${path}", str(self._logger_path)))

        # JVM argument for launch wrapper JAR path
        if main_class == "net.minecraft.launchwrapper.Launch":
            jvm_args.append(f"-Dminecraft.client.jar={self._jar_path}")

        # If no modern arguments, fix some arguments.
        if modern_args is None:
            # Old versions seems to prefer having the main class first in class path.
            # This fix cannot be disabled for now (fixme?).
            class_path.insert(0, str(self._jar_path))
            fixes.append(ArgsFixesEvent.MAIN_CLASS_FIRST)
        else:
            # Modern versions seems to prefer having the main class last in class path.
            class_path.append(str(self._jar_path))
   
        # Apply some fixes for legacy versions.
        if len(self.fixes):

            # Get the last version in the parent's tree, we use it to apply legacy fixes.
            ancestor_id = list(self._version.recurse())[-1].id

            # Legacy proxy aims to fix things like skins on old versions.
            # This is applicable to all alpha/beta and 1.0:1.5
            if FIX_LEGACY_PROXY in self.fixes:

                proxy_port = None
                if ancestor_id.startswith("a1.0."):
                    proxy_port = 80
                elif ancestor_id.startswith("a1.1."):
                    proxy_port = 11702
                elif ancestor_id.startswith(("a1.", "b1.")):
                    proxy_port = 11705
                elif ancestor_id in ("1.0", "1.1", "1.3", "1.4", "1.5") or \
                    ancestor_id.startswith(("1.2.", "1.3.", "1.4.", "1.5.")):
                    proxy_port = 11707
                
                if proxy_port is not None:
                    fixes.append(ArgsFixesEvent.LEGACY_PROXY)
                    jvm_args.append("-Dhttp.proxyHost=betacraft.uk")
                    jvm_args.append(f"-Dhttp.proxyPort={proxy_port}")
            
            # Legacy merge sort is applicable to alpha and beta versions.
            if FIX_LEGACY_MERGE_SORT in self.fixes and ancestor_id.startswith(("a1.", "b1.")):
                fixes.append(ArgsFixesEvent.LEGACY_MERGE_SORT)
                jvm_args.append("-Djava.util.Arrays.useLegacyMergeSort=true")

            # The arguments do not support custom resolution, try to fix.
            if self.resolution is not None and "has_custom_resolution" not in all_features:
                if FIX_LEGACY_RESOLUTION in self.fixes:
                    fixes.append(ArgsFixesEvent.LEGACY_RESOLUTION)
                    game_args.extend((
                        "--width", str(self.resolution[0]),
                        "--height", str(self.resolution[1]),
                    ))
            
            # The arguments do not support quick play.
            if isinstance(self.quick_play, QuickPlayMultiplayer) and "is_quick_play_multiplayer" not in all_features:
                if FIX_LEGACY_QUICK_PLAY in self.fixes:
                    # TODO: fixes.append(...)
                    game_args.extend(("--server", self.quick_play.host))
                    game_args.extend(("--port", str(self.quick_play.port)))

        # Global options.        
        if self.disable_multiplayer:
            game_args.append("--disableMultiplayer")
        if self.disable_chat:
            game_args.append("--disableChat")

        # Arguments replacements
        self._args_replacements: Dict[str, str] = {
            # Game
            "auth_player_name": auth_session.username,
            "version_name": self._version.id,
            "library_directory": str(self.context.libraries_dir),
            "game_directory": str(self.context.work_dir),
            "assets_root": str(self.context.assets_dir),
            "assets_index_name": self._assets_index_version,
            "auth_uuid": auth_session.uuid,
            "auth_access_token": auth_session.format_token_argument(False),
            "auth_xuid": auth_session.get_xuid(),
            "clientid": auth_session.client_id,
            "user_type": auth_session.user_type,
            "version_type": self._metadata.get("type", ""),
            # Game (legacy)
            "auth_session": auth_session.format_token_argument(True),
            "game_assets": str(self._assets_virtual_dir or ""),
            "user_properties": "{}",
            # JVM
            "natives_directory": "",
            "launcher_name": LAUNCHER_NAME,
            "launcher_version": LAUNCHER_VERSION,
            "classpath_separator": os.pathsep,
            "classpath": os.pathsep.join(class_path)
        }

        if self.quick_play is not None and self.quick_play.feature in all_features:
            self.quick_play.add_args_replacements(self._args_replacements)

        if self.resolution is not None:
            self._args_replacements["resolution_width"] = str(self.resolution[0])
            self._args_replacements["resolution_height"] = str(self.resolution[1])


class QuickPlay:
    """Base class for quick play launch methods for the game.
    Note that these quick play types may not be supported by the game.
    """

    feature: str

    def add_args_replacements(self, args_replacements: Dict[str, str]) -> None:
        raise NotImplementedError

class QuickPlaySingleplayer(QuickPlay):
    """Quick play mode to launch a singleplayer level given its name.
    """

    feature = "is_quick_play_singleplayer"

    def __init__(self, level_name: str) -> None:
        self.level_name = level_name

    def add_args_replacements(self, args_replacements: Dict[str, str]) -> None:
        args_replacements["quickPlaySingleplayer"] = self.level_name

class QuickPlayMultiplayer(QuickPlay):
    """Quick play mode to automatically connect to a given server when launching game.
    """

    feature = "is_quick_play_multiplayer"

    def __init__(self, host: str, port: int = 25565) -> None:
        self.host = host
        self.port = port

    def add_args_replacements(self, args_replacements: Dict[str, str]) -> None:
        args_replacements["quickPlayMultiplayer"] = f"{self.host}:{self.port}"

class QuickPlayRealms(QuickPlay):
    """Quick play mode to automatically connection to a given realm when launching game.
    """

    feature = "is_quick_play_realms"

    def __init__(self, realm: str) -> None:
        self.realm = realm
    
    def add_args_replacements(self, args_replacements: Dict[str, str]) -> None:
        args_replacements["quickPlayRealms"] = self.realm


class Args:
    """Indicates java arguments, game arguments and main class required to run the game.
    This can later be used to run the game and is usually prepared by the `ArgsTask`.
    """

    __slots__ = "jvm_args", "game_args", "main_class", "args_replacements"

    def __init__(self, jvm_args: List[str], game_args: List[str], main_class: str, args_replacements: Dict[str, str]) -> None:
        self.jvm_args = jvm_args
        self.game_args = game_args
        self.main_class = main_class
        self.args_replacements = args_replacements
    
    def username(self) -> Optional[str]:
        """Retrieve the authenticated username from `args_replacements` (if provided).
        """
        return self.args_replacements.get("auth_player_name")

    def uuid(self) -> Optional[str]:
        """Retrieve the authenticated UUID from `args_replacements` (if provided).
        """
        return self.args_replacements.get("auth_uuid")

    def full_args(self) -> List[str]:
        """Compute the full arguments list, starting with the JVM executable, followed by
        JVM arguments, the main class name and then the game arguments. All arguments are
        formatted using `args_replacements` mapping.
        """
        return [
            *replace_list_vars(self.jvm_args, self.args_replacements),
            self.main_class,
            *replace_list_vars(self.game_args, self.args_replacements)
        ]


class VersionNotFoundError(Exception):
    """Raised when a version was not found. The version that was not found is given.
    """
    def __init__(self, version: str) -> None:
        self.version = version

class TooMuchParentsError(Exception):
    """Raised when a version hierarchy is too deep. The hierarchy of versions is given
    in property `versions`.
    """
    def __init__(self, versions: List[VersionHandle]) -> None:
        self.versions = versions

class JarNotFoundError(Exception):
    """Raised when no version's JAR file could be found from the metadata.
    """

class JvmNotFoundError(Exception):
    """Raised if no JVM can be found, the particular reason is given as code. This error
    is raised only if no builtin Java can be resolved.
    """

    UNSUPPORTED_LIBC = "unsupported_libc"
    UNSUPPORTED_ARCH = "unsupported_arch"
    UNSUPPORTED_VERSION = "unsupported_version"
    BUILTIN_INVALID_VERSION = "builtin_invalid_version"

    def __init__(self, code: str) -> None:
        self.code = code

class DownloadError(Exception):
    """Raised when the downloader failed to download some entries.
    """
    def __init__(self, errors: List[Tuple[DownloadEntry, str]]) -> None:
        super().__init__()
        self.errors = errors


class VersionEvent:
    """Base class for events regarding version.
    """
    __slots__ = "version",
    def __init__(self, version: str) -> None:
        self.version = version

class VersionLoadingEvent(VersionEvent):
    pass

class VersionFetchingEvent(VersionEvent):
    pass

class VersionLoadedEvent(VersionEvent):
    pass

class JarFoundEvent:
    pass

class AssetsResolveEvent:
    __slots__ = "index_version", "count"
    def __init__(self, index_version: str, count: Optional[int]) -> None:
        self.index_version = index_version
        self.count = count

class LibrariesResolvingEvent:
    """Event triggered when libraries start being resolved.
    """

class LibrariesResolvedEvent:
    """Event triggered when all libraries has been successfully resolved.
    """
    __slots__ = "class_libs_count", "native_libs_count", "excluded_libs"
    def __init__(self, class_libs_count: int, native_libs_count: int, excluded_libs: List[LibrarySpecifier]) -> None:
        self.class_libs_count = class_libs_count
        self.native_libs_count = native_libs_count
        self.excluded_libs = excluded_libs

class LoggerFoundEvent:
    __slots__ = "version"
    def __init__(self, version: str) -> None:
        self.version = version

class JvmLoadingEvent:
    """Event triggered when JVM start being resolved.
    """

class JvmLoadedEvent:
    """Event triggered when JVM has been resolved. If count is none then the resolved 
    version is a builtin JVM.
    """
    __slots__ = "version", "files_count"
    def __init__(self, version: Optional[str], files_count: Optional[int]) -> None:
        self.version = version
        self.files_count = files_count

class ArgsFixesEvent:
    """Event triggered when arguments where computed, and sum up applied fixes.
    """

    LEGACY_RESOLUTION = "legacy_resolution"
    MAIN_CLASS_FIRST = "main_class_first"
    LEGACY_PROXY = "legacy_proxy"
    LEGACY_MERGE_SORT = "legacy_merge_sort"

    __slots__ = "fixes",
    def __init__(self, fixes: List[str]) -> None:
        self.fixes = fixes

class BinaryInstallEvent:
    """Event triggered when a game's binary has been extracted to the temporary bin
    directory, this include source path and the destination name within bin directory.
    """
    __slots__ = "src_file", "dst_name",
    def __init__(self, src_file: Path, dst_name: str) -> None:
        self.src_file = src_file
        self.dst_name = dst_name

class DownloadStartEvent:
    __slots__ = "threads_count", "entries_count", "size"
    def __init__(self, threads_count: int, entries_count: int, size: int) -> None:
        self.threads_count = threads_count
        self.entries_count = entries_count
        self.size = size

class DownloadProgressEvent:
    __slots__ = "thread_id", "count", "entry", "size", "speed"
    def __init__(self, thread_id: int, count: int, entry: DownloadEntry, size: int, speed: float) -> None:
        self.thread_id = thread_id
        self.count = count
        self.entry = entry
        self.size = size
        self.speed = speed

class DownloadCompleteEvent:
    __slots__ = tuple()


def parse_download_entry(value: Any, dst: Path, path: str) -> DownloadEntry:
    """Common function to parse a download entry from a metadata JSON file.
    """

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


def interpret_rule(rules: Any, features: Dict[str, bool], path: str, *, 
    all_features: Optional[Set[str]] = None
) -> bool:
    """Common function to interpret rules and determine if the condition is met.
    """

    if not isinstance(rules, list):
        raise ValueError(f"{path} must be a list")

    allowed = False
    for i, rule in enumerate(rules):

        if not isinstance(rule, dict):
            raise ValueError(f"{path}/{i} must be an object")

        rule_os = rule.get("os")
        if rule_os is not None and not interpret_rule_os(rule_os, f"{path}/{i}/os"):
            continue

        rule_features = rule.get("features")
        if rule_features is not None:
            
            if not isinstance(rule_features, dict):
                raise ValueError(f"{path}/{i}/features must be an object")

            feat_valid = True
            for feat_name, feat_expected in rule_features.items():
                if all_features is not None:
                    all_features.add(feat_name)
                if features.get(feat_name) != feat_expected:
                    feat_valid = False
            
            if not feat_valid:
                continue
        
        action = rule.get("action")
        if action == "disallow":
            return False    # Early return because of disallow.
        elif action == "allow":
            allowed = True  # Only other possible value is "allow".
        else:
            raise ValueError(f"{path}/{i}/action must be 'allow' and 'disallow'")

    return allowed


def interpret_rule_os(rule_os: Any, path: str) -> bool:
    """Common function to interpret a rule constraint on the running OS.
    """

    if not isinstance(rule_os, dict):
        raise ValueError(f"{path} must be an object")
    
    os_name = rule_os.get("name")
    if os_name is None or os_name == minecraft_os:
        os_arch = rule_os.get("arch")
        if os_arch is None or os_arch == minecraft_arch:
            os_version = rule_os.get("version")
            if os_version is None or re.search(os_version, platform.version()) is not None:
                return True
    return False


def interpret_args(args: Any, features: Dict[str, bool], dst: List[str], path: str, *, 
    all_features: Optional[Set[str]] = None
) -> None:

    if not isinstance(args, list):
        raise ValueError(f"{path} must be a list")

    for i, arg in enumerate(args):

        if isinstance(arg, str):
            dst.append(arg)
        elif isinstance(arg, dict):

            rules = arg.get("rules")
            if rules is not None:
                if not interpret_rule(rules, features, f"{path}/{i}/rules", all_features=all_features):
                    continue
            
            arg_value = arg["value"]
            if isinstance(arg_value, list):
                dst.extend(arg_value)
            elif isinstance(arg_value, str):
                dst.append(arg_value)
            else:
                raise ValueError(f"{path}/{i}/value must be a list or a string")
        else:
            raise ValueError(f"{path}/{i} must be an object or a string")


def replace_vars(text: str, replacements: Dict[str, str]) -> str:
    """Replace all variables of the form `${foo}` in a string. If some keys are missing,
    the unformatted text is returned.
    """
    try:
        return text.replace("${", "{").format_map(replacements)
    except KeyError:
        return text


def replace_list_vars(text_list: List[str], replacements: Dict[str, str]) -> Iterator[str]:
    """Call `replace_vars` on multiple texts in a list with the same replacements.
    """
    return (replace_vars(elt, replacements) for elt in text_list)


def get_minecraft_dir() -> Path:
    """Internal function to get the default directory for installing
    and running Minecraft.
    """
    home = Path.home()
    return {
        "Windows": home.joinpath("AppData", "Roaming", ".minecraft"),
        "Darwin": home.joinpath("Library", "Application Support", "minecraft"),
    }.get(platform.system(), home / ".minecraft")


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
    "64bit": 64,
    "32bit": 32
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
