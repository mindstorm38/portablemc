"""Definition of tasks supporting the standard metadata format used by Mojang. This 
repository also provide Mojang's version manifest which can be used as default version
repository, allowing resolution of what we call "vanilla" versions.
"""

from subprocess import Popen, PIPE, STDOUT, TimeoutExpired
from json import JSONDecodeError
from pathlib import Path
from uuid import uuid4
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

from typing import Optional, List, Iterator, Any, Dict, Tuple, Union, Callable


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
    
    def gen_bin_dir(self) -> Path:
        """Generate a random named binary directory, may be used for any kind of temporary
        files and data. Usually for shared libraries used by the game. Note that this 
        directory isn't created by this method, only its path is returned.
        """
        return self.bin_dir / str(uuid4())


class FullMetadata:
    """Fully computed metadata, all the layers are merged together.
    """

    def __init__(self, data: dict) -> None:
        self.data = data

class Version:
    """This class holds version's metadata, as resolved by `MetadataTask`. Can be 
    obtained from `Version.merge()`.
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


class VersionRepository:
    """Abstract class for a version repository. A repository provides a way of validating
    and if required, fetching the version metadata associated to the version.

    Examples of repositories includes the Mojang's version manifest, fabric/quilt 
    repositories or even archive.org repository.
    """

    def load_version(self, version: Version, state: State) -> bool:
        """This function is responsible for loading a version's metadata. Note that 
        implementations are free to load other things.

        This function returns true if loading was successful, if this function returns
        false, the `fetch_version` function is then called to fetch the version. This can
        be used to check integrity of a version.

        De default implementation of this function just read metadata file.

        :param version: The version metadata to validate or not.
        :param state: Sequence state when the MetadataTask execute.
        :return: True if the given version is valid and its metadata was properly loaded.
        """
        return version.read_metadata_file()

    def fetch_version(self, version: Version, state: State) -> None:
        """Fetch the data of the given version.

        :param version: The version meta to fetch data into.
        :param state: Sequence state when the MetadataTask execute.
        :raises VersionNotFoundError: In case of error finding the version.
        """
        raise NotImplementedError

class VersionRepositories:
    """Mapping of version identifiers to the repository to use for them. This state is
    set up by `MetadataTask` and can be altered in order to add a repository. A default
    repository is also given (usually the Mojang's version manifest) for cases where no
    mapping exists.
    """

    def __init__(self, default: VersionRepository) -> None:
        self.default = default
        self.mapping: Dict[str, VersionRepository] = {}
    
    def get(self, version_id: str) -> VersionRepository:
        return self.mapping.get(version_id, self.default)
    
    def insert(self, version_id: str, repository: VersionRepository) -> None:
        self.mapping[version_id] = repository


class MetadataRoot:
    """Small class used to specify the root version to load with its parents by 
    `MetadataTask`.
    """
    __slots__ = "version",
    def __init__(self, version: str) -> None:
        self.version = version

class Jar:
    """This state object contains the version JAR to use for launching the game.
    It may not be present if no jar is specified by the metadata.
    """
    __slots__ = "path",
    def __init__(self, path: Path) -> None:
        self.path = path

class Assets:
    """Represent the loaded assets for the current version. This contains the index 
    version as well as all assets and if they need to copied to virtual or resources
    directory.
    """
    __slots__ = "index_version", "assets", "virtual_dir", "resources_dir"
    def __init__(self, index_version: str, assets: Dict[str, Path], virtual_dir: Optional[Path], resources_dir: Optional[Path]) -> None:
        self.index_version = index_version
        self.assets = assets
        self.virtual_dir = virtual_dir
        self.resources_dir = resources_dir

class LibrariesOptions:
    """Options for resolving libraries, it provides predicates to filtering out some 
    libraries and also version fixes to change version of some libraries. **Predicates are
    applied before fixing.**

    By default, this include a version fix for com.mojang:authlib:2.1.28 which is broken
    and only used for versions 1.16.4 and 1.16.5. To fix disabled multiplayer buttons 
    with offline sessions, this version is modified to 2.2.30.

    This state is set up by `LibrariesTask`.
    """
    __slots__ = "predicates", "version_fixes"
    def __init__(self) -> None:
        self.predicates: List[Callable[[LibrarySpecifier], bool]] = []
        self.version_fixes: Dict[LibrarySpecifier, str] = {
            LibrarySpecifier("com.mojang", "authlib", "2.1.28"): "2.2.30"
        }

class Libraries:
    """Represent the loaded libraries for the current version. This contains both 
    class path libraries that should be added to the class path, and the native libraries
    that should be extracted in a temporary bin directory. These native libraries are no
    longer used in modern versions.

    Native libraries may points either to a JAR file where so/dll/dylib files are stored,
    or directly to a shared library, in such case a symlink is created to the game's
    binaries directory (or copied if not possible). If the file is .so and has version 
    numbers like .so.1.2.3, the version numbers are removed.

    This state is set up by `LibrariesTask`.
    """
    __slots__ = "class_libs", "native_libs"
    def __init__(self) -> None:
        self.class_libs: List[Path] = []
        self.native_libs: List[Path] = []

class Logger:
    """Represent the loaded logging configuration for the current version. It contain
    the logging file's path and the argument to add to the command line, this argument
    contains a format placeholder '${path}' that should be replaced with the file's path.
    """
    __slots__ = "path", "arg"
    def __init__(self, path: Path, arg: str) -> None:
        self.path = path
        self.arg = arg

class Jvm:
    """Indicates JVM to use for running the game. This state is automatically generated
    by the 'JvmTask' when used and successful. It specifies the executable file's path
    and the JVM version selected. The JVM version is optional because it may not be
    known at this time.
    """
    __slots__ = "executable_file", "version"
    def __init__(self, executable_file: Path, version: Optional[str]) -> None:
        self.executable_file = executable_file
        self.version = version

class ArgsOptions:
    """Global options applied to vanilla version preparation. This state options is
    optional and may not be present, in such case the default options are used.
    This includes a set of predefined fixes that are enabled by default in order to
    address some known common problems of Minecraft versions.

    These options does not alter the content of installed files, but rather which files
    are installed and which ones are used to run the game.
    """

    FIX_LEGACY_PROXY = object()
    FIX_LEGACY_MERGE_SORT = object()
    FIX_LEGACY_RESOLUTION = object()

    def __init__(self):
        
        self.features: Dict[str, bool] = {}
        
        self.auth_session: Optional[AuthSession] = None
        self.demo: bool = False
        self.resolution: Optional[Tuple[int, int]] = None
        self.disable_multiplayer: bool = False
        self.disable_chat: bool = False
        self.server_address: Optional[str] = None
        self.server_port: Optional[int] = None
        
        self.fixes = { 
            self.FIX_LEGACY_PROXY, 
            self.FIX_LEGACY_MERGE_SORT,
            self.FIX_LEGACY_RESOLUTION,
        }
    
    def set_offline(self, username: Optional[str], uuid: Optional[str]) -> None:
        """Shortcut for setting an offline session with the given username/uuid pair.
        """
        self.auth_session = OfflineAuthSession(uuid, username)

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


class MetadataTask(Task):
    """Version metadata resolving.

    This task resolves the current version's metadata and inherited versions.

    :in Context: The installation context.
    :in MetadataRoot: Describe which root version to start resolving.
    :in VersionRepositories: Mapping of version identifiers to their repository, also
    providing the default repository, like Mojang's version manifest.
    :out Version: The root version.
    :out VersionMetadata: The resolved version metadata.
    """

    def __init__(self, *, max_parents: int = 10) -> None:
        self.max_parents = max_parents

    def execute(self, state: State, watcher: Watcher) -> None:

        context = state[Context]
        repositories = state[VersionRepositories]

        version_id: Optional[str] = state[MetadataRoot].version
        versions: List[Version] = []

        while version_id is not None:

            if len(versions) > self.max_parents:
                raise TooMuchParentsError(versions)
            
            watcher.handle(VersionLoadingEvent(version_id))

            # Get version instance and load/fetch is needed.
            version = context.get_version(version_id)
            repo = repositories.get(version_id)

            if not repo.load_version(version, state):
                watcher.handle(VersionLoadingEvent(version_id))
                repo.fetch_version(version, state)
            
            watcher.handle(VersionLoadedEvent(version_id))

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


class JarTask(Task):
    """Version JAR file resolving task.

    This task resolve which JAR file should be used to run the current version. This can
    be any valid JAR file present in the version's hierarchy.

    :in VersionMetadata: The version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out Jar: The resolved version JAR with associated version ID providing it.
    """

    def execute(self, state: State, watcher: Watcher) -> None:

        version = state[Version]
        metadata = state[FullMetadata].data

        jar_file = version.jar_file()

        # First try to find a /downloads/client download entry.
        version_dls = metadata.get("downloads")
        if version_dls is not None:
            if not isinstance(version_dls, dict):
                raise ValueError("metadata: /downloads must be an object")
            client_dl = version_dls.get("client")
            if client_dl is not None:
                state[DownloadList].add(parse_download_entry(client_dl, jar_file, "metadata: /downloads/client"), verify=True)
                state.insert(Jar(jar_file))
                watcher.handle(JarFoundEvent())
                return
        
        # If no download entry has been found, but the JAR exists, we use it.
        if jar_file.is_file():
            state.insert(Jar(jar_file))
            watcher.handle(JarFoundEvent())
            return
        
        raise JarNotFoundError()


class AssetsTask(Task):
    """Version assets resolving task.

    This task resolves which asset index to use for the version.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out Assets: Optional, present if the assets are specified.
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

        state.insert(Assets(assets_index_version, assets, virtual_dir, resources_dir))
        watcher.handle(AssetsResolveEvent(assets_index_version, len(assets)))


class AssetsFinalizeTask(Task):
    """This task finalize the installation of assets after being downloaded.
    """

    def execute(self, state: State, watcher: Watcher) -> None:
        
        assets = state[Assets]

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
    :in Version: The root version, used to recursively resolve libraries.
    :in(setup) LibrariesOptions: Options for library resolution.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out Libraries: Optional, present if the libraries are specified.
    """

    def setup(self, state: State) -> None:
        state.insert(LibrariesOptions())
        state.insert(Libraries())

    def execute(self, state: State, watcher: Watcher) -> None:
        
        context = state[Context]
        version = state[Version]
        options = state[LibrariesOptions]
        libraries = state[Libraries]
        dl = state[DownloadList]
            
        excluded_libs = []
        watcher.handle(LibrariesResolvingEvent())

        # Recursion order is important for libraries resolving, root libraries should
        # be placed first.
        for version in state[Version].recurse():

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
                    
                    # TODO: Support features in this?
                    if not interpret_rule(rules, {}, f"metadata: /libraries/{library_idx}/rules"):
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
                    
                    libs = libraries.native_libs

                else:
                    libs = libraries.class_libs
                
                # Apply predicates after the final classifier has been set, if relevant.
                if not all((pred(spec) for pred in options.predicates)):
                    excluded_libs.append(spec)
                    continue

                # Check version fixes.
                version_fix = options.version_fixes.get(spec)
                if version_fix is not None:
                    spec.version = version_fix

                dl_entry: Optional[DownloadEntry] = None
                jar_path_rel = spec.file_path()
                jar_path = context.libraries_dir / jar_path_rel
                
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
                    dl.add(dl_entry, verify=True)

        watcher.handle(LibrariesResolvedEvent(
            len(libraries.class_libs), 
            len(libraries.native_libs), 
            excluded_libs))


class LoggerTask(Task):
    """Logger resolving task.

    This task resolves which logger configuration to use for the selected version.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out Logger: Optional, the logging configuration if this version specifies it.
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

        state.insert(Logger(file_path, argument))
        watcher.handle(LoggerFoundEvent(file_id.replace(".xml", "")))


class JvmTask(Task):
    """JVM resolving task, may not succeed on all platforms.

    This task resolves which official Mojang JVM should be used.

    :in Context: The installation context.
    :in FullMetadata: The full version metadata.
    :in DownloadList: Used to add the JAR file to download if relevant.
    :out Jvm: If JVM is found.
    """

    def execute(self, state: State, watcher: Watcher) -> None:

        # Don't do anything if JVM is already provided.
        if state.get(Jvm) is not None:
            return
        
        context = state[Context]
        metadata = state[FullMetadata].data
        dl = state[DownloadList]

        watcher.handle(JvmLoadingEvent())
        
        jvm_version_info = metadata.get("javaVersion", {})
        if not isinstance(jvm_version_info, dict):
            raise ValueError("metadata: /javaVersion must be a string")

        jvm_major_version = jvm_version_info.get("majorVersion")
        if jvm_major_version is not None and not isinstance(jvm_major_version, int):
            raise ValueError("metadata: /javaVersion/majorVersion must be an integer")

        if platform.system() == "Linux" and platform.libc_ver()[0] != "glibc":
            return self.find_builtin(state, watcher, JvmNotFoundError.UNSUPPORTED_LIBC, jvm_major_version)

        jvm_version_type = jvm_version_info.get("component", "jre-legacy")
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
                return self.find_builtin(state, watcher, JvmNotFoundError.UNSUPPORTED_ARCH, jvm_major_version)

            jvm_meta = jvm_arch_meta.get(jvm_version_type)
            if not isinstance(jvm_meta, list) or not len(jvm_meta):
                return self.find_builtin(state, watcher, JvmNotFoundError.UNSUPPORTED_VERSION, jvm_major_version)

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
        
        state.insert(Jvm(jvm_exec, jvm_version))
        watcher.handle(JvmLoadedEvent(jvm_version, len(jvm_files)))

    def find_builtin(self, state: State, watcher: Watcher, reason: str, major_version: Optional[int]) -> None:
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

        state.insert(Jvm(Path(builtin_path), version))
        watcher.handle(JvmLoadedEvent(version, None))


class ArgsTask(Task):
    """This task compute the final arguments from all previous states.

    :in Context: The installation context.
    :in Version: The version object that's being installed.
    :in FullMetadata: The full version metadata.
    :in Jar: Version JAR file, it is added at the end of class path.
    :in Assets: Version assets listing.
    :in Libraries: Version libraries listing.
    :in Logger: Version logger (optional).
    :in Jvm: Version JVM for execution.
    :in(setup) ArgsOptions: Options for this task to customize arguments.
    :out Args: Computed version arguments.
    """

    def setup(self, state: State) -> None:
        state.insert(ArgsOptions())

    def execute(self, state: State, watcher: Watcher) -> None:
        
        context = state[Context]
        version = state[Version]
        metadata = state[FullMetadata].data
        jar = state[Jar]
        libraries = state[Libraries]
        assets = state[Assets]
        logging = state.get(Logger)
        jvm = state[Jvm]
        opts = state[ArgsOptions]

        # Main class
        main_class = metadata.get("mainClass")
        if not isinstance(main_class, str):
            raise ValueError("metadata: /mainClass must be a string")
        
        # Features
        features = {
            "is_demo_user": opts.demo,
            "has_custom_resolution": opts.resolution is not None,
            "is_quick_play_multiplayer": opts.server_address is not None,
            **opts.features
        }

        # List of fixes for summary...
        fixes = []

        # Get authentication of create a random offline.
        auth_session = opts.auth_session or OfflineAuthSession(None, None)

        # Class path, without main class (added later depending on arguments present).
        class_path = list(map(str, libraries.class_libs))
        
        # Arguments
        jvm_args = [str(jvm.executable_file)]
        game_args = []

        # Check if modern arguments are present (> 1.12.2).
        modern_args = metadata.get("arguments")
        if modern_args is not None:

            if not isinstance(modern_args, dict):
                raise ValueError("metadata: /arguments must be an object")
        
            # Interpret JVM arguments.
            modern_jvm_args = modern_args.get("jvm", [])
            if not isinstance(modern_jvm_args, list):
                raise ValueError("metadata: /arguments/jvm must be a list")
            interpret_args(modern_jvm_args, features, jvm_args)
            
            # Interpret Game arguments.
            modern_game_args = modern_args.get("game", [])
            if not isinstance(modern_game_args, list):
                raise ValueError("metadata: /arguments/game must be a list")
            interpret_args(modern_game_args, features, game_args)
        
        else:

            interpret_args(legacy_jvm_args, features, jvm_args)

            # Append legacy game arguments, if available.
            legacy_game_args = metadata.get("minecraftArguments")
            if legacy_game_args is not None:
                if not isinstance(legacy_game_args, str):
                    raise ValueError("metadata: /minecraftArguments must be a string")
                game_args.extend(legacy_game_args.split(" "))

        # JVM argument for logging config
        if logging is not None:
            jvm_args.append(logging.arg.replace("${path}", str(logging.path)))

        # JVM argument for launch wrapper JAR path
        if main_class == "net.minecraft.launchwrapper.Launch":
            jvm_args.append(f"-Dminecraft.client.jar={jar.path}")

        # If no modern arguments, fix some arguments.
        if modern_args is None:

            # Resolution arguments are usually supported by many versions prior to their
            # addition in the modern game arguments.
            if opts.resolution is not None:
                if ArgsOptions.FIX_LEGACY_RESOLUTION in opts.fixes:
                    fixes.append(ArgsFixesEvent.LEGACY_RESOLUTION)
                    game_args.extend((
                        "--width", str(opts.resolution[0]),
                        "--height", str(opts.resolution[1]),
                    ))
            
            # Old versions seems to prefer having the main class first in class path.
            # This fix cannot be disabled for now (fixme?).
            class_path.insert(0, str(jar.path))
            fixes.append(ArgsFixesEvent.MAIN_CLASS_FIRST)

        else:
            # Modern versions seems to prefer having the main class last in class path.
            class_path.append(str(jar.path))
   
        # Apply some fixes for legacy versions.
        if len(opts.fixes):

            # Get the last version in the parent's tree, we use it to apply legacy fixes.
            ancestor_id = list(version.recurse())[-1].id

            # Legacy proxy aims to fix things like skins on old versions.
            # This is applicable to all alpha/beta and 1.0:1.5
            if ArgsOptions.FIX_LEGACY_PROXY in opts.fixes:

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
            if ArgsOptions.FIX_LEGACY_MERGE_SORT in opts.fixes and ancestor_id.startswith(("a1.", "b1.")):
                fixes.append(ArgsFixesEvent.LEGACY_MERGE_SORT)
                jvm_args.append("-Djava.util.Arrays.useLegacyMergeSort=true")

        # Global options.        
        if opts.disable_multiplayer:
            game_args.append("--disableMultiplayer")
        if opts.disable_chat:
            game_args.append("--disableChat")
        if opts.server_address is not None:
            game_args.extend(("--server", opts.server_address))
        if opts.server_port is not None:
            game_args.extend(("--port", str(opts.server_port)))

        # Arguments replacements
        args_replacements: Dict[str, str] = {
            # Game
            "auth_player_name": auth_session.username,
            "version_name": version.id,
            "library_directory": str(context.libraries_dir),
            "game_directory": str(context.work_dir),
            "assets_root": str(context.assets_dir),
            "assets_index_name": assets.index_version,
            "auth_uuid": auth_session.uuid,
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
            "classpath": os.pathsep.join(class_path)
        }

        if opts.server_address is not None:
            args_replacements["quickPlayMultiplayer"] = f"{opts.server_address}:{opts.server_port or 25565}"

        if opts.resolution is not None:
            args_replacements["resolution_width"] = str(opts.resolution[0])
            args_replacements["resolution_height"] = str(opts.resolution[1])

        watcher.handle(ArgsFixesEvent(fixes))
        state.insert(Args(jvm_args, game_args, main_class, args_replacements))


class RunTask(Task):
    """This task run the game.

    :in Context: The installation context.
    :in Args: Resolved arguments for running the game.
    :in Libraries: For retrieving native libraries to extract.
    """

    def execute(self, state: State, watcher: Watcher) -> None:

        context = state[Context]
        args = state[Args]
        libraries = state[Libraries]

        bin_dir = context.gen_bin_dir()
        replacements = args.args_replacements.copy()
        replacements["natives_directory"] = str(bin_dir)
        
        from zipfile import ZipFile

        bin_dir.mkdir(parents=True, exist_ok=True)

        try:

            # Here we copy libraries into the bin directory, in case of archives (jar, zip)
            # we extract all so/dll/dylib files into the directory, if this is a directly
            # pointing to an archive, we symlink or copy it in-place.
            if len(libraries.native_libs):
                for src_file in libraries.native_libs:

                    if not src_file.is_file():
                        raise ValueError(f"source native file not found: {src_file}")

                    native_name = src_file.name
                    if native_name.endswith((".zip", ".jar")):

                        with ZipFile(src_file, "r") as native_zip:
                            for native_zip_info in native_zip.infolist():
                                native_name = native_zip_info.filename
                                if native_name.endswith((".so", ".dll", ".dylib")):

                                    try:
                                        native_name = native_name[native_name.rindex("/") + 1:]
                                    except ValueError:
                                        native_name = native_name
                                    
                                    dst_file = bin_dir / native_name

                                    with native_zip.open(native_zip_info, "r") as src_fp:
                                        with dst_file.open("wb") as dst_fp:
                                            shutil.copyfileobj(src_fp, dst_fp)
                                    
                                    watcher.handle(BinaryInstallEvent(src_file / native_name, native_name))

                    else:

                        # Here we try to remove the version numbers of .so files.
                        so_idx = native_name.rfind(".so")
                        if so_idx >= 0:
                            native_name = native_name[:so_idx + len(".so")]
                        # Try to symlink the file in the bin dir, and fallback to simple copy.
                        dst_file = bin_dir / native_name

                        try:
                            dst_file.symlink_to(src_file)
                        except OSError:
                            shutil.copyfile(src_file, dst_file)
                        
                        watcher.handle(BinaryInstallEvent(src_file, native_name))

            # We create the wrapper process with required arguments.
            process = self.process_create([
                *replace_list_vars(args.jvm_args, replacements),
                args.main_class,
                *replace_list_vars(args.game_args, replacements)
            ], context.work_dir)


            self.process_wait(process)

        finally:
            # Any error while setting up the binary directory cause it to be deleted.
            shutil.rmtree(bin_dir, ignore_errors=True)

    def process_create(self, args: List[str], work_dir: Path) -> Popen:
        """This function is called when process needs to be created with the given 
        arguments in the given working directory. The default implementation does nothing
        special but this can be used to create the process with enabled output piping,
        to later use in `process_wait`.
        """
        return Popen(args, cwd=work_dir)

    def process_wait(self, process: Popen) -> None:
        """This function is called with the running Minecraft process for waiting the end
        of the process. Implementors may want to read incoming logging.
        """
        process.wait()


class StreamRunTask(RunTask):
    """A specialized implementation of `RunTask` which allows streaming the game's output
    logs. This implementation also provides parsing of log4j XML layouts for logs.
    """
    
    def process_create(self, args: List[str], work_dir: Path) -> Popen:
        return Popen(args, cwd=work_dir, stdout=PIPE, stderr=STDOUT, bufsize=1, universal_newlines=True)

    def process_wait(self, process: Popen) -> None:

        from threading import Thread

        thread = Thread(target=self.process_stream_thread, name="Minecraft Stream Thread", args=(process,))
        thread.start()

        process.wait()

    def process_stream_thread(self, process: Popen) -> None:

        stdout = process.stdout
        assert stdout is not None, "should not be none because it should be piped"

        parser = None
        for line in iter(stdout.readline, ""):

            if parser is None:
                if line.lstrip().startswith("<log4j:"):
                    parser = XmlStreamParser()
                else:
                    parser = StreamParser()

            parser.feed(line, self.process_stream_event)
    
    def process_stream_event(self, event: Any) -> None:
        """This function gets called when an event is received from the game's log.
        """

class StreamParser:
    """Base implementation of game's output stream parsing, this default implementation
    just forward incoming lines to the callback.
    """

    def feed(self, line: str, callback: Callable[[Any], None]) -> None:
        callback(line)

class XmlStreamParser(StreamParser):
    """This parser produces `XmlStreamEvent` kind of events by parsing the game's stream
    as a log4j log stream.
    """

    def __init__(self) -> None:
        import xml.etree.ElementTree as ET
        self.xml = ET.XMLPullParser(["start", "end"])
        self.xml.feed("<?xml version=\"1.0\"?><root xmlns:log4j=\"log4j\">")
        self.next_event = None

    def feed(self, line: str, callback: Callable[[Any], None]) -> None:
        self.xml.feed(line)
        for event, elem in self.xml.read_events():
            if elem.tag == "{log4j}Event":
                if event == "start":
                    self.next_event = XmlStreamEvent(int(elem.attrib["timestamp"]) / 1000.0,
                        elem.attrib["logger"],
                        elem.attrib["level"],
                        elem.attrib["thread"])
                elif event == "end" and self.next_event is not None:
                    callback(self.next_event)
                    self.next_event = None
            elif event == "end" and self.next_event is not None:
                if elem.tag == "{log4j}Message":
                    self.next_event.message = elem.text
                elif elem.tag == "{log4j}Throwable":
                    self.next_event.throwable = elem.text

class XmlStreamEvent:
    """Class representing an event happening in the game's logs.
    """

    __slots__ = "time", "logger", "level", "thread", "message", "throwable"

    def __init__(self, time: float, logger: str, level: str, thread: str) -> None:
        self.time = time
        self.logger = logger
        self.level = level
        self.thread = thread
        self.message = None
        self.throwable = None
    
    def __repr__(self) -> str:
        return f"<ProcessEvent date: {self.time}, logger: {self.logger}, level: {self.level}, thread: {self.thread}, message: {repr(self.message)}>"


class VersionNotFoundError(Exception):
    """Raised when a version was not found. The version that was not found is given.
    """
    def __init__(self, version: str) -> None:
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
    """Raised if no JVM can be found, the particular reason is given as code. This error
    is raised only if no builtin Java can be resolved.
    """

    UNSUPPORTED_LIBC = "unsupported_libc"
    UNSUPPORTED_ARCH = "unsupported_arch"
    UNSUPPORTED_VERSION = "unsupported_version"
    BUILTIN_INVALID_VERSION = "builtin_invalid_version"

    def __init__(self, code: str) -> None:
        self.code = code


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


class VersionManifest(VersionRepository):
    """The Mojang's official version manifest. Providing officially available versions 
    with optional cache file. It's an implementation of `VersionRepository` and so can
    be used for the default repository to resolve versions.
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
    
    def load_version(self, version: Version, state: State) -> bool:

        # If default implementation fails.
        if not super().load_version(version, state):
            return False
        
        try:
            version_super_meta = self.get_version(version.id)
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
    
    def fetch_version(self, version: Version, state: State) -> None:
        
        version_super_meta = self.get_version(version.id)
        if version_super_meta is None:
            raise VersionNotFoundError(version.id)

        res = http_request("GET", version_super_meta["url"], accept="application/json")
        
        # First decode the data and set it to the version meta. Raising if invalid.
        version.metadata = res.json()
        
        # If successful, write the raw data directly to the file.
        version.dir.mkdir(parents=True, exist_ok=True)
        with version.metadata_file().open("wb") as fp:
            fp.write(res.data)


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


def interpret_rule(rules: Any, features: Dict[str, bool], path: str) -> bool:
    """Common function to interpret rules and determine if the condition is met or not.
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
                if features.get(feat_name) != feat_expected:
                    feat_valid = False
                    break
            
            if not feat_valid:
                continue
        
        action = rule.get("action")
        if action not in ("allow", "disallow"):
            raise ValueError(f"{path}/{i}/action must be 'allow' and 'disallow'")
        
        if action == "disallow":
            return False  # Early return because of disallow.
        allowed = True    # Only other possible value is "allow".

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


def interpret_args(args: List[Union[str, dict]], features: Dict[str, bool], dst: List[str]) -> None:
    for arg in args:
        if isinstance(arg, str):
            dst.append(arg)
        else:
            rules = arg.get("rules")
            if rules is not None:
                if not interpret_rule(rules, features, "<TODO>"):
                    continue
            arg_value = arg["value"]
            if isinstance(arg_value, list):
                dst.extend(arg_value)
            elif isinstance(arg_value, str):
                dst.append(arg_value)


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


def add_vanilla_tasks(seq: Sequence, *, run: bool = False) -> None:
    """Alter a task sequence by inserting tasks for running a standard game. Vanilla 
    sequence is the most basic and required logic to run a game based on Mojang's version
    metadata format.

    This sequence take as input a `MetadataRoot` instance for the root version to load,
    a `Context` for knowing game's directories and a `VersionRepositories` with a proper
    default repository.
    
    The output of this sequence is `Args`, and the `run` argument can specify if
    you want or not to add the `RunTask` that will take these arguments to run the game.
    """

    seq.append_task(MetadataTask())
    
    # JVM resolution as early as possible, because we may need it after (or for add-ons).
    seq.append_task(JvmTask())
    seq.append_task(JarTask())
    seq.append_task(AssetsTask())
    seq.append_task(LibrariesTask())
    seq.append_task(LoggerTask())

    # Download and finalize assets that need to be copied.
    seq.append_task(DownloadTask())
    seq.append_task(AssetsFinalizeTask())

    # Finally, compute all arguments.
    seq.append_task(ArgsTask())

    # Then run, if requested.
    if run:
        seq.append_task(RunTask())


def make_vanilla_sequence(version: str, *, 
    run: bool = False, 
    context: Optional[Context] = None,
    default_repository: Optional[VersionRepository] = None
) -> Sequence:
    """Shortcut version of `add_vanilla_tasks` that construct the sequence for you and
    add required states.
    """

    seq = Sequence()
    add_vanilla_tasks(seq, run=run)

    seq.state.insert(context or Context())
    seq.state.insert(VersionRepositories(default_repository or VersionManifest()))
    seq.state.insert(MetadataRoot(version))

    return seq
