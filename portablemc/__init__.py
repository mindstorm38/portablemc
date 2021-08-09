# encoding: utf8

# Copyright (C) 2021  Théo Rozier
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

"""
Core module of PortableMC, it provides a flexible API to download and start Minecraft.
"""

from typing import cast, Generator, Callable, Optional, Tuple, Dict, Type, List
from http.client import HTTPConnection, HTTPSConnection, HTTPResponse
from urllib import parse as url_parse, request as url_request
from urllib.request import Request as UrlRequest
from urllib.error import HTTPError
from json import JSONDecodeError
from zipfile import ZipFile
from uuid import uuid4
from os import path
import platform
import hashlib
import shutil
import base64
import json
import sys
import os
import re


__all__ = [
    "LAUNCHER_NAME", "LAUNCHER_VERSION", "LAUNCHER_AUTHORS", "LAUNCHER_COPYRIGHT", "LAUNCHER_URL",
    "Context", "Version", "StartOptions", "Start", "VersionManifest",
    "AuthSession", "YggdrasilAuthSession", "MicrosoftAuthSession", "AuthDatabase",
    "DownloadEntry", "DownloadList", "DownloadProgress", "DownloadEntryProgress",
    "BaseError", "JsonRequestError", "AuthError", "VersionError", "JvmLoadingError", "DownloadError",
    "json_request", "json_simple_request",
    "merge_dict",
    "interpret_rule_os", "interpret_rule", "interpret_args",
    "replace_vars", "replace_list_vars",
    "get_minecraft_dir", "get_minecraft_os", "get_minecraft_arch", "get_minecraft_archbits", "get_minecraft_jvm_os",
    "can_extract_native",
    "LEGACY_JVM_ARGUMENTS"
]


LAUNCHER_NAME = "portablemc"
LAUNCHER_VERSION = "2.0.1"
LAUNCHER_AUTHORS = ["Théo Rozier <contact@theorozier.fr>", "Github contributors"]
LAUNCHER_COPYRIGHT = "PortableMC  Copyright (C) 2021  Théo Rozier"
LAUNCHER_URL = "https://github.com/mindstorm38/portablemc"


class Context:

    """
    This class is used to manage an installation context for Minecraft. This context can be reused multiple
    times to install multiple versions. A context stores multiple important paths but all these paths can be
    changed after the construction and before preparing versions.
    """

    def __init__(self, main_dir: Optional[str] = None, work_dir: Optional[str] = None):

        """
        Construct a Minecraft context. The main directory `main_dir` is used to construct versions, assets, libraries
        and JVM directories, but it is not stored afterward. The working directory `work_dir` (also named "game
        directory"), however it is stored as-is.\n
        By default `main_dir` is set to the default .minecraft (https://minecraft.fandom.com/fr/wiki/.minecraft) and
        `work_dir` is set by default to the value of `main_dir`.
        """

        main_dir = get_minecraft_dir() if main_dir is None else path.realpath(main_dir)
        self.work_dir = main_dir if work_dir is None else path.realpath(work_dir)
        self.versions_dir = path.join(main_dir, "versions")
        self.assets_dir = path.join(main_dir, "assets")
        self.libraries_dir = path.join(main_dir, "libraries")
        self.jvm_dir = path.join(main_dir, "jvm")
        self.bin_dir = path.join(self.work_dir, "bin")

    def has_version_metadata(self, version: str) -> bool:
        """ Return True if the given version has a metadata file. """
        return path.isfile(path.join(self.versions_dir, version, f"{version}.json"))

    def get_version_dir(self, version_id: str) -> str:
        return path.join(self.versions_dir, version_id)

    def list_versions(self) -> Generator[Tuple[str, int], None, None]:
        """ A generator method that yields all versions (version, mtime) that have a version metadata file. """
        if path.isdir(self.versions_dir):
            for version in os.listdir(self.versions_dir):
                try:
                    yield version, path.getmtime(path.join(self.versions_dir, version, f"{version}.json"))
                except OSError:
                    pass


