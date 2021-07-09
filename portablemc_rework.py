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
import atexit
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
        main_dir = Util.get_minecraft_dir() if main_dir is None else path.realpath(main_dir)
        self.work_dir = main_dir if work_dir is None else path.realpath(work_dir)
        self.versions_dir = path.join(main_dir, "versions")
        self.assets_dir = path.join(main_dir, "assets")
        self.libraries_dir = path.join(main_dir, "libraries")
        self.jvm_dir = path.join(main_dir, "jvm")
        self.bin_dir = path.join(self.work_dir, "bin")


class Version:

    """
    This class is used to manage the installation of a version and then run it.\n
    All public function in this class can be executed multiple times, however they might add duplicate URLs to
    the download list. The game still requires some parts to be prepared before starting.
    """

    def __init__(self, context: Context, version: str):

        self.context = context
        self.version = version

        self.manifest: Optional[VersionManifest] = None
        self.dl = DownloadList()

        self.version_meta: Optional[dict] = None
        self.version_dir: Optional[str] = None
        self.version_jar_file: Optional[str] = None

        self.assets_index_version: Optional[int] = None
        self.assets_virtual_dir: Optional[str] = None

        self.logging_file: Optional[str] = None
        self.logging_argument: Optional[str] = None

        self.classpath_libs: List[str] = []
        self.native_libs: List[str] = []

        self.jvm_version: Optional[str] = None
        self.jvm_exec: Optional[str] = None

    def prepare_meta(self):

        """
        Prepare all metadata files for this version, this take 'inheritsFrom' key into account and all parents metadata
        files are downloaded. Each metadata file is downloaded (if not already cached) in their own directory named
        after the version ID, the directory is placed in the 'versions_dir' of the context.\n
        This method will load the official Mojang version manifest, however you can set the 'manifest' attribute of this
        object before with a custom manifest if you want to support more versions.\n
        If any version in the inherit tree is not found, a VersionError is raised with VersionError.NOT_FOUND and the
        version ID as argument.
        """

        if self.manifest is None:
            self.manifest = VersionManifest.load_from_url()

        version_meta, version_dir = self._prepare_meta_internal(self.version)
        while "inheritsFrom" in version_meta:  # TODO: Add a safe recursion limit
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
            raise ValueError("You must install metadata before.")

    def prepare_jar(self):

        """
        Must be called once metadata file are prepared, using 'prepare_meta', if not, ValueError is raised.\n
        If the metadata provides a client download URL, and the version JAR file doesn't exists or have not the expected
        size, it's added to the download list to be downloaded to the same directory as the metadata file.\n
        If no download URL is provided by metadata and the JAR file does not exists, a VersionError is raised with
        VersionError.JAR_NOT_FOUND.
        """

        self._check_version_meta()
        self.version_jar_file = path.join(self.version_dir, f"{self.version}.jar")
        client_download = self.version_meta.get("downloads", {}).get("client")
        if client_download is not None:
            entry = DownloadEntry.from_meta(client_download, self.version_jar_file, name=f"{self.version}.jar")
            if not path.isfile(entry.dst) or path.getsize(entry.dst) != entry.size:
                self.dl.append(entry)
        elif not path.isfile(self.version_jar_file):
            raise VersionError(VersionError.JAR_NOT_FOUND)

    def prepare_assets(self):

        """
        Must be called once metadata file are prepared, using 'prepare_meta', if not, ValueError is raised.\n
        This method download the asset index file (if not already cached) named after the asset version into the
        directory 'indexes' placed into the directory 'assets_dir' of the context. Once ready, the asset index file
        is analysed and each object is checked, if it does not exist or not have the expected size, it is downloaded
        to the 'objects' directory placed into the directory 'assets_dir' of the context.
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

    def prepare_logger(self):

        """
        Must be called once metadata file are prepared, using 'prepare_meta', if not, ValueError is raised.\n
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
            # return client_logging["argument"].replace("${path}", logging_file)

    def prepare_libraries(self):

        """
        Must be called once metadata file are prepared, using 'prepare_meta', if not, ValueError is raised.\n
        This method check all libraries found in the metadata, each library is downloaded if not already stored. Real
        Java libraries are added to the classpath list and native libraries are added to the native list.
        """

        self._check_version_meta()
        self.classpath_libs.clear()
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
            if lib_dl_entry is not None and path.isfile(lib_path) and path.getsize(lib_path) == lib_dl_entry.size:
                self.dl.append(lib_dl_entry)

    def prepare_jvm(self):

        """
        Must be called once metadata file are prepared, using 'prepare_meta', if not, ValueError is raised.\n
        This method check all libraries found in the metadata, each library is downloaded if not already stored. Real
        Java libraries are added to the classpath list and native libraries are added to the native list.
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
        self.dl.download_files(progress_callback=progress_callback)
        self.dl.reset()

    def install(self, *, jvm: bool = False):
        self.prepare_meta()
        self.prepare_jar()
        self.prepare_assets()
        self.prepare_logger()
        self.prepare_libraries()
        if jvm:
            self.prepare_jvm()
        self.download()

    def start(self, options: 'StartOptions'):

        self._check_version_meta()

        main_class = self.version_meta.get("mainClass")
        if main_class is None:
            raise ValueError("This version metadata has no main class to start.")

        bin_dir = path.join(self.context.bin_dir, str(uuid4()))

        @atexit.register
        def _bin_dir_cleanup():
            if path.isdir(bin_dir):
                shutil.rmtree(bin_dir)

        for native_lib in self.native_libs:
            with ZipFile(native_lib, 'r') as native_zip:
                for native_zip_info in native_zip.infolist():
                    if Util.can_extract_native(native_zip_info.filename):
                        native_zip.extract(native_zip_info, bin_dir)

        # Features
        features = {
            "is_demo_user": options.demo,
            "has_custom_resolution": options.resolution is not None,
            **options.features
        }

        # Auth
        auth_session = options.auth_session
        if auth_session is not None:
            uuid = auth_session.uuid
            username = auth_session.username
        else:
            uuid = uuid4().hex if options.uuid is None else options.uuid.replace("-", "")
            username = uuid[:8] if options.username is None else options.username[:16]  # Max username length is 16

        # Arguments replacements
        args_replacements = {
            # Game
            "auth_player_name": username,
            "version_name": self.version,
            "game_directory": self.context.work_dir,
            "assets_root": self.context.assets_dir,
            "assets_index_name": self.assets_index_version,
            "auth_uuid": uuid,
            "auth_access_token": "" if auth_session is None else auth_session.format_token_argument(False),
            "user_type": "mojang",
            "version_type": self.version_meta.get("type", ""),
            # Game (legacy)
            "auth_session": "notok" if auth_session is None else auth_session.format_token_argument(True),
            "game_assets": self.assets_virtual_dir,
            "user_properties": "{}",
            # JVM
            "natives_directory": bin_dir,
            "launcher_name": LAUNCHER_NAME,
            "launcher_version": LAUNCHER_VERSION,
            "classpath": path.pathsep.join(self.classpath_libs),
            **options.args_replacements
        }

        modern_args = self.version_meta.get("arguments", {})
        modern_jvm_args = modern_args.get("jvm")
        modern_game_args = modern_args.get("game")

        raw_args = []

        # JVM arguments
        Util.interpret_args(Util.LEGACY_JVM_ARGUMENTS if modern_jvm_args is None else modern_jvm_args, features, raw_args)

        # JVM argument for logging config
        if self.logging_argument is not None:
            raw_args.append(self.logging_argument.replace("${path}", self.logging_file))

        # JVM argument for launch wrapper JAR path
        if main_class == "net.minecraft.launchwrapper.Launch":
            raw_args.append("-Dminecraft.client.jar={}".format(self.version_jar_file))

        raw_args.append(main_class)

        # Game arguments
        if modern_game_args is None:
            raw_args.extend(self.version_meta.get("minecraftArguments", "").split(" "))
        else:
            Util.interpret_args(modern_game_args, features, raw_args)

        for i in range(len(raw_args)):
            raw_arg = raw_args[i]
            start = raw_arg.find("${")
            if start == -1:
                break
            end = raw_arg.find("}", start + 1)
            if end == -1:
                break
            var_name = raw_arg[(start + 1):end]





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

        self.features: Dict[str, bool] = {}
        self.args_replacements: Dict[str, str] = {}
        self.jvm_args: List[str] = []
        self.games_args: List[str] = []


class VersionManifest:

    def __init__(self, data: dict):
        self._data = data

    @classmethod
    def load_from_url(cls):
        return cls(Util.json_simple_request(VERSION_MANIFEST_URL))

    def filter_latest(self, version: str) -> Tuple[Optional[str], bool]:
        return (self._data["latest"][version], True) if version in self._data["latest"] else (version, False)

    def get_version(self, version: str) -> Optional[dict]:
        version, _alias = self.filter_latest(version)
        for version_data in self._data["versions"]:
            if version_data["id"] == version:
                return version_data
        return None

    def all_versions(self) -> list:
        return self._data["versions"]

    def search_versions(self, inp: str) -> Generator[dict, None, None]:
        inp, alias = self.filter_latest(inp)
        for version_data in self._data["versions"]:
            if (alias and version_data["id"] == inp) or (not alias and inp in version_data["id"]):
                yield version_data


class AuthSession:

    type = "raw"
    fields = "access_token", "username", "uuid"

    def __init__(self, access_token: str, username: str, uuid: str):
        self.access_token = access_token
        self.username = username
        self.uuid = uuid

    def format_token_argument(self, legacy: bool) -> str:
        return "token:{}:{}".format(self.access_token, self.uuid) if legacy else self.access_token

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
            "identityToken": "XBL3.0 x={};{}".format(xbl_user_hash, xsts_token)
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
        return Util.json_request(url, "GET", headers={"Authorization": "Bearer {}".format(bearer)})

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
    rule_os_checks: Optional[list] = None

    @staticmethod
    def json_request(url: str, method: str, *,
                     data: Optional[bytes] = None,
                     headers: Optional[dict] = None,
                     ignore_error: bool = False,
                     timeout: Optional[int] = None) -> Tuple[int, dict]:

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
                    raise JsonRequestError(JsonRequestError.INVALID_RESPONSE_NOT_JSON, res.status)
        except OSError:
            raise JsonRequestError(JsonRequestError.SOCKET_ERROR)
        finally:
            conn.close()

    @classmethod
    def json_simple_request(cls, url: str, *, ignore_error: bool = False, timeout: Optional[int] = None) -> dict:
        return cls.json_request(url, "GET", ignore_error=ignore_error, timeout=timeout)[1]

    @classmethod
    def merge_dict(cls, dst: dict, other: dict):
        """ Merge the 'other' dict into the 'dst' dict. For every key/value in 'other', if the key is present in 'dst'
        it does nothing. Unless if the value in both dict are also dict, in this case the merge is recursive. If the
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
                if "rules" in arg:
                    if not cls.interpret_rule(arg["rules"], features):
                        continue
                arg_value = arg["value"]
                if isinstance(arg_value, list):
                    dst.extend(arg_value)
                elif isinstance(arg_value, str):
                    dst.append(arg_value)

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
        host_key = "{}{}".format(int(url_parsed.scheme == "https"), url_parsed.netloc)
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
        only at the end, where 'fails' is a dict associating the entry URL and its error ('not_found', 'invalid_size',
        'invalid_sha1').
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

                        total_size -= size

                    else:
                        # If the break was not triggered, an error should be set.
                        fails[entry.url] = error

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
    def __init__(self, code: str, *args):
        super().__init__(code, args)
        self.code = code


class JsonRequestError(BaseError):
    INVALID_URL_SCHEME = "invalid_url_scheme"
    INVALID_RESPONSE_NOT_JSON = "invalid_response_not_json"
    SOCKET_ERROR = "socket_error"


class AuthError(BaseError):
    YGGDRASIL = "yggdrasil"
    MICROSOFT = "microsoft"
    MICROSOFT_INCONSISTENT_USER_HASH = "microsoft.inconsistent_user_hash"
    MICROSOFT_DOES_NOT_OWN_MINECRAFT = "microsoft.does_not_own_minecraft"
    MICROSOFT_OUTDATED_TOKEN = "microsoft.outdated_token"


class VersionError(BaseError):
    NOT_FOUND = "not_found"
    JAR_NOT_FOUND = "jar_not_found"


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

    def cli_start():

        ctx = Context()
        ver = Version(ctx, "1.16.5")
        ver.install()

    cli_start()
