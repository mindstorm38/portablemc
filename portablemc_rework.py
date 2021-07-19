#!/usr/bin/env python
# encoding: utf8

# PortableMC is a portable Minecraft launcher in only one Python script (without addons).
# Copyright (C) 2021 Théo Rozier
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

from typing import Generator, Callable, Optional, Tuple, Dict, Type, List
from http.client import HTTPConnection, HTTPSConnection
from urllib import parse as url_parse
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


LAUNCHER_NAME = "portablemc"
LAUNCHER_VERSION = "1.2.0"
LAUNCHER_AUTHORS = "Théo Rozier"

VERSION_MANIFEST_URL = "https://launchermeta.mojang.com/mc/game/version_manifest.json"
ASSET_BASE_URL = "https://resources.download.minecraft.net/{}/{}"
AUTHSERVER_URL = "https://authserver.mojang.com/{}"
JVM_META_URL = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json"

MS_OAUTH_CODE_URL = "https://login.live.com/oauth20_authorize.srf"
MS_OAUTH_LOGOUT_URL = "https://login.live.com/oauth20_logout.srf"
MS_OAUTH_TOKEN_URL = "https://login.live.com/oauth20_token.srf"
MS_XBL_AUTH_DOMAIN = "user.auth.xboxlive.com"
MS_XBL_AUTH_URL = "https://user.auth.xboxlive.com/user/authenticate"
MS_XSTS_AUTH_URL = "https://xsts.auth.xboxlive.com/xsts/authorize"
MS_GRAPH_UPN_REQUEST_URL = "https://graph.microsoft.com/v1.0/me?$select=userPrincipalName"
MC_AUTH_URL = "https://api.minecraftservices.com/authentication/login_with_xbox"
MC_PROFILE_URL = "https://api.minecraftservices.com/minecraft/profile"


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

        main_dir = Util.get_minecraft_dir() if main_dir is None else path.realpath(main_dir)
        self.work_dir = main_dir if work_dir is None else path.realpath(work_dir)
        self.versions_dir = path.join(main_dir, "versions")
        self.assets_dir = path.join(main_dir, "assets")
        self.libraries_dir = path.join(main_dir, "libraries")
        self.jvm_dir = path.join(main_dir, "jvm")
        self.bin_dir = path.join(self.work_dir, "bin")

    def has_version_metadata(self, version: str) -> bool:
        """ Return True if the given version has a metadata file. """
        return path.isfile(path.join(self.versions_dir, version, f"{version}.json"))

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

    def __init__(self, context: Context, version: str):

        """ Construct a new version, using a specific context and the exact version ID you want to start. """

        self.context = context
        self.version = version

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

        if self.manifest is None:
            self.manifest = VersionManifest.load_from_url()

        version_meta, version_dir = self._prepare_meta_internal(self.version)
        while "inheritsFrom" in version_meta:
            if recursion_limit <= 0:
                raise VersionError(VersionError.TO_MUCH_PARENTS, self.version)
            recursion_limit -= 1
            parent_meta, _ = self._prepare_meta_internal(version_meta["inheritsFrom"])
            del version_meta["inheritsFrom"]
            Util.merge_dict(version_meta, parent_meta)

        self.version_meta, self.version_dir = version_meta, version_dir

    def _prepare_meta_internal(self, version: str) -> Tuple[dict, str]:

        version_dir = path.join(self.context.versions_dir, version)
        version_meta_file = path.join(version_dir, f"{version}.json")

        try:
            with open(version_meta_file, "rt") as version_meta_fp:
                return json.load(version_meta_fp), version_dir
        except (OSError, JSONDecodeError):
            version_super_meta = self.manifest.get_version(version)
            if version_super_meta is not None:
                content = Util.json_simple_request(version_super_meta["url"])
                os.makedirs(version_dir, exist_ok=True)
                with open(version_meta_file, "wt") as version_meta_fp:
                    json.dump(content, version_meta_fp, indent=2)
                return content, version_dir
            else:
                raise VersionError(VersionError.NOT_FOUND, version)

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
        self.version_jar_file = path.join(self.version_dir, f"{self.version}.jar")
        client_download = self.version_meta.get("downloads", {}).get("client")
        if client_download is not None:
            entry = DownloadEntry.from_meta(client_download, self.version_jar_file, name=f"{self.version}.jar")
            if not path.isfile(entry.dst) or path.getsize(entry.dst) != entry.size:
                self.dl.append(entry)
        elif not path.isfile(self.version_jar_file):
            raise VersionError(VersionError.JAR_NOT_FOUND, self.version)

    def prepare_assets(self):

        """
        Must be called once metadata file are prepared, using `prepare_meta`, if not, `ValueError` is raised.\n
        This method download the asset index file (if not already cached) named after the asset version into the
        directory `indexes` placed into the directory `assets_dir` of the context. Once ready, the asset index file
        is analysed and each object is checked, if it does not exist or not have the expected size, it is downloaded
        to the `objects` directory placed into the directory `assets_dir` of the context.\n
        This method also set the `assets_count` attribute with the number of assets for this version.\n
        This method can raise `JsonRequestError` if it fails to load the asset index file.
        """

        self._check_version_meta()

        assets_indexes_dir = path.join(self.context.assets_dir, "indexes")
        assets_index_version = self.version_meta["assets"]
        assets_index_file = path.join(assets_indexes_dir, f"{assets_index_version}.json")

        try:
            with open(assets_index_file, "rb") as assets_index_fp:
                assets_index = json.load(assets_index_fp)
        except (OSError, JSONDecodeError):
            asset_index_info = self.version_meta["assetIndex"]
            asset_index_url = asset_index_info["url"]
            assets_index = Util.json_simple_request(asset_index_url)
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
                asset_url = ASSET_BASE_URL.format(asset_hash_prefix, asset_hash)
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
                if not Util.interpret_rule(lib_obj["rules"]):
                    continue

            lib_name: str = lib_obj["name"]
            lib_dl_name = lib_name
            lib_natives: Optional[dict] = lib_obj.get("natives")

            if lib_natives is not None:
                lib_classifier = lib_natives.get(Util.get_minecraft_os())
                if lib_classifier is None:
                    continue  # If natives are defined, but the OS is not supported, skip.
                lib_dl_name += f":{lib_classifier}"
                archbits = Util.get_minecraft_archbits()
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

        all_jvm_meta = Util.json_simple_request(JVM_META_URL)
        jvm_arch_meta = all_jvm_meta.get(Util.get_minecraft_jvm_os())
        if jvm_arch_meta is None:
            raise JvmLoadingError(JvmLoadingError.UNSUPPORTED_ARCH)

        jvm_meta = jvm_arch_meta.get(jvm_version_type)
        if jvm_meta is None:
            raise JvmLoadingError(JvmLoadingError.UNSUPPORTED_VERSION)

        jvm_dir = path.join(self.context.jvm_dir, jvm_version_type)
        jvm_manifest = Util.json_simple_request(jvm_meta[0]["manifest"]["url"])["files"]
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
        self.runner: Callable[[List[str]], None] = self.default_runner

    def _check_version(self):
        if self.version.version_meta is None:
            raise ValueError("You should install the version metadata first.")

    def prepare(self, opts: StartOptions):

        """
        This method is used to prepare internal arguments arrays, main class and arguments variables according to the
        version of this object and the given options. After this method you can call multiple times the `start` method.
        However before calling the `start` method you can changer `args_replacements`, `main_class`, `jvm_args`,
        `game_args`.
        """

        self._check_version()

        # Main class
        self.main_class = self.version.version_meta.get("mainClass")
        if self.main_class is None:
            raise ValueError("The version metadata has no main class to start.")

        # Features
        features = {
            "is_demo_user": opts.demo,
            "has_custom_resolution": opts.resolution is not None
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
            "version_name": self.version.version,
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
        Util.interpret_args(Util.LEGACY_JVM_ARGUMENTS if modern_jvm_args is None else modern_jvm_args, features, self.jvm_args)

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
            Util.interpret_args(modern_game_args, features, self.game_args)

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
        Start the game using prevously configured attributes `args_replacements`, `main_class`, `jvm_args`, `game_args`.
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
                    if Util.can_extract_native(native_zip_info.filename):
                        native_zip.extract(native_zip_info, bin_dir)

        self.args_replacements["natives_directory"] = bin_dir

        self.runner([
            *Util.replace_list_vars(self.jvm_args, self.args_replacements),
            self.main_class,
            *Util.replace_list_vars(self.game_args, self.args_replacements)
        ])

        cleanup()

    @staticmethod
    def default_bin_dir_factory(common_bin_dir: str) -> str:
        return path.join(common_bin_dir, str(uuid4()))

    @staticmethod
    def default_runner(args: List[str]) -> None:
        import subprocess
        subprocess.run(args)


class VersionManifest:

    def __init__(self, data: dict):
        self._data = data

    @classmethod
    def load_from_url(cls):
        """ Load the version manifest from the official URL. Might raise `JsonRequestError` if failed. """
        return cls(Util.json_simple_request(VERSION_MANIFEST_URL))

    def filter_latest(self, version: str) -> Tuple[str, bool]:
        latest = self._data["latest"].get(version)
        return (version, False) if latest is None else (latest, True)

    def get_version(self, version: str) -> Optional[dict]:
        version, _alias = self.filter_latest(version)
        for version_data in self._data["versions"]:
            if version_data["id"] == version:
                return version_data
        return None

    def all_versions(self) -> list:
        return self._data["versions"]

    """def search_versions(self, inp: str) -> Generator[dict, None, None]:
        inp, alias = self.filter_latest(inp)
        for version_data in self._data["versions"]:
            if (alias and version_data["id"] == inp) or (not alias and inp in version_data["id"]):
                yield version_data"""


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
    def authenticate(cls, email_or_username: str, password: str) -> 'YggdrasilAuthSession':
        _, res = cls.request("authenticate", {
            "agent": {
                "name": "Minecraft",
                "version": 1
            },
            "username": email_or_username,
            "password": password,
            "clientToken": uuid4().hex
        })
        return cls(res["accessToken"], res["selectedProfile"]["name"], res["selectedProfile"]["id"], res["clientToken"])

    @classmethod
    def request(cls, req: str, payload: dict, error: bool = True) -> Tuple[int, dict]:
        code, res = Util.json_request(AUTHSERVER_URL.format(req), "POST",
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
        code, res = self.mc_request(MC_PROFILE_URL, self.access_token)
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
        return "{}?{}".format(MS_OAUTH_CODE_URL, url_parse.urlencode({
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
        return "{}?{}".format(MS_OAUTH_LOGOUT_URL, url_parse.urlencode({
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
        _, res = cls.ms_request(MS_OAUTH_TOKEN_URL, request_token_payload, payload_url_encoded=True)
        ms_refresh_token = res["refresh_token"]

        # Xbox Live Token
        _, res = cls.ms_request(MS_XBL_AUTH_URL, {
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": MS_XBL_AUTH_DOMAIN,
                "RpsTicket": "d={}".format(res["access_token"])
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        })

        xbl_token = res["Token"]
        xbl_user_hash = res["DisplayClaims"]["xui"][0]["uhs"]

        # Xbox Live XSTS Token
        _, res = cls.ms_request(MS_XSTS_AUTH_URL, {
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
        _, res = cls.ms_request(MC_AUTH_URL, {
            "identityToken": f"XBL3.0 x={xbl_user_hash};{xsts_token}"
        })
        mc_access_token = res["access_token"]

        # MC Services Profile
        code, res = cls.mc_request(MC_PROFILE_URL, mc_access_token)

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
        return Util.json_request(url, "POST", data=data, headers={"Content-Type": content_type})

    @classmethod
    def mc_request(cls, url: str, bearer: str) -> Tuple[int, dict]:
        return Util.json_request(url, "GET", headers={"Authorization": f"Bearer {bearer}"})

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
        self._filename = filename
        self._legacy_filename = legacy_filename
        self._sessions: Dict[str, Dict[str, AuthSession]] = {}

    def load(self):
        self._sessions.clear()
        if not path.isfile(self._filename):
            self._load_legacy_and_delete()
        try:
            with open(self._filename, "rb") as fp:
                data = json.load(fp)
                for typ, typ_data in data.items():
                    if typ not in self.types:
                        continue
                    sess_type = self.types[typ]
                    sessions = self._sessions[typ] = {}
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
            with open(self._legacy_filename, "rt") as fp:
                for line in fp.readlines():
                    parts = line.split(" ")
                    if len(parts) == 5:
                        self.put(parts[0], YggdrasilAuthSession(parts[4], parts[2], parts[3], parts[1]))
            os.remove(self._legacy_filename)
        except OSError:
            pass

    def save(self):
        with open(self._filename, "wt") as fp:
            data = {}
            for typ, sessions in self._sessions.items():
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

    def get(self, email_or_username: str, sess_type: Type[AuthSession]) -> Optional[AuthSession]:
        sessions = self._sessions.get(sess_type.type)
        return None if sessions is None else sessions.get(email_or_username)

    def put(self, email_or_username: str, sess: AuthSession):
        sessions = self._sessions.get(sess.type)
        if sessions is None:
            if sess.type not in self.types:
                raise ValueError("Given session's type is not supported.")
            sessions = self._sessions[sess.type] = {}
        sessions[email_or_username] = sess

    def remove(self, email_or_username: str, sess_type: Type[AuthSession]) -> Optional[AuthSession]:
        sessions = self._sessions.get(sess_type.type)
        if sessions is not None:
            session = sessions.get(email_or_username)
            if session is not None:
                del sessions[email_or_username]
                return session


class Util:

    minecraft_os: Optional[str] = None
    minecraft_arch: Optional[str] = None
    minecraft_archbits: Optional[str] = None
    minecraft_jvm_os: Optional[str] = None

    @staticmethod
    def json_request(url: str, method: str, *,
                     data: Optional[bytes] = None,
                     headers: Optional[dict] = None,
                     ignore_error: bool = False,
                     timeout: Optional[int] = None) -> Tuple[int, dict]:

        """ Make a request for a JSON API at specified URL. Might raise `JsonRequestError` if failed. """

        url_parsed = url_parse.urlparse(url)
        conn_type = {"http": HTTPConnection, "https": HTTPSConnection}.get(url_parsed.scheme)
        if conn_type is None:
            raise JsonRequestError(JsonRequestError.INVALID_URL_SCHEME, url_parsed.scheme)
        conn = conn_type(url_parsed.netloc, timeout=timeout)
        if headers is None:
            headers = {}
        if "Accept" not in headers:
            headers["Accept"] = "application/json"
        headers["Connection"] = "close"

        try:
            conn.request(method, url, data, headers)
            res = conn.getresponse()
            try:
                return res.status, json.load(res)
            except JSONDecodeError:
                if ignore_error:
                    return res.status, {}
                else:
                    raise JsonRequestError(JsonRequestError.INVALID_RESPONSE_NOT_JSON, str(res.status))
        except OSError as os_err:
            raise JsonRequestError(JsonRequestError.SOCKET_ERROR, str(os_err))
        finally:
            conn.close()

    @classmethod
    def json_simple_request(cls, url: str, *, ignore_error: bool = False, timeout: Optional[int] = None) -> dict:
        """ Make a GET request for a JSON API at specified URL. Might raise `JsonRequestError` if failed. """
        return cls.json_request(url, "GET", ignore_error=ignore_error, timeout=timeout)[1]

    @classmethod
    def merge_dict(cls, dst: dict, other: dict):
        """ Merge the 'other' dict into the 'dst' dict. For every key/value in 'other', if the key is present in 'dst'
        it does nothing. Unless values in both dict are also dict, in this case the merge is recursive. If the
        value in both dict are list, the 'dst' list is extended (.extend()) with the one of 'other'. """
        for k, v in other.items():
            if k in dst:
                if isinstance(dst[k], dict) and isinstance(other[k], dict):
                    cls.merge_dict(dst[k], other[k])
                elif isinstance(dst[k], list) and isinstance(other[k], list):
                    dst[k].extend(other[k])
            else:
                dst[k] = other[k]

    @classmethod
    def interpret_rule_os(cls, rule_os: dict) -> bool:
        os_name = rule_os.get("name")
        if os_name is None or os_name == cls.get_minecraft_os():
            os_arch = rule_os.get("arch")
            if os_arch is None or os_arch == cls.get_minecraft_arch():
                os_version = rule_os.get("version")
                if os_version is None or re.search(os_version, platform.version()) is not None:
                    return True
        return False

    @classmethod
    def interpret_rule(cls, rules: List[dict], features: Optional[dict] = None) -> bool:
        allowed = False
        for rule in rules:
            rule_os = rule.get("os")
            if rule_os is not None and not cls.interpret_rule_os(rule_os):
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

    @classmethod
    def interpret_args(cls, args: list, features: dict, dst: List[str]):
        for arg in args:
            if isinstance(arg, str):
                dst.append(arg)
            else:
                rules = arg.get("rules")
                if rules is not None:
                    if not cls.interpret_rule(rules, features):
                        continue
                arg_value = arg["value"]
                if isinstance(arg_value, list):
                    dst.extend(arg_value)
                elif isinstance(arg_value, str):
                    dst.append(arg_value)

    @classmethod
    def replace_vars(cls, txt: str, replacements: Dict[str, str]) -> str:
        parts = []
        last_end = 0
        while True:
            start = txt.find("${", last_end)
            if start == -1:
                break
            end = txt.find("}", start + 2)
            if end == -1:
                break
            parts.append(txt[last_end:start])
            var = txt[(start + 2):end]
            parts.append(replacements.get(var, txt[start:end + 1]))
            last_end = end + 1
        parts.append(txt[last_end:])
        return "".join(parts)

    @classmethod
    def replace_list_vars(cls, lst: List[str], replacements: Dict[str, str]) -> Generator[str, None, None]:
        return (cls.replace_vars(elt, replacements) for elt in lst)

    @classmethod
    def update_dict_keep(cls, orig: dict, other: dict):
        for k, v in other.items():
            if k not in orig:
                orig[k] = v

    @staticmethod
    def get_minecraft_dir() -> str:
        home = path.expanduser("~")
        return {
            "Linux": path.join(home, ".minecraft"),
            "Windows": path.join(home, "AppData", "Roaming", ".minecraft"),
            "Darwin": path.join(home, "Library", "Application Support", "minecraft")
        }.get(platform.system())

    @classmethod
    def get_minecraft_os(cls) -> str:
        if cls.minecraft_os is None:
            cls.minecraft_os = {"Linux": "linux", "Windows": "windows", "Darwin": "osx"}.get(platform.system(), "")
        return cls.minecraft_os

    @classmethod
    def get_minecraft_arch(cls) -> str:
        if cls.minecraft_arch is None:
            machine = platform.machine().lower()
            cls.minecraft_arch = "x86" if machine in ("i386", "i686") else "x86_64" if machine in ("x86_64", "amd64", "ia64") else ""
        return cls.minecraft_arch

    @classmethod
    def get_minecraft_archbits(cls) -> str:
        if cls.minecraft_archbits is None:
            raw_bits = platform.architecture()[0]
            cls.minecraft_archbits = "64" if raw_bits == "64bit" else "32" if raw_bits == "32bit" else ""
        return cls.minecraft_archbits

    @classmethod
    def get_minecraft_jvm_os(cls) -> str:
        if cls.minecraft_jvm_os is None:
            cls.minecraft_jvm_os = {
                "osx": {"x86": "mac-os"},
                "linux": {"x86": "linux-i386", "x86_64": "linux"},
                "windows": {"x86": "windows-x86", "x86_64": "windows-x64"}
            }.get(cls.get_minecraft_os(), {}).get(cls.get_minecraft_arch())
        return cls.minecraft_jvm_os

    @staticmethod
    def can_extract_native(filename: str) -> bool:
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

                    conn.request("GET", entry.url, None, headers)
                    res = conn.getresponse()
                    error = None

                    size_target = 0 if entry.size is None else entry.size

                    for _ in range(max_try_count):

                        if res.status != 200:
                            error = DownloadError.NOT_FOUND
                            continue

                        sha1 = hashlib.sha1()
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
        self.size: int = 0
        self.total = total


class BaseError(Exception):

    def __init__(self, code: str):
        super().__init__()
        self.code = code


class JsonRequestError(BaseError):

    INVALID_URL_SCHEME = "invalid_url_scheme"
    INVALID_RESPONSE_NOT_JSON = "invalid_response_not_json"
    SOCKET_ERROR = "socket_error"

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

    NOT_FOUND = "not_found"
    INVALID_SIZE = "invalid_size"
    INVALID_SHA1 = "invalid_sha1"

    def __init__(self, fails: Dict[str, str]):
        super().__init__()
        self.fails = fails


if __name__ == '__main__':

    from argparse import ArgumentParser, Namespace, HelpFormatter
    from http.server import HTTPServer, BaseHTTPRequestHandler
    from typing import cast, Union, Any
    from datetime import datetime
    import webbrowser
    import time


    EXIT_OK = 0
    EXIT_WRONG_USAGE = 9
    EXIT_VERSION_NOT_FOUND = 10
    EXIT_DOWNLOAD_ERROR = 13
    EXIT_AUTHENTICATION_FAILED = 14
    EXIT_DEPRECATED_ARGUMENT = 16
    EXIT_LOGOUT_FAILED = 17
    EXIT_JSON_REQUEST_ERROR = 18
    EXIT_JVM_LOADING_ERROR = 19

    AUTH_DB_FILE_NAME = "portablemc_auth.json"
    AUTH_DB_LEGACY_FILE_NAME = "portablemc_tokens"

    MS_AZURE_APP_ID = "708e91b5-99f8-4a1d-80ec-e746cbb24771"


    def cli(args: List[str]):

        parser = register_arguments()
        ns = parser.parse_args(args)

        command_handlers = get_command_handlers()
        command_attr = "subcommand"
        while True:
            command = getattr(ns, command_attr)
            handler = command_handlers.get(command)
            if handler is None:
                parser.print_help()
                sys.exit(EXIT_WRONG_USAGE)
            elif callable(handler):
                handler(ns, new_context(ns.main_dir, ns.work_dir))
            elif isinstance(handler, dict):
                command_attr = f"{command}_{command_attr}"
                command_handlers = handler
                continue
            sys.exit(EXIT_OK)

    # CLI Parser

    def register_arguments() -> ArgumentParser:
        _ = get_message
        parser = ArgumentParser(allow_abbrev=False, prog="portablemc", description=_("args"))
        parser.add_argument("--main-dir", help=_("args.main_dir"))
        parser.add_argument("--work-dir", help=_("args.work_dir"))
        register_subcommands(parser.add_subparsers(title="subcommands", dest="subcommand"))
        return parser

    def register_subcommands(subparsers):
        _ = get_message
        register_search_arguments(subparsers.add_parser("search", help=_("args.search")))
        register_start_arguments(subparsers.add_parser("start", help=_("args.start")))
        register_login_arguments(subparsers.add_parser("login", help=_("args.login")))
        register_logout_arguments(subparsers.add_parser("logout", help=_("args.logout")))
        register_addon_arguments(subparsers.add_parser("addon", help=_("args.addon")))

    def register_search_arguments(parser: ArgumentParser):
        parser.add_argument("-l", "--local", help=get_message("args.search.local"), action="store_true")
        parser.add_argument("input", nargs="?")

    def register_start_arguments(parser: ArgumentParser):
        _ = get_message
        parser.formatter_class = new_help_formatter_class(32)
        parser.add_argument("--dry", help=_("args.start.dry"), action="store_true")
        parser.add_argument("--disable-mp", help=_("args.start.disable_multiplayer"), action="store_true")
        parser.add_argument("--disable-chat", help=_("args.start.disable_chat"), action="store_true")
        parser.add_argument("--demo", help=_("args.start.demo"), action="store_true")
        parser.add_argument("--resol", help=_("args.start.resol"), type=decode_resolution)
        parser.add_argument("--jvm", help=_("args.start.jvm"))
        parser.add_argument("--jvm-args", help=_("args.start.jvm_args"))
        parser.add_argument("--no-better-logging", help=_("args.start.no_better_logging"), action="store_true")
        parser.add_argument("-t", "--temp-login", help=_("args.start.temp_login"), action="store_true")
        parser.add_argument("-l", "--login", help=_("args.start.login"))
        parser.add_argument("-m", "--microsoft", help=_("args.start.microsoft"), action="store_true")
        parser.add_argument("-u", "--username", help=_("args.start.username"), metavar="NAME")
        parser.add_argument("-i", "--uuid", help=_("args.start.uuid"))
        parser.add_argument("-s", "--server", help=_("args.start.server"))
        parser.add_argument("-p", "--server-port", type=int, help=_("args.start.server_port"), metavar="PORT")
        parser.add_argument("version", nargs="?", default="release")

    def register_login_arguments(parser: ArgumentParser):
        parser.add_argument("-m", "--microsoft", help=get_message("args.login.microsoft"), action="store_true")
        parser.add_argument("email_or_username")

    def register_logout_arguments(parser: ArgumentParser):
        parser.add_argument("-m", "--microsoft", help=get_message("args.logout.microsoft"), action="store_true")
        parser.add_argument("email_or_username")

    def register_addon_arguments(parser: ArgumentParser):
        _ = get_message
        subparsers = parser.add_subparsers(title="subcommands", dest="addon_subcommand")
        subparsers.required = True
        subparsers.add_parser("list", help=_("args.addon.list"))
        init_parser = subparsers.add_parser("init", help=_("args.addon.init"))
        init_parser.add_argument("--single-file", help=_("args.addon.init.single_file"), action="store_true")
        init_parser.add_argument("addon_name")
        show_parser = subparsers.add_parser("show", help=_("args.addon.show"))
        show_parser.add_argument("addon_name")

    def new_help_formatter_class(max_help_position: int) -> Type[HelpFormatter]:

        class CustomHelpFormatter(HelpFormatter):
            def __init__(self, prog):
                super().__init__(prog, max_help_position=max_help_position)

        return CustomHelpFormatter

    def decode_resolution(raw: str):
        return tuple(int(size) for size in raw.split("x"))

    # Commands handlers

    def get_command_handlers():
        return {
            "search": cmd_search,
            "start": cmd_start,
            "login": cmd_login,
            "logout": cmd_logout,
            "addon": {
                "list": cmd_addon_list,
                "init": cmd_addon_init,
                "show": cmd_addon_show
            }
        }

    def cmd_search(ns: Namespace, ctx: Context):

        _ = get_message
        table = []
        search = ns.input
        no_version = (search is None)

        if ns.local:
            for version, mtime in ctx.list_versions():
                if no_version or search in version:
                    table.append((version, format_iso_date(mtime)))
        else:
            manifest = load_version_manifest()
            search, alias = manifest.filter_latest(search)
            for version_data in manifest.all_versions():
                version = version_data["id"]
                if no_version or (alias and search == version) or (not alias and search in version):
                    table.append((
                        version_data["type"],
                        version,
                        format_iso_date(version_data["releaseTime"]),
                        _("cmd.search.flags.local") if ctx.has_version_metadata(version) else ""
                    ))

        if len(table):
            table.insert(0, (
                _("cmd.search.name"),
                _("cmd.search.last_modified")
            ) if ns.local else (
                _("cmd.search.type"),
                _("cmd.search.name"),
                _("cmd.search.release_date"),
                _("cmd.search.flags")
            ))
            print_table(table, header=0)
            sys.exit(EXIT_OK)
        else:
            print_message("cmd.search.not_found")
            sys.exit(EXIT_VERSION_NOT_FOUND)

    def cmd_start(ns: Namespace, ctx: Context):

        try:

            manifest = load_version_manifest()
            version_id, alias = manifest.filter_latest(ns.version)

            version = new_version(ctx, version_id)
            version.manifest = manifest

            print_task("", "version.resolving", {"version": version_id})
            version.prepare_meta()
            print_task("OK", "version.resolved", {"version": version_id}, done=True)

            print_task("", "version.jar.loading")
            version.prepare_jar()
            print_task("OK", "version.jar.loaded", done=True)

            print_task("", "assets.checking")
            version.prepare_assets()
            print_task("OK", "assets.checked", {"count": version.assets_count}, done=True)

            print_task("", "logger.loading")
            version.prepare_logger()
            print_task("OK", "logger.loaded", done=True)

            print_task("", "libraries.loading")
            version.prepare_libraries()
            libs_count = len(version.classpath_libs) + len(version.native_libs)
            print_task("OK", "libraries.loaded", {"count": libs_count}, done=True)

            if ns.jvm is None:
                print_task("", "jvm.loading")
                version.prepare_jvm()
                print_task("OK", "jvm.loaded", {"version": version.jvm_version}, done=True)

            pretty_download(version.dl)
            version.dl.reset()

            if ns.dry:
                return

            start_opts = new_start_options()
            start_opts.disable_multiplayer = ns.disable_mp
            start_opts.disable_chat = ns.disable_chat
            start_opts.demo = ns.demo
            start_opts.server_address = ns.server
            start_opts.server_port = ns.server_port

            if ns.resol is not None and len(ns.resol) == 2:
                start_opts.resolution = ns.resol

            if ns.login is not None:
                start_opts.auth_session = prompt_authenticate(ctx, ns.login, not ns.temp_login, ns.microsoft)
                if start_opts.auth_session:
                    sys.exit(EXIT_AUTHENTICATION_FAILED)
            else:
                start_opts.uuid = ns.uuid
                start_opts.username = ns.username

            # TODO: Handle JVM path and arguments

            start = new_start(version)
            start.prepare(start_opts)
            start.start()

        except VersionError as err:
            print_task("FAILED", f"version.error.{err.code}", {"version": err.version}, done=True)
            sys.exit(EXIT_VERSION_NOT_FOUND)
        except JvmLoadingError as err:
            print_task("FAILED", f"jvm.error.{err.code}", done=True)
            sys.exit(EXIT_JVM_LOADING_ERROR)
        except JsonRequestError as err:
            print_task("FAILED", f"son_request.error.{err.code}", {"details": err.details}, done=True)
            sys.exit(EXIT_JSON_REQUEST_ERROR)

    def cmd_login(ns: Namespace, ctx: Context):
        print("cmd_login")

    def cmd_logout(ns: Namespace, ctx: Context):
        print("cmd_logout")

    def cmd_addon_list(ns: Namespace, ctx: Context):
        print("cmd_addon_list")

    def cmd_addon_init(ns: Namespace, ctx: Context):
        print("cmd_addon_init")

    def cmd_addon_show(ns: Namespace, ctx: Context):
        print("cmd_addon_show")

    # Constructors to override

    def new_context(main_dir: Optional[str], work_dir: Optional[str]) -> Context:
        return Context(main_dir, work_dir)

    def new_version(ctx: Context, version: str) -> Version:
        return Version(ctx, version)

    def new_start(version: Version) -> Start:
        return Start(version)

    def new_start_options() -> StartOptions:
        return StartOptions()

    def new_auth_database(ctx: Context) -> AuthDatabase:
        return AuthDatabase(path.join(ctx.work_dir, AUTH_DB_FILE_NAME), path.join(ctx.work_dir, AUTH_DB_LEGACY_FILE_NAME))

    # Internal utilities

    def format_iso_date(raw: Union[str, float]) -> str:
        if isinstance(raw, float):
            return datetime.fromtimestamp(raw).strftime("%c")
        else:
            return datetime.strptime(str(raw).rsplit("+", 2)[0], "%Y-%m-%dT%H:%M:%S").strftime("%c")

    def format_bytes(n: int) -> str:
        """ Return a byte with suffix B, kB, MB and GB. The string is always 7 chars unless the size exceed 1 TB. """
        if n < 1000:
            return "{:6d}B".format(int(n))
        elif n < 1000000:
            return "{:5.1f}kB".format(int(n / 100) / 10)
        elif n < 1000000000:
            return "{:5.1f}MB".format(int(n / 100000) / 10)
        else:
            return "{:5.1f}GB".format(int(n / 100000000) / 10)

    def load_version_manifest() -> VersionManifest:
        return VersionManifest.load_from_url()

    _term_width = 0
    _term_width_update_time = 0
    def get_term_width() -> int:
        global _term_width, _term_width_update_time
        now = time.monotonic()
        if now - _term_width_update_time > 1:
            _term_width_update_time = now
            _term_width = shutil.get_terminal_size().columns
        return _term_width

    # Pretty download

    def pretty_download(dl_list: DownloadList):

        start_time = time.perf_counter()
        last_print_time: Optional[bool] = None
        called_once = False

        dl_text = get_message("download.downloading")
        non_path_len = len(dl_text) + 21

        def progress_callback(progress: 'DownloadProgress'):
            nonlocal called_once, last_print_time
            now = time.perf_counter()
            if last_print_time is None or (now - last_print_time) > 0.1:
                last_print_time = now
                speed = format_bytes(int(progress.size / (now - start_time)))
                percentage = progress.size / progress.total * 100
                entries = ", ".join((entry.name for entry in progress.entries))
                path_len = max(0, min(80, get_term_width()) - non_path_len - len(speed))
                print(f"[      ] {dl_text} {entries[:path_len].ljust(path_len)} {percentage:6.2f}% {speed}/s\r", end="")
                called_once = True

        def complete_task(error: bool = False):
            if called_once:
                result_text = get_message("download.downloaded",
                                          count=dl_list.count,
                                          size=format_bytes(dl_list.size).lstrip(" "),
                                          duration=(time.perf_counter() - start_time))
                if error:
                    result_text = get_message("download.errors", count=result_text)
                result_len = max(0, min(80, get_term_width()) - 9)
                template = "\r[FAILED] {}" if error else "\r[  OK  ] {}"
                print(template.format(result_text[:result_len].ljust(result_len)))

        try:
            dl_list.callbacks.insert(0, complete_task)
            dl_list.download_files(progress_callback=progress_callback)
        except DownloadError as err:
            complete_task(True)
            for entry_url, entry_error in err.args[0]:
                entry_error_msg = get_message(f"download.error.{entry_error}")
                print(f"         {entry_url}: {entry_error_msg}")
        finally:
            dl_list.callbacks.pop(0)

    # Authentication

    def prompt_authenticate(ctx: Context, email_or_username: str, cache_in_db: bool, microsoft: bool) -> Optional[AuthSession]:

        auth_db = new_auth_database(ctx)
        auth_db.load()

        task_text = "auth.microsoft" if microsoft else "auth.yggdrasil"
        task_text_args = {"username": email_or_username}
        print_task("", task_text, task_text_args)

        session = auth_db.get(email_or_username, MicrosoftAuthSession if microsoft else YggdrasilAuthSession)
        if session is not None:
            try:
                if not session.validate():
                    print_task("", "auth.refreshing")
                    session.refresh()
                    auth_db.save()
                    print_task("OK", "auth.refreshed", task_text_args, done=True)
                else:
                    print_task("OK", "auth.validated", task_text_args, done=True)
                return session
            except AuthError as err:
                print_task("FAILED", "auth.error.{}".format(err.args[0]), *err.args[1:], done=True)

        print_task("..", task_text, task_text_args, done=True)

        try:
            session = prompt_microsoft_authenticate(email_or_username) if microsoft else prompt_yggdrasil_authenticate(email_or_username)
            if session is None:
                return None
            if cache_in_db:
                print_task("", "auth.caching")
                auth_db.put(email_or_username, session)
                auth_db.save()
            print_task("OK", "auth.logged_in", done=True)
            return session
        except AuthError as err:
            print_task("FAILED", f"auth.error.{err.code}", {"details": err.details}, done=True)
            return None

    def prompt_yggdrasil_authenticate(email_or_username: str) -> Optional[YggdrasilAuthSession]:
        print_task(None, "auth.yggdrasil.enter_password")
        password = prompt(password=True)
        if password is None:
            print_task("FAILED", "cancelled")
            return None
        else:
            return YggdrasilAuthSession.authenticate(email_or_username, password)

    def prompt_microsoft_authenticate(email: str) -> Optional[MicrosoftAuthSession]:

        server_port = 12782
        client_id = MS_AZURE_APP_ID
        redirect_auth = "http://localhost:{}".format(server_port)
        code_redirect_uri = "{}/code".format(redirect_auth)
        exit_redirect_uri = "{}/exit".format(redirect_auth)

        nonce = uuid4().hex

        if not webbrowser.open(MicrosoftAuthSession.get_authentication_url(client_id, code_redirect_uri, email, nonce)):
            print_task("FAILED", "auth.microsoft.no_browser", done=True)
            return None

        class AuthServer(HTTPServer):

            def __init__(self):
                super().__init__(("", server_port), RequestHandler)
                self.timeout = 0.5
                self.ms_auth_done = False
                self.ms_auth_id_token: Optional[str] = None
                self.ms_auth_code: Optional[str] = None

        class RequestHandler(BaseHTTPRequestHandler):

            server_version = "PortableMC/{}".format(LAUNCHER_VERSION)

            def __init__(self, request: bytes, client_address: Tuple[str, int], auth_server: AuthServer) -> None:
                super().__init__(request, client_address, auth_server)

            def log_message(self, _format: str, *args: Any):
                return

            def send_auth_response(self, msg: str):
                self.end_headers()
                self.wfile.write("{}{}".format(msg, "\n\nClose this tab and return to the launcher." if cast(AuthServer, self.server).ms_auth_done else "").encode())
                self.wfile.flush()

            def do_POST(self):
                if self.path.startswith(
                        "/code") and self.headers.get_content_type() == "application/x-www-form-urlencoded":
                    content_length = int(self.headers.get("Content-Length"))
                    qs = url_parse.parse_qs(self.rfile.read(content_length).decode())
                    auth_server = cast(AuthServer, self.server)
                    if "code" in qs and "id_token" in qs:
                        self.send_response(307)
                        # We logout the user directly after authorization, this just clear the browser cache to allow
                        # another user to authenticate with another email after. This doesn't invalide the access token.
                        self.send_header("Location", MicrosoftAuthSession.get_logout_url(client_id, exit_redirect_uri))
                        auth_server.ms_auth_id_token = qs["id_token"][0]
                        auth_server.ms_auth_code = qs["code"][0]
                        self.send_auth_response("Redirecting...")
                    elif "error" in qs:
                        self.send_response(400)
                        auth_server.ms_auth_done = True
                        self.send_auth_response("Error: {} ({}).".format(qs["error_description"][0], qs["error"][0]))
                    else:
                        self.send_response(404)
                        self.send_auth_response("Missing parameters.")
                else:
                    self.send_response(404)
                    self.send_auth_response("Unexpected page.")

            def do_GET(self):
                auth_server = cast(AuthServer, self.server)
                if self.path.startswith("/exit"):
                    self.send_response(200)
                    auth_server.ms_auth_done = True
                    self.send_auth_response("Logged in.")
                else:
                    self.send_response(404)
                    self.send_auth_response("Unexpected page.")

        print_task("", "auth.microsoft.opening_browser_and_listening")

        try:
            with AuthServer() as server:
                while not server.ms_auth_done:
                    server.handle_request()
        except KeyboardInterrupt:
            pass

        if server.ms_auth_code is None:
            print_task("FAILED", "auth.microsoft.failed_to_authenticate", done=True)
            return None
        else:
            print_task("", "auth.microsoft.processing")
            if MicrosoftAuthSession.check_token_id(server.ms_auth_id_token, email, nonce):
                return MicrosoftAuthSession.authenticate(client_id, server.ms_auth_code, code_redirect_uri)
            else:
                print_task("FAILED", "auth.microsoft.incoherent_dat", done=True)
                return None

    # Messages

    def get_message_raw(key: str, kwargs: Optional[dict]) -> str:
        try:
            return messages[key].format_map(kwargs or {})
        except KeyError:
            return key

    def get_message(key: str, **kwargs) -> str:
        return get_message_raw(key, kwargs)

    def print_message(key: str, end: str = "\n", **kwargs):
        print(get_message(key, **kwargs), end=end)

    def prompt(password: bool = False) -> Optional[str]:
        try:
            if password:
                import getpass
                return getpass.getpass("")
            else:
                return input("")
        except KeyboardInterrupt:
            return None

    def print_table(lines: List[Tuple[str, ...]], *, header: int = -1):
        if not len(lines):
            return
        columns_count = len(lines[0])
        columns_length = [0] * columns_count
        for line in lines:
            if len(line) != columns_count:
                raise ValueError(f"Inconsistent cell count '{line}', expected {columns_count}.")
            for i, cell in enumerate(line):
                cell_len = len(cell)
                if columns_length[i] < cell_len:
                    columns_length[i] = cell_len
        format_string = "│ {} │".format(" │ ".join(("{{:{}s}}".format(length) for length in columns_length)))
        columns_lines = ["─" * length for length in columns_length]
        print("┌─{}─┐".format("─┬─".join(columns_lines)))
        for i, line in enumerate(lines):
            print(format_string.format(*line))
            if i == header:
                print("├─{}─┤".format("─┼─".join(columns_lines)))
        print("└─{}─┘".format("─┴─".join(columns_lines)))

    _print_task_last_len = 0
    def print_task(status: Optional[str], msg_key: str, msg_args: Optional[dict] = None, *, done: bool = False):
        global _print_task_last_len
        len_limit = max(0, get_term_width() - 9)
        msg = get_message_raw(msg_key, msg_args)[:len_limit]
        missing_len = max(0, _print_task_last_len - len(msg))
        status_header = "\r         " if status is None else "\r[{:^6s}] ".format(status)
        _print_task_last_len = 0 if done else len(msg)
        print(status_header, msg, " " * missing_len, sep="", end="\n" if done else "", flush=True)

    messages = {

        "addon.defined_twice": "The addon '{}' is defined twice, both single-file and package, loaded the package one.",
        "addon.missing_requirement.module": "Addon '{0}' requires module '{1}' to load. You can try to install "
                                            "it using 'pip install {1}' or search for it on the web.",
        "addon.missing_requirement.ext": "Addon '{}' requires another addon '{}' to load.",
        "addon.failed_to_build": "Failed to build addon '{}' (contact addon's authors):",

        "args": "PortableMC is an easy to use portable Minecraft launcher in only one Python "
                "script! This single-script launcher is still compatible with the official "
                "(Mojang) Minecraft Launcher stored in .minecraft and use it.",
        "args.main_dir": "Set the main directory where libraries, assets and versions. "
                         "This argument can be used or not by subcommand.",
        "args.work_dir": "Set the working directory where the game run and place for examples "
                         "saves, screenshots (and resources for legacy versions), it also store "
                         "runtime binaries and authentication. "
                         "This argument can be used or not by subcommand.",
        "args.search": "Search for Minecraft versions.",
        "args.search.local": "Search only for local installed Minecraft versions.",
        "args.start": "Start a Minecraft version, default to the latest release.",
        "args.start.dry": "Simulate game starting.",
        "args.start.disable_multiplayer": "Disable the multiplayer buttons (>= 1.16).",
        "args.start.disable_chat": "Disable the online chat (>= 1.16).",
        "args.start.demo": "Start game in demo mode.",
        "args.start.resol": "Set a custom start resolution (<width>x<height>).",
        "args.start.jvm": "Set a custom JVM 'javaw' executable path. If this argument is omitted a public build "
                          "of a JVM is downloaded from Mojang services.",
        "args.start.jvm_args": "Change the default JVM arguments.",
        "args.start.no_better_logging": "Disable the better logging configuration built by the launcher in "
                                        "order to improve the log readability in the console.",
        "args.start.temp_login": "Flag used with -l (--login) to tell launcher not to cache your session if "
                                 "not already cached, disabled by default.",
        "args.start.login": "Use a email (or deprecated username) to authenticate using Mojang services (it override --username and --uuid).",
        "args.start.microsoft": "Login using Microsoft account, to use with -l (--login).",
        "args.start.username": "Set a custom user name to play.",
        "args.start.uuid": "Set a custom user UUID to play.",
        "args.start.server": "Start the game and auto-connect to this server address (since 1.6).",
        "args.start.server_port": "Set the server address port (given with -s, --server, since 1.6).",
        "args.login": "Login into your account, this will cache your session.",
        "args.login.microsoft": "Login using Microsoft account.",
        "args.logout": "Logout and invalidate a session.",
        "args.logout.microsoft": "Logout from a Microsoft account.",
        "args.addon": "Addons management subcommands.",
        "args.addon.list": "List addons.",
        "args.addon.init": "For developers: Given an addon's name, initialize its package if it doesn't already exists.",
        "args.addon.init.single_file": "Make a single-file addon instead of a package one.",
        "args.addon.show": "Show an addon details.",

        "continue_using_main_dir": "Continue using this main directory ({})? (y/N) ",
        # "http_request_error": "HTTP request error: {}",
        "cancelled": "Cancelled.",

        f"json_request.error.{JsonRequestError.INVALID_URL_SCHEME}": "Invalid URL scheme: {details}",
        f"json_request.error.{JsonRequestError.INVALID_RESPONSE_NOT_JSON}": "Invalid response, not JSON: {details}",
        f"json_request.error.{JsonRequestError.SOCKET_ERROR}": "Socket error: {details}",

        "cmd.search.type": "Type",
        "cmd.search.name": "Identifier",
        "cmd.search.release_date": "Release date",
        "cmd.search.last_modified": "Last modified",
        "cmd.search.flags": "Flags",
        "cmd.search.flags.local": "local",
        # "cmd.search.pending": "Searching for version '{input}'...",
        # "cmd.search.pending_local": "Searching for local version '{input}'...",
        # "cmd.search.pending_all": "Searching for all versions...",
        # "cmd.search.result": "=> {type:10s} {version:16s} {date:24s} {more}",
        # "cmd.search.result.more.local": "[LOCAL]",
        # "cmd.search.not_found": "=> No version found",

        "cmd.logout.yggdrasil.pending": "Logging out {} from Mojang...",
        "cmd.logout.microsoft.pending": "Logging out {} from Microsoft...",
        "cmd.logout.success": "Logged out {}.",
        "cmd.logout.unknown_session": "No session for {}.",

        "cmd.addon.list.title": "Addons list ({}):",
        "cmd.addon.list.result": "=> {:20s} v{} by {} [{}]",
        "cmd.addon.init.already_exits": "An addon '{}' already exists at '{}'.",
        "cmd.addon.init.done": "The addon '{}' was initialized at '{}'.",
        "cmd.addon.show.unknown": "No addon named '{}' exists.",
        "cmd.addon.show.title": "Addon {} ({}):",
        "cmd.addon.show.version": "=> Version: {}",
        "cmd.addon.show.authors": "=> Authors: {}",
        "cmd.addon.show.description": "=> Description: {}",
        "cmd.addon.show.requires": "=> Requires: {}",

        "download.downloading": "Downloading",
        "download.downloaded": "Downloaded {count} files, {size} in {duration:.1f}s.",
        "download.errors": "{count} errors happened, can't continue.",
        f"download.error.{DownloadError.NOT_FOUND}": "Not found",
        f"download.error.{DownloadError.INVALID_SIZE}": "Invalid size",
        f"download.error.{DownloadError.INVALID_SHA1}": "Invalid SHA1",

        "auth.refreshing": "Invalid session, refreshing...",
        "auth.refreshed": "Session refreshed for {username}.",
        "auth.validated": "Session validated for {username}.",
        "auth.caching": "Caching your session...",
        "auth.logged_in": "Logged in",

        "auth.yggdrasil": "Authenticating {username} with Mojang...",
        "auth.yggdrasil.enter_password": "Password: ",
        f"auth.error.{AuthError.YGGDRASIL}": "{details}",

        "auth.microsoft": "Authenticating {username} with Microsoft...",
        "auth.microsoft.no_browser": "Failed to open Microsoft login page, no web browser is supported.",
        "auth.microsoft.opening_browser_and_listening": "Opened authentication page in browser...",
        "auth.microsoft.failed_to_authenticate": "Failed to authenticate.",
        "auth.microsoft.processing": "Processing authentication against Minecraft services...",
        "auth.microsoft.incoherent_data": "Incoherent authentication data, please retry.",
        f"auth.error.{AuthError.MICROSOFT_INCONSISTENT_USER_HASH}": "Inconsistent user hash.",
        f"auth.error.{AuthError.MICROSOFT_DOES_NOT_OWN_MINECRAFT}": "This account does not own Minecraft.",
        f"auth.error.{AuthError.MICROSOFT_OUTDATED_TOKEN}": "The token is no longer valid.",
        f"auth.error.{AuthError.MICROSOFT}": "Misc error: {details}.",

        "version.resolving": "Resolving version {version}... ",
        "version.resolved": "Resolved version {version}.",
        "version.jar.loading": "Loading version JAR... ",
        "version.jar.loaded": "Loaded version JAR.",
        f"version.error.{VersionError.NOT_FOUND}": "Version {version} not found.",
        f"version.error.{VersionError.TO_MUCH_PARENTS}": "The version {version} has to much parents.",
        f"version.error.{VersionError.JAR_NOT_FOUND}": "Version {version} JAR not found.",
        "assets.checking": "Checking assets... ",
        "assets.checked": "Checked {count} assets.",
        "logger.loading": "Loading logger... ",
        "logger.loaded": "Loaded logger.",
        "logger.loaded_pretty": "Loaded pretty logger.",
        "libraries.loading": "Loading libraries... ",
        "libraries.loaded": "Loaded {count} libraries.",
        "jvm.loading": "Loading java... ",
        "jvm.loaded": "Loaded Mojang Java {version}.",
        f"jvm.error.{JvmLoadingError.UNSUPPORTED_ARCH}": "No JVM download was found for your platform architecture, use --jvm argument to set the JVM executable of path to it.",
        f"jvm.error.{JvmLoadingError.UNSUPPORTED_VERSION}": "No JVM download was found, use --jvm argument to set the JVM executable of path to it.",

        "start.dry": "Dry run, stopping.",
        "start.starting": "Starting game...",
        "start.extracting_natives": "=> Extracting natives...",
        "start.running": "Running...",
        "start.stopped": "Game stopped, clearing natives.",
        "start.run.session": "=> Username: {}, UUID: {}",
        "start.run.command_line": "=> Command line: {}",

    }

    # Actual start

    cli(sys.argv[1:])