class Version:

    """
    This class is used to manage the installation of a version and then run it.\n
    All public function in this class can be executed multiple times, however they might add duplicate URLs to
    the download list. The game still requires some parts to be prepared before starting.
    """

    def __init__(self, context: Context, version_id: str):

        """ Construct a new version, using a specific context and the exact version ID you want to start. """

        self.context = context
        self.id = version_id

        self.manifest: Optional[VersionManifest] = None
        self.dl = DownloadList()

        self.version_meta: Optional[dict] = None
        self.version_dir: Optional[str] = None
        self.version_jar_file: Optional[str] = None

        self.assets_index_version: Optional[int] = None
        self.assets_virtual_dir: Optional[str] = None
        self.assets_count: Optional[int] = None

        self.logging_file: Optional[str] = None
        self.logging_argument: Optional[str] = None

        self.classpath_libs: List[str] = []
        self.native_libs: List[str] = []

        self.jvm_version: Optional[str] = None
        self.jvm_exec: Optional[str] = None

    def prepare_meta(self, *, recursion_limit: int = 50):

        """
        Prepare all metadata files for this version, this take `inheritsFrom` key into account and all parents metadata
        files are downloaded. You can change the limit of parents metadata to download with the `recursion_limit`
        argument, if the number of parents exceed this argument, a `VersionError` is raised with
        `VersionError.TO_MUCH_PARENTS` and the version ID as argument. Each metadata file is downloaded (if not already
        cached) in their own directory named after the version ID, the directory is placed in the `versions_dir` of the
        context.\n
        This method will load the official Mojang version manifest, however you can set the `manifest` attribute of this
        object before with a custom manifest if you want to support more versions.\n
        If any version in the inherit tree is not found, a `VersionError` is raised with `VersionError.NOT_FOUND` and
        the version ID as argument.\n
        This method can raise `JsonRequestError` for any error for requests to JSON file.
        """

        version_meta, version_dir = self._prepare_meta_internal(self.id)
        while "inheritsFrom" in version_meta:
            if recursion_limit <= 0:
                raise VersionError(VersionError.TO_MUCH_PARENTS, self.id)
            recursion_limit -= 1
            parent_meta, _ = self._prepare_meta_internal(version_meta["inheritsFrom"])
            del version_meta["inheritsFrom"]
            merge_dict(version_meta, parent_meta)

        self.version_meta, self.version_dir = version_meta, version_dir

    def _prepare_meta_internal(self, version_id: str) -> Tuple[dict, str]:

        version_dir = self.context.get_version_dir(version_id)
        version_meta_file = path.join(version_dir, f"{version_id}.json")

        try:
            with open(version_meta_file, "rt") as version_meta_fp:
                return json.load(version_meta_fp), version_dir
        except (OSError, JSONDecodeError):
            version_super_meta = self._ensure_version_manifest().get_version(version_id)
            if version_super_meta is not None:
                content = json_simple_request(version_super_meta["url"])
                os.makedirs(version_dir, exist_ok=True)
                with open(version_meta_file, "wt") as version_meta_fp:
                    json.dump(content, version_meta_fp, indent=2)
                return content, version_dir
            else:
                raise VersionError(VersionError.NOT_FOUND, version_id)

    def _ensure_version_manifest(self) -> 'VersionManifest':
        if self.manifest is None:
            self.manifest = VersionManifest.load_from_url()
        return self.manifest

    def _check_version_meta(self):
        if self.version_meta is None:
            raise ValueError("You should install metadata first.")

    def prepare_jar(self):

        """
        Must be called once metadata file are prepared, using `prepare_meta`, if not, `ValueError` is raised.\n
        If the metadata provides a client download URL, and the version JAR file doesn't exists or have not the expected
        size, it's added to the download list to be downloaded to the same directory as the metadata file.\n
        If no download URL is provided by metadata and the JAR file does not exists, a VersionError is raised with
        `VersionError.JAR_NOT_FOUND` and the version ID as argument.
        """

        self._check_version_meta()
        self.version_jar_file = path.join(self.version_dir, f"{self.id}.jar")
        client_download = self.version_meta.get("downloads", {}).get("client")
        if client_download is not None:
            entry = DownloadEntry.from_meta(client_download, self.version_jar_file, name=f"{self.id}.jar")
            if not path.isfile(entry.dst) or path.getsize(entry.dst) != entry.size:
                self.dl.append(entry)
        elif not path.isfile(self.version_jar_file):
            raise VersionError(VersionError.JAR_NOT_FOUND, self.id)

    def prepare_assets(self):

        """
        Must be called once metadata file are prepared, using `prepare_meta`, if not, `ValueError` is raised.\n
        This method download the asset index file (if not already cached) named after the asset version into the
        directory `indexes` placed into the directory `assets_dir` of the context. Once ready, the asset index file
        is analysed and each object is checked, if it does not exist or not have the expected size, it is downloaded
        to the `objects` directory placed into the directory `assets_dir` of the context.\n
        If the metadata doesn't provide an `assetIndex`, the process is skipped.\n
        This method also set the `assets_count` attribute with the number of assets for this version.\n
        This method can raise `JsonRequestError` if it fails to load the asset index file.
        """

        self._check_version_meta()

        assets_indexes_dir = path.join(self.context.assets_dir, "indexes")
        asset_index_info = self.version_meta.get("assetIndex")
        if asset_index_info is None:
            return

        assets_index_version = self.version_meta.get("assets", asset_index_info.get("id", None))
        if assets_index_version is None:
            return

        assets_index_file = path.join(assets_indexes_dir, f"{assets_index_version}.json")

        try:
            with open(assets_index_file, "rb") as assets_index_fp:
                assets_index = json.load(assets_index_fp)
        except (OSError, JSONDecodeError):
            asset_index_url = asset_index_info["url"]
            assets_index = json_simple_request(asset_index_url)
            os.makedirs(assets_indexes_dir, exist_ok=True)
            with open(assets_index_file, "wt") as assets_index_fp:
                json.dump(assets_index, assets_index_fp)

        assets_objects_dir = path.join(self.context.assets_dir, "objects")
        assets_virtual_dir = path.join(self.context.assets_dir, "virtual", assets_index_version)
        assets_mapped_to_resources = assets_index.get("map_to_resources", False)  # For version <= 13w23b
        assets_virtual = assets_index.get("virtual", False)  # For 13w23b < version <= 13w48b (1.7.2)

        for asset_id, asset_obj in assets_index["objects"].items():
            asset_hash = asset_obj["hash"]
            asset_hash_prefix = asset_hash[:2]
            asset_size = asset_obj["size"]
            asset_file = path.join(assets_objects_dir, asset_hash_prefix, asset_hash)
            if not path.isfile(asset_file) or path.getsize(asset_file) != asset_size:
                asset_url = f"https://resources.download.minecraft.net/{asset_hash_prefix}/{asset_hash}"
                self.dl.append(DownloadEntry(asset_url, asset_file, size=asset_size, sha1=asset_hash, name=asset_id))

        def finalize():
            if assets_mapped_to_resources or assets_virtual:
                for asset_id_to_cpy in assets_index["objects"].keys():
                    if assets_mapped_to_resources:
                        resources_asset_file = path.join(self.context.work_dir, "resources", asset_id_to_cpy)
                        if not path.isfile(resources_asset_file):
                            os.makedirs(path.dirname(resources_asset_file), exist_ok=True)
                            shutil.copyfile(asset_file, resources_asset_file)
                    if assets_virtual:
                        virtual_asset_file = path.join(assets_virtual_dir, asset_id_to_cpy)
                        if not path.isfile(virtual_asset_file):
                            os.makedirs(path.dirname(virtual_asset_file), exist_ok=True)
                            shutil.copyfile(asset_file, virtual_asset_file)

        self.dl.add_callback(finalize)
        self.assets_index_version = assets_index_version
        self.assets_virtual_dir = assets_virtual_dir
        self.assets_count = len(assets_index["objects"])

    def prepare_logger(self):

        """
        Must be called once metadata file are prepared, using `prepare_meta`, if not, `ValueError` is raised.\n
        This method check the metadata for a client logging configuration, it it doesn't exist the configuration is
        added to the download list.
        """

        self._check_version_meta()
        client_logging = self.version_meta.get("logging", {}).get("client")
        if client_logging is not None:
            logging_file_info = client_logging["file"]
            logging_file = path.join(self.context.assets_dir, "log_configs", logging_file_info["id"])
            download_entry = DownloadEntry.from_meta(logging_file_info, logging_file, name=logging_file_info["id"])
            if not path.isfile(logging_file) or path.getsize(logging_file) != download_entry.size:
                self.dl.append(download_entry)
            self.logging_file = logging_file
            self.logging_argument = client_logging["argument"]

    def prepare_libraries(self):

        """
        Must be called once metadata file are prepared, using `prepare_meta`, if not, `ValueError` is raised.\n
        If the version JAR file is not set, a ValueError is raised because it is required to be added in classpath.\n
        This method check all libraries found in the metadata, each library is downloaded if not already stored. Real
        Java libraries are added to the classpath list and native libraries are added to the native list.
        """

        self._check_version_meta()

        if self.version_jar_file is None:
            raise ValueError("The version JAR file is not ")

        self.classpath_libs.clear()
        self.classpath_libs.append(self.version_jar_file)
        self.native_libs.clear()

        for lib_obj in self.version_meta["libraries"]:

            if "rules" in lib_obj:
                if not interpret_rule(lib_obj["rules"]):
                    continue

            lib_name: str = lib_obj["name"]
            lib_dl_name = lib_name
            lib_natives: Optional[dict] = lib_obj.get("natives")

            if lib_natives is not None:
                lib_classifier = lib_natives.get(get_minecraft_os())
                if lib_classifier is None:
                    continue  # If natives are defined, but the OS is not supported, skip.
                lib_dl_name += f":{lib_classifier}"
                archbits = get_minecraft_archbits()
                if len(archbits):
                    lib_classifier = lib_classifier.replace("${arch}", archbits)
                lib_libs = self.native_libs
            else:
                lib_classifier = None
                lib_libs = self.classpath_libs

            lib_path: Optional[str] = None
            lib_dl_entry: Optional[DownloadEntry] = None
            lib_dl: Optional[dict] = lib_obj.get("downloads")

            if lib_dl is not None:

                if lib_classifier is not None:
                    lib_dl_classifiers = lib_dl.get("classifiers")
                    lib_dl_meta = None if lib_dl_classifiers is None else lib_dl_classifiers.get(lib_classifier)
                else:
                    lib_dl_meta = lib_dl.get("artifact")

                if lib_dl_meta is not None:
                    lib_path = path.join(self.context.libraries_dir, lib_dl_meta["path"])
                    lib_dl_entry = DownloadEntry.from_meta(lib_dl_meta, lib_path, name=lib_dl_name)

            if lib_dl_entry is None:

                lib_name_parts = lib_name.split(":")
                if len(lib_name_parts) != 3:
                    continue  # If the library name is not maven-formatted, skip.

                vendor, package, version = lib_name_parts
                jar_file = f"{package}-{version}.jar" if lib_classifier is None else f"{package}-{version}-{lib_classifier}.jar"
                lib_path_raw = "/".join((*vendor.split("."), package, version, jar_file))
                lib_path = path.join(self.context.libraries_dir, lib_path_raw)

                if not path.isfile(lib_path):
                    lib_repo_url: Optional[str] = lib_obj.get("url")
                    if lib_repo_url is None:
                        continue  # If the file doesn't exists, and no server url is provided, skip.
                    lib_dl_entry = DownloadEntry(f"{lib_repo_url}{lib_path_raw}", lib_path, name=lib_dl_name)

            lib_libs.append(lib_path)
            if lib_dl_entry is not None and (not path.isfile(lib_path) or path.getsize(lib_path) != lib_dl_entry.size):
                self.dl.append(lib_dl_entry)

    def prepare_jvm(self):

        """
        Must be called once metadata file are prepared, using `prepare_meta`, if not, `ValueError` is raised.\n
        This method ensure that the JVM adapted to this version is downloaded to the `jvm_dir` of the context.\n
        This method can raise `JvmLoadingError` with `JvmLoadingError.UNSUPPORTED_ARCH` if Mojang does not provide
        a JVM for your current architecture, or `JvmLoadingError.UNSUPPORTED_VERSION` if the required JVM version is
        not provided by Mojang. It can also raise `JsonRequestError` when failing to get JSON files.\n
        """

        self._check_version_meta()
        jvm_version_type = self.version_meta.get("javaVersion", {}).get("component", "jre-legacy")

        all_jvm_meta = json_simple_request("https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json")
        jvm_arch_meta = all_jvm_meta.get(get_minecraft_jvm_os())
        if jvm_arch_meta is None:
            raise JvmLoadingError(JvmLoadingError.UNSUPPORTED_ARCH)

        jvm_meta = jvm_arch_meta.get(jvm_version_type)
        if jvm_meta is None:
            raise JvmLoadingError(JvmLoadingError.UNSUPPORTED_VERSION)

        jvm_dir = path.join(self.context.jvm_dir, jvm_version_type)
        jvm_manifest = json_simple_request(jvm_meta[0]["manifest"]["url"])["files"]
        self.jvm_version = jvm_meta[0]["version"]["name"]
        self.jvm_exec = path.join(jvm_dir, "bin", "javaw.exe" if sys.platform == "win32" else "java")

        if not path.isfile(self.jvm_exec):

            jvm_exec_files = []
            os.makedirs(jvm_dir, exist_ok=True)
            for jvm_file_path_suffix, jvm_file in jvm_manifest.items():
                if jvm_file["type"] == "file":
                    jvm_file_path = path.join(jvm_dir, jvm_file_path_suffix)
                    jvm_download_info = jvm_file["downloads"]["raw"]
                    self.dl.append(DownloadEntry.from_meta(jvm_download_info, jvm_file_path, name=jvm_file_path_suffix))
                    if jvm_file.get("executable", False):
                        jvm_exec_files.append(jvm_file_path)

            def finalize():
                for exec_file in jvm_exec_files:
                    os.chmod(exec_file, 0o777)

            self.dl.add_callback(finalize)

    def download(self, *, progress_callback: 'Optional[Callable[[DownloadProgress], None]]' = None):
        """ Download all missing files computed in `prepare_` methods. """
        self.dl.download_files(progress_callback=progress_callback)
        self.dl.reset()

    def install(self, *, jvm: bool = False):
        """ Prepare (meta, jar, assets, logger, libs, jvm) and download the version with optional JVM installation. """
        self.prepare_meta()
        self.prepare_jar()
        self.prepare_assets()
        self.prepare_logger()
        self.prepare_libraries()
        if jvm:
            self.prepare_jvm()
        self.download()

    def start(self, opts: 'Optional[StartOptions]' = None):
        """ Faster method to start the version. This actually use `Start` class, however, you can use it directly. """
        start = Start(self)
        start.prepare(opts or StartOptions())
        start.start()


class StartOptions:

    def __init__(self):
        self.auth_session: Optional[AuthSession] = None
        self.uuid: Optional[str] = None
        self.username: Optional[str] = None
        self.demo: bool = False
        self.resolution: Optional[Tuple[int, int]] = None
        self.disable_multiplayer: bool = False
        self.disable_chat: bool = False
        self.server_address: Optional[str] = None
        self.server_port: Optional[int] = None
        self.jvm_exec: Optional[str] = None
        self.features: Dict[str, bool] = {}  # Additional features

    @classmethod
    def with_online(cls, auth_session: 'AuthSession') -> 'StartOptions':
        opts = StartOptions()
        opts.auth_session = auth_session
        return opts

    @classmethod
    def with_offline(cls, username: Optional[str], uuid: Optional[str]) -> 'StartOptions':
        opts = StartOptions()
        opts.username = username
        opts.uuid = uuid
        return opts


class Start:

    """
    Class used to control the starting procedure of Minecraft, it is made in order to allow the user to customize
    every argument given to the executable.
    """

    def __init__(self, version: Version):

        self.version = version

        self.args_replacements: Dict[str, str] = {}
        self.main_class: Optional[str] = None
        self.jvm_args: List[str] = []
        self.game_args: List[str] = []

        self.bin_dir_factory: Callable[[str], str] = self.default_bin_dir_factory
        self.runner: Callable[[List[str], str], None] = self.default_runner

    def _check_version(self):
        if self.version.version_meta is None:
            raise ValueError("You should install the version metadata first.")

    def get_username(self) -> str:
        return self.args_replacements.get("auth_player_name", "n/a")

    def get_uuid(self) -> str:
        return self.args_replacements.get("auth_uuid", "n/a")

    def prepare(self, opts: StartOptions):

        """
        This method is used to prepare internal arguments arrays, main class and arguments variables according to the
        version of this object and the given options. After this method you can call multiple times the `start` method.
        However before calling the `start` method you can changer `args_replacements`, `main_class`, `jvm_args`,
        `game_args`.\n
        This method can raise a `ValueError` if the version metadata has no `mainClass` or if no JVM executable was set
        in the given options nor downloaded by `Version` instance. You can ignore these errors if you ensure that
        """

        self._check_version()

        # Main class
        self.main_class = self.version.version_meta.get("mainClass")
        if self.main_class is None:
            raise ValueError("The version metadata has no main class to start.")

        # Prepare JVM exec
        jvm_exec = opts.jvm_exec
        if jvm_exec is None:
            jvm_exec = self.version.jvm_exec
            if jvm_exec is None:
                raise ValueError("No JVM executable set in options or downloaded by the version.")

        # Features
        features = {
            "is_demo_user": opts.demo,
            "has_custom_resolution": opts.resolution is not None,
            **opts.features
        }

        # Auth
        if opts.auth_session is not None:
            uuid = opts.auth_session.uuid
            username = opts.auth_session.username
        else:
            uuid = uuid4().hex if opts.uuid is None else opts.uuid.replace("-", "").lower()
            username = uuid[:8] if opts.username is None else opts.username[:16]  # Max username length is 16

        # Arguments replacements
        self.args_replacements = {
            # Game
            "auth_player_name": username,
            "version_name": self.version.id,
            "game_directory": self.version.context.work_dir,
            "assets_root": self.version.context.assets_dir,
            "assets_index_name": self.version.assets_index_version,
            "auth_uuid": uuid,
            "auth_access_token": "" if opts.auth_session is None else opts.auth_session.format_token_argument(False),
            "user_type": "mojang",
            "version_type": self.version.version_meta.get("type", ""),
            # Game (legacy)
            "auth_session": "" if opts.auth_session is None else opts.auth_session.format_token_argument(True),
            "game_assets": self.version.assets_virtual_dir,
            "user_properties": "{}",
            # JVM
            "natives_directory": "",
            "launcher_name": LAUNCHER_NAME,
            "launcher_version": LAUNCHER_VERSION,
            "classpath": path.pathsep.join(self.version.classpath_libs)
        }

        if opts.resolution is not None:
            self.args_replacements["resolution_width"] = str(opts.resolution[0])
            self.args_replacements["resolution_height"] = str(opts.resolution[1])

        # Arguments
        modern_args = self.version.version_meta.get("arguments", {})
        modern_jvm_args = modern_args.get("jvm")
        modern_game_args = modern_args.get("game")

        self.jvm_args.clear()
        self.game_args.clear()

        # JVM arguments
        self.jvm_args.append(jvm_exec)
        interpret_args(LEGACY_JVM_ARGUMENTS if modern_jvm_args is None else modern_jvm_args, features, self.jvm_args)

        # JVM argument for logging config
        if self.version.logging_argument is not None and self.version.logging_file is not None:
            self.jvm_args.append(self.version.logging_argument.replace("${path}", self.version.logging_file))

        # JVM argument for launch wrapper JAR path
        if self.main_class == "net.minecraft.launchwrapper.Launch":
            self.jvm_args.append(f"-Dminecraft.client.jar={self.version.version_jar_file}")

        # Game arguments
        if modern_game_args is None:
            self.game_args.extend(self.version.version_meta.get("minecraftArguments", "").split(" "))
        else:
            interpret_args(modern_game_args, features, self.game_args)

        if opts.disable_multiplayer:
            self.game_args.append("--disableMultiplayer")
        if opts.disable_chat:
            self.game_args.append("--disableChat")

        if opts.server_address is not None:
            self.game_args.extend(("--server", opts.server_address))
        if opts.server_port is not None:
            self.game_args.extend(("--port", str(opts.server_port)))

    def start(self):

        """
        Start the game using configured attributes `args_replacements`, `main_class`, `jvm_args`, `game_args`.
        You can easily configure these attributes with the `prepare` method.\n
        This method actually use the `bin_dir_factory` of this object to produce a path where to extract binaries, by
        default a random UUID is appended to the common `bin_dir` of the context. The `runner` argument is also used to
        run the game, by default is uses the `subprocess.run` method. These two attributes can be changed before calling
        this method.
        """

        if self.main_class is None:
            raise ValueError("Main class should be set before starting the game.")

        bin_dir = self.bin_dir_factory(self.version.context.bin_dir)
        cleaned = False

        def cleanup():
            nonlocal cleaned
            if not cleaned:
                shutil.rmtree(bin_dir, ignore_errors=True)
                cleaned = True

        import atexit
        atexit.register(cleanup)

        for native_lib in self.version.native_libs:
            with ZipFile(native_lib, "r") as native_zip:
                for native_zip_info in native_zip.infolist():
                    if can_extract_native(native_zip_info.filename):
                        native_zip.extract(native_zip_info, bin_dir)

        self.args_replacements["natives_directory"] = bin_dir

        self.runner([
            *replace_list_vars(self.jvm_args, self.args_replacements),
            self.main_class,
            *replace_list_vars(self.game_args, self.args_replacements)
        ], self.version.context.work_dir)

        cleanup()

    @staticmethod
    def default_bin_dir_factory(common_bin_dir: str) -> str:
        return path.join(common_bin_dir, str(uuid4()))

    @staticmethod
    def default_runner(args: List[str], cwd: str) -> None:
        import subprocess
        subprocess.run(args, cwd=cwd)


class VersionManifest:

    def __init__(self, data: dict):
        self.data = data

    @classmethod
    def load_from_url(cls):
        """ Load the version manifest from the official URL. Can raise `JsonRequestError` if failed. """
        return cls(json_simple_request("https://launchermeta.mojang.com/mc/game/version_manifest.json"))

    def filter_latest(self, version: str) -> Tuple[str, bool]:
        latest = self.data["latest"].get(version)
        return (version, False) if latest is None else (latest, True)

    def get_version(self, version: str) -> Optional[dict]:
        version, _alias = self.filter_latest(version)
        for version_data in self.data["versions"]:
            if version_data["id"] == version:
                return version_data
        return None

    def all_versions(self) -> list:
        return self.data["versions"]


class AuthSession:

    type = "raw"
    fields = "access_token", "username", "uuid"

    def __init__(self, access_token: str, username: str, uuid: str):
        self.access_token = access_token
        self.username = username
        self.uuid = uuid

    def format_token_argument(self, legacy: bool) -> str:
        return f"token:{self.access_token}:{self.uuid}" if legacy else self.access_token

    def validate(self) -> bool:
        return True

    def refresh(self):
        pass

    def invalidate(self):
        pass


class YggdrasilAuthSession(AuthSession):

    type = "yggdrasil"
    fields = "access_token", "username", "uuid", "client_token"

    def __init__(self, access_token: str, username: str, uuid: str, client_token: str):
        super().__init__(access_token, username, uuid)
        self.client_token = client_token

    def validate(self) -> bool:
        return self.request("validate", {
            "accessToken": self.access_token,
            "clientToken": self.client_token
        }, False)[0] == 204

    def refresh(self):
        _, res = self.request("refresh", {
            "accessToken": self.access_token,
            "clientToken": self.client_token
        })
        self.access_token = res["accessToken"]
        self.username = res["selectedProfile"]["name"]  # Refresh username if renamed (does it works? to check.).

    def invalidate(self):
        self.request("invalidate", {
            "accessToken": self.access_token,
            "clientToken": self.client_token
        }, False)

    @classmethod
    def authenticate(cls, email: str, password: str) -> 'YggdrasilAuthSession':
        _, res = cls.request("authenticate", {
            "agent": {
                "name": "Minecraft",
                "version": 1
            },
            "username": email,
            "password": password,
            "clientToken": uuid4().hex
        })
        return cls(res["accessToken"], res["selectedProfile"]["name"], res["selectedProfile"]["id"], res["clientToken"])

    @classmethod
    def request(cls, req: str, payload: dict, error: bool = True) -> Tuple[int, dict]:
        code, res = json_request(f"https://authserver.mojang.com/{req}", "POST",
                                 data=json.dumps(payload).encode("ascii"),
                                 headers={"Content-Type": "application/json"},
                                 ignore_error=True)
        if error and code != 200:
            raise AuthError(AuthError.YGGDRASIL, res["errorMessage"])
        return code, res


class MicrosoftAuthSession(AuthSession):

    type = "microsoft"
    fields = "access_token", "username", "uuid", "refresh_token", "client_id", "redirect_uri"

    def __init__(self, access_token: str, username: str, uuid: str, refresh_token: str, client_id: str, redirect_uri: str):
        super().__init__(access_token, username, uuid)
        self.refresh_token = refresh_token
        self.client_id = client_id
        self.redirect_uri = redirect_uri
        self._new_username: Optional[str] = None

    def validate(self) -> bool:
        self._new_username = None
        code, res = self.mc_request_profile(self.access_token)
        if code == 200:
            username = res["name"]
            if self.username != username:
                self._new_username = username
                return False
            return True
        return False

    def refresh(self):
        if self._new_username is not None:
            self.username = self._new_username
            self._new_username = None
        else:
            res = self.authenticate_base({
                "client_id": self.client_id,
                "redirect_uri": self.redirect_uri,
                "refresh_token": self.refresh_token,
                "grant_type": "refresh_token",
                "scope": "xboxlive.signin"
            })
            self.access_token = res["access_token"]
            self.username = res["username"]
            self.uuid = res["uuid"]
            self.refresh_token = res["refresh_token"]

    @staticmethod
    def get_authentication_url(app_client_id: str, redirect_uri: str, email: str, nonce: str):
        return "https://login.live.com/oauth20_authorize.srf?{}".format(url_parse.urlencode({
            "client_id": app_client_id,
            "redirect_uri": redirect_uri,
            "response_type": "code id_token",
            "scope": "xboxlive.signin offline_access openid email",
            "login_hint": email,
            "nonce": nonce,
            "response_mode": "form_post"
        }))

    @staticmethod
    def get_logout_url(app_client_id: str, redirect_uri: str):
        return "https://login.live.com/oauth20_logout.srf?{}".format(url_parse.urlencode({
            "client_id": app_client_id,
            "redirect_uri": redirect_uri
        }))

    @classmethod
    def check_token_id(cls, token_id: str, email: str, nonce: str) -> bool:
        id_token_payload = json.loads(cls.base64url_decode(token_id.split(".")[1]))
        return id_token_payload["nonce"] == nonce and id_token_payload["email"] == email

    @classmethod
    def authenticate(cls, app_client_id: str, code: str, redirect_uri: str) -> 'MicrosoftAuthSession':
        res = cls.authenticate_base({
            "client_id": app_client_id,
            "redirect_uri": redirect_uri,
            "code": code,
            "grant_type": "authorization_code",
            "scope": "xboxlive.signin"
        })
        return cls(res["access_token"], res["username"], res["uuid"], res["refresh_token"], app_client_id, redirect_uri)

    @classmethod
    def authenticate_base(cls, request_token_payload: dict) -> dict:

        # Microsoft OAuth
        _, res = cls.ms_request("https://login.live.com/oauth20_token.srf", request_token_payload, payload_url_encoded=True)
        ms_refresh_token = res["refresh_token"]

        # Xbox Live Token
        _, res = cls.ms_request("https://user.auth.xboxlive.com/user/authenticate", {
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": "d={}".format(res["access_token"])
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        })

        xbl_token = res["Token"]
        xbl_user_hash = res["DisplayClaims"]["xui"][0]["uhs"]

        # Xbox Live XSTS Token
        _, res = cls.ms_request("https://xsts.auth.xboxlive.com/xsts/authorize", {
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        })
        xsts_token = res["Token"]

        if xbl_user_hash != res["DisplayClaims"]["xui"][0]["uhs"]:
            raise AuthError(AuthError.MICROSOFT_INCONSISTENT_USER_HASH)

        # MC Services Auth
        _, res = cls.ms_request("https://api.minecraftservices.com/authentication/login_with_xbox", {
            "identityToken": f"XBL3.0 x={xbl_user_hash};{xsts_token}"
        })
        mc_access_token = res["access_token"]

        # MC Services Profile
        code, res = cls.mc_request_profile(mc_access_token)

        if code == 404:
            raise AuthError(AuthError.MICROSOFT_DOES_NOT_OWN_MINECRAFT)
        elif code == 401:
            raise AuthError(AuthError.MICROSOFT_OUTDATED_TOKEN)
        elif "error" in res or code != 200:
            raise AuthError(AuthError.MICROSOFT, res.get("errorMessage", res.get("error", "Unknown error")))

        return {
            "refresh_token": ms_refresh_token,
            "access_token": mc_access_token,
            "username": res["name"],
            "uuid": res["id"]
        }

    @classmethod
    def ms_request(cls, url: str, payload: dict, *, payload_url_encoded: bool = False) -> Tuple[int, dict]:
        data = (url_parse.urlencode(payload) if payload_url_encoded else json.dumps(payload)).encode("ascii")
        content_type = "application/x-www-form-urlencoded" if payload_url_encoded else "application/json"
        return json_request(url, "POST", data=data, headers={"Content-Type": content_type})

    @classmethod
    def mc_request_profile(cls, bearer: str) -> Tuple[int, dict]:
        url = "https://api.minecraftservices.com/minecraft/profile"
        return json_request(url, "GET", headers={"Authorization": f"Bearer {bearer}"})

    @classmethod
    def base64url_decode(cls, s: str) -> bytes:
        rem = len(s) % 4
        if rem > 0:
            s += "=" * (4 - rem)
        return base64.urlsafe_b64decode(s)


class AuthDatabase:

    types = {
        YggdrasilAuthSession.type: YggdrasilAuthSession,
        MicrosoftAuthSession.type: MicrosoftAuthSession
    }

    def __init__(self, filename: str, legacy_filename: str):
        self.filename = filename
        self.legacy_filename = legacy_filename
        self.sessions: Dict[str, Dict[str, AuthSession]] = {}

    def load(self):
        self.sessions.clear()
        if not path.isfile(self.filename):
            self._load_legacy_and_delete()
        try:
            with open(self.filename, "rb") as fp:
                data = json.load(fp)
                for typ, typ_data in data.items():
                    if typ not in self.types:
                        continue
                    sess_type = self.types[typ]
                    sessions = self.sessions[typ] = {}
                    sessions_data = typ_data["sessions"]
                    for email, sess_data in sessions_data.items():
                        sess_params = []
                        for field in sess_type.fields:
                            sess_params.append(sess_data.get(field, ""))
                        sessions[email] = sess_type(*sess_params)
        except (OSError, KeyError, TypeError, JSONDecodeError):
            pass

    def _load_legacy_and_delete(self):
        try:
            with open(self.legacy_filename, "rt") as fp:
                for line in fp.readlines():
                    parts = line.split(" ")
                    if len(parts) == 5:
                        self.put(parts[0], YggdrasilAuthSession(parts[4], parts[2], parts[3], parts[1]))
            os.remove(self.legacy_filename)
        except OSError:
            pass

    def save(self):
        with open(self.filename, "wt") as fp:
            data = {}
            for typ, sessions in self.sessions.items():
                if typ not in self.types:
                    continue
                sess_type = self.types[typ]
                sessions_data = {}
                data[typ] = {"sessions": sessions_data}
                for email, sess in sessions.items():
                    sess_data = sessions_data[email] = {}
                    for field in sess_type.fields:
                        sess_data[field] = getattr(sess, field)
            json.dump(data, fp, indent=2)

    def get(self, email: str, sess_type: Type[AuthSession]) -> Optional[AuthSession]:
        sessions = self.sessions.get(sess_type.type)
        return None if sessions is None else sessions.get(email)

    def put(self, email: str, sess: AuthSession):
        sessions = self.sessions.get(sess.type)
        if sessions is None:
            if sess.type not in self.types:
                raise ValueError("Given session's type is not supported.")
            sessions = self.sessions[sess.type] = {}
        sessions[email] = sess

    def remove(self, email: str, sess_type: Type[AuthSession]) -> Optional[AuthSession]:
        sessions = self.sessions.get(sess_type.type)
        if sessions is not None:
            session = sessions.get(email)
            if session is not None:
                del sessions[email]
                return session


class DownloadEntry:

    __slots__ = "url", "size", "sha1", "dst", "name"

    def __init__(self, url: str, dst: str, *, size: Optional[int] = None, sha1: Optional[str] = None, name: Optional[str] = None):
        self.url = url
        self.dst = dst
        self.size = size
        self.sha1 = sha1
        self.name = url if name is None else name

    @classmethod
    def from_meta(cls, info: dict, dst: str, *, name: Optional[str] = None) -> 'DownloadEntry':
        return DownloadEntry(info["url"], dst, size=info["size"], sha1=info["sha1"], name=name)


class DownloadList:

    __slots__ = "entries", "callbacks", "count", "size"

    def __init__(self):
        self.entries: Dict[str, List[DownloadEntry]] = {}
        self.callbacks: List[Callable[[], None]] = []
        self.count = 0
        self.size = 0

    def append(self, entry: DownloadEntry):
        url_parsed = url_parse.urlparse(entry.url)
        if url_parsed.scheme not in ("http", "https"):
            raise ValueError("Illegal URL scheme for HTTP connection.")
        host_key = f"{int(url_parsed.scheme == 'https')}{url_parsed.netloc}"
        entries = self.entries.get(host_key)
        if entries is None:
            self.entries[host_key] = entries = []
        entries.append(entry)
        self.count += 1
        if entry.size is not None:
            self.size += entry.size

    def reset(self):
        self.entries.clear()
        self.callbacks.clear()

    def add_callback(self, callback: Callable[[], None]):
        self.callbacks.append(callback)

    def download_files(self, *, progress_callback: 'Optional[Callable[[DownloadProgress], None]]' = None):

        """
        Downloads the given list of files. Even if some downloads fails, it continue and raise DownloadError(fails)
        only at the end (but not calling callbacks), where 'fails' is a dict associating the entry URL and its error
        ('not_found', 'invalid_size', 'invalid_sha1').
        """

        if len(self.entries):

            headers = {}
            buffer = bytearray(65536)
            total_size = 0
            fails: Dict[str, str] = {}
            max_try_count = 3

            if progress_callback is not None:
                progress = DownloadProgress(self.size)
                entry_progress = DownloadEntryProgress()
                progress.entries.append(entry_progress)
            else:
                progress = None
                entry_progress = None

            for host, entries in self.entries.items():

                conn_type = HTTPSConnection if (host[0] == "1") else HTTPConnection
                conn = conn_type(host[1:])
                max_entry_idx = len(entries) - 1
                headers["Connection"] = "keep-alive"

                for i, entry in enumerate(entries):

                    last_entry = (i == max_entry_idx)
                    if last_entry:
                        headers["Connection"] = "close"

                    size_target = 0 if entry.size is None else entry.size
                    error = None

                    for _ in range(max_try_count):

                        try:
                            conn.request("GET", entry.url, None, headers)
                            res = conn.getresponse()
                        except ConnectionError:
                            error = DownloadError.CONN_ERROR
                            continue

                        if res.status != 200:
                            error = DownloadError.NOT_FOUND
                            continue

                        sha1 = None if entry.sha1 is None else hashlib.sha1()
                        size = 0

                        os.makedirs(path.dirname(entry.dst), exist_ok=True)
                        with open(entry.dst, "wb") as dst_fp:
                            while True:
                                read_len = res.readinto(buffer)
                                if not read_len:
                                    break
                                buffer_view = buffer[:read_len]
                                size += read_len
                                total_size += read_len
                                if sha1 is not None:
                                    sha1.update(buffer_view)
                                dst_fp.write(buffer_view)
                                if progress_callback is not None:
                                    progress.size = total_size
                                    entry_progress.name = entry.name
                                    entry_progress.total = size_target
                                    entry_progress.size = size
                                    progress_callback(progress)

                        if entry.size is not None and size != entry.size:
                            error = DownloadError.INVALID_SIZE
                        elif entry.sha1 is not None and sha1.hexdigest() != entry.sha1:
                            error = DownloadError.INVALID_SHA1
                        else:
                            break

                        total_size -= size  # If error happened, subtract the size and restart from latest total_size.

                    else:
                        fails[entry.url] = error  # If the break was not triggered, an error should be set.

                conn.close()

            if len(fails):
                raise DownloadError(fails)

        for callback in self.callbacks:
            callback()


class DownloadEntryProgress:

    __slots__ = "name", "size", "total"

    def __init__(self):
        self.name = ""
        self.size = 0
        self.total = 0


class DownloadProgress:

    __slots__ = "entries", "size", "total"

    def __init__(self, total: int):
        self.entries: List[DownloadEntryProgress] = []
        self.size: int = 0  # Size can be greater that total, this happen if any DownloadEntry has an unknown size.
        self.total = total


class BaseError(Exception):

    def __init__(self, code: str):
        super().__init__()
        self.code = code


class JsonRequestError(BaseError):

    INVALID_RESPONSE_NOT_JSON = "invalid_response_not_json"

    def __init__(self, code: str, details: str):
        super().__init__(code)
        self.details = details


class AuthError(BaseError):

    YGGDRASIL = "yggdrasil"
    MICROSOFT = "microsoft"
    MICROSOFT_INCONSISTENT_USER_HASH = "microsoft.inconsistent_user_hash"
    MICROSOFT_DOES_NOT_OWN_MINECRAFT = "microsoft.does_not_own_minecraft"
    MICROSOFT_OUTDATED_TOKEN = "microsoft.outdated_token"

    def __init__(self, code: str, details: Optional[str] = None):
        super().__init__(code)
        self.details = details


class VersionError(BaseError):

    NOT_FOUND = "not_found"
    TO_MUCH_PARENTS = "to_much_parents"
    JAR_NOT_FOUND = "jar_not_found"

    def __init__(self, code: str, version: str):
        super().__init__(code)
        self.version = version


class JvmLoadingError(BaseError):
    UNSUPPORTED_ARCH = "unsupported_arch"
    UNSUPPORTED_VERSION = "unsupported_version"


class DownloadError(Exception):

    CONN_ERROR = "conn_error"
    NOT_FOUND = "not_found"
    INVALID_SIZE = "invalid_size"
    INVALID_SHA1 = "invalid_sha1"

    def __init__(self, fails: Dict[str, str]):
        super().__init__()
        self.fails = fails


def json_request(url: str, method: str, *,
                 data: Optional[bytes] = None,
                 headers: Optional[dict] = None,
                 ignore_error: bool = False,
                 timeout: Optional[float] = None) -> Tuple[int, dict]:

    """
    Make a request for a JSON API at specified URL. Might raise `JsonRequestError` if failed.\n
    The parameter `ignore_error` can be used to ignore JSONDecodeError handling and just return a dict with a
    single key 'raw' and the raw data on failure, instead of raising an `JsonRequestError` with
    `JsonRequestError.INVALID_RESPONSE_NOT_JSON`.
    """

    if headers is None:
        headers = {}
    if "Accept" not in headers:
        headers["Accept"] = "application/json"

    try:
        req = UrlRequest(url, data, headers, method=method)
        res: HTTPResponse = url_request.urlopen(req, timeout=timeout)
    except HTTPError as err:
        res = cast(HTTPResponse, err)

    try:
        data = res.read()
        return res.status, json.loads(data)
    except JSONDecodeError:
        if ignore_error:
            return res.status, {"raw": data}
        else:
            raise JsonRequestError(JsonRequestError.INVALID_RESPONSE_NOT_JSON, str(res.status))


def json_simple_request(url: str, *, ignore_error: bool = False, timeout: Optional[int] = None) -> dict:
    """ Make a GET request for a JSON API at specified URL. Might raise `JsonRequestError` if failed. """
    return json_request(url, "GET", ignore_error=ignore_error, timeout=timeout)[1]


def merge_dict(dst: dict, other: dict):

    """
    Merge the 'other' dict into the 'dst' dict. For every key/value in 'other', if the key is present in 'dst'
    it does nothing. Unless values in both dict are also dict, in this case the merge is recursive. If the
    value in both dict are list, the 'dst' list is extended (.extend()) with the one of 'other'.
    """

    for k, v in other.items():
        if k in dst:
            if isinstance(dst[k], dict) and isinstance(other[k], dict):
                merge_dict(dst[k], other[k])
            elif isinstance(dst[k], list) and isinstance(other[k], list):
                dst[k].extend(other[k])
        else:
            dst[k] = other[k]


def interpret_rule_os(rule_os: dict) -> bool:
    os_name = rule_os.get("name")
    if os_name is None or os_name == get_minecraft_os():
        os_arch = rule_os.get("arch")
        if os_arch is None or os_arch == get_minecraft_arch():
            os_version = rule_os.get("version")
            if os_version is None or re.search(os_version, platform.version()) is not None:
                return True
    return False


def interpret_rule(rules: List[dict], features: Optional[dict] = None) -> bool:
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


def interpret_args(args: list, features: dict, dst: List[str]):
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


def replace_vars(txt: str, replacements: Dict[str, str]) -> str:
    return txt.replace("${", "{").format_map(replacements)


def replace_list_vars(lst: List[str], replacements: Dict[str, str]) -> Generator[str, None, None]:
    return (replace_vars(elt, replacements) for elt in lst)


def get_minecraft_dir() -> str:
    home = path.expanduser("~")
    return {
        "Linux": path.join(home, ".minecraft"),
        "Windows": path.join(home, "AppData", "Roaming", ".minecraft"),
        "Darwin": path.join(home, "Library", "Application Support", "minecraft")
    }.get(platform.system())


_minecraft_os: Optional[str] = None
def get_minecraft_os() -> str:
    """ Return the current OS identifier used in rules matching, 'linux', 'windows', 'osx' and '' if not found. """
    global _minecraft_os
    if _minecraft_os is None:
        _minecraft_os = {"Linux": "linux", "Windows": "windows", "Darwin": "osx"}.get(platform.system(), "")
    return _minecraft_os


_minecraft_arch: Optional[str] = None
def get_minecraft_arch() -> str:
    """ Return the architecture to use in rules matching, 'x86', 'x86_64' or '' if not found. """
    global _minecraft_arch
    if _minecraft_arch is None:
        machine = platform.machine().lower()
        _minecraft_arch = "x86" if machine in ("i386", "i686") else "x86_64" if machine in ("x86_64", "amd64", "ia64") else ""
    return _minecraft_arch


_minecraft_archbits: Optional[str] = None
def get_minecraft_archbits() -> str:
    """ Return the address size of the architecture used for rules matching, '64', '32', or '' if not found. """
    global _minecraft_archbits
    if _minecraft_archbits is None:
        raw_bits = platform.architecture()[0]
        _minecraft_archbits = "64" if raw_bits == "64bit" else "32" if raw_bits == "32bit" else ""
    return _minecraft_archbits


_minecraft_jvm_os: Optional[str] = None
def get_minecraft_jvm_os() -> str:
    """ Return the OS identifier used to choose the right JVM to download. """
    global _minecraft_jvm_os
    if _minecraft_jvm_os is None:
        _minecraft_jvm_os = {
            "osx": {"x86": "mac-os"},
            "linux": {"x86": "linux-i386", "x86_64": "linux"},
            "windows": {"x86": "windows-x86", "x86_64": "windows-x64"}
        }.get(get_minecraft_os(), {}).get(get_minecraft_arch())
    return _minecraft_jvm_os


def can_extract_native(filename: str) -> bool:
    """ Return True if a file should be extracted to binaries directory. """
    return not filename.startswith("META-INF") and not filename.endswith(".git") and not filename.endswith(".sha1")


LEGACY_JVM_ARGUMENTS = [
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
