import sys

if sys.version_info[0] < 3 or sys.version_info[1] < 6:
    print("PortableMC cannot be used with Python version prior to 3.6.x")
    exit(1)


from typing import cast, Dict, Set, Callable, Any, Optional, Generator, Tuple
from argparse import ArgumentParser, Namespace
from json.decoder import JSONDecodeError
from datetime import datetime
from zipfile import ZipFile
from uuid import uuid4
from os import path
import subprocess
import platform
import hashlib
import atexit
import shutil
import time
import json
import re
import os


LAUNCHER_NAME = "portablemc"
LAUNCHER_VERSION = "1.1.0"
LAUNCHER_AUTHORS = "Théo Rozier"

VERSION_MANIFEST_URL = "https://launchermeta.mojang.com/mc/game/version_manifest.json"
ASSET_BASE_URL = "https://resources.download.minecraft.net/{}/{}"
AUTHSERVER_URL = "https://authserver.mojang.com/{}"

JVM_EXEC_DEFAULT = "java"
JVM_ARGS_DEFAULT = "-Xmx2G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M"

EXIT_VERSION_NOT_FOUND = 10
EXIT_CLIENT_JAR_NOT_FOUND = 11
EXIT_NATIVES_DIR_ALREADY_EXITS = 12
EXIT_DOWNLOAD_FILE_CORRUPTED = 13
EXIT_AUTHENTICATION_FAILED = 14
EXIT_VERSION_SEARCH_NOT_FOUND = 15
EXIT_DEPRECATED_ARGUMENT = 15

LOGGING_CONSOLE_REPLACEMENT = "<PatternLayout pattern=\"%d{HH:mm:ss.SSS} [%t] %-5level %logger{36} - %msg%n\"/>"


class PortableMC:

    def __init__(self):

        self._argument_parser = ArgumentParser(
            allow_abbrev=False,
            prog="portablemc",
            description="PortableMC is an easy to use portable Minecraft launcher in only one Python script! "
                        "This single-script launcher is still compatible with the official (Mojang) Minecraft "
                        "Launcher stored in .minecraft and use it."
        )

        self._extensions: Dict[str, PortableExtension] = {}
        self._event_listeners: Dict[str, Set[Callable]] = {}
        self._main_dir: Optional[str] = None

        self._mc_os = self.get_minecraft_os()
        self._mc_arch = self.get_minecraft_arch()
        self._mc_archbits = self.get_minecraft_archbits()

        self._version_manifest: Optional[VersionManifest] = None
        self._auth_database: Optional[AuthDatabase] = None
        self._download_buffer: Optional[bytearray] = None

    def start(self):

        self._register_extensions()
        self._register_arguments()
        args = self._argument_parser.parse_args()
        subcommand = args.subcommand

        if subcommand is None:
            self._argument_parser.print_help()
            return

        self._main_dir = self.get_minecraft_dir() if args.main_dir is None else path.realpath(args.main_dir)
        if not path.isdir(self._main_dir):
            if input("Continue using this main directory? (y/N) ") != "y":
                print("=> Abort")
                exit(0)
            os.makedirs(self._main_dir, 0o777, True)

        builtin_func_name = "_cmd_{}".format(subcommand)
        if hasattr(self, builtin_func_name) and callable(getattr(self, builtin_func_name)):
            getattr(self, builtin_func_name)(args)

        self.trigger_event("subcommand", subcommand, args)

    def _register_extensions(self):

        from importlib.machinery import SourceFileLoader
        import importlib.util

        ext_dir = path.join(path.dirname(__file__), "exts")
        if path.isdir(ext_dir):
            for raw_ext_file in os.listdir(ext_dir):
                ext_file = path.abspath(path.join(ext_dir, raw_ext_file))
                if path.isfile(ext_file) and ext_file.endswith(".py"):
                    ext_name = raw_ext_file[:-3]
                    module_spec = importlib.util.spec_from_file_location("__ext_main__", ext_file)
                    module_loader = cast(SourceFileLoader, module_spec.loader)
                    module = importlib.util.module_from_spec(module_spec)
                    module_loader.exec_module(module)
                    if hasattr(module, "load") and callable(module.load):
                        ext = PortableExtension(module, ext_name)
                        if ext.load():
                            self._extensions[ext_name] = ext

    def _register_arguments(self):

        parser = self._argument_parser
        sub_parsers = parser.add_subparsers(
            title="subcommands",
            dest="subcommand"
        )

        # Main directory is placed here in order to know the path of the auth database.
        parser.add_argument("--main-dir", help="Set the main directory where libraries, assets, versions and "
                                               "binaries (at runtime) are stored. It also contains the launcher "
                                               "authentication database.", dest="main_dir")

        search = sub_parsers.add_parser("search", help="Search for official Minecraft versions.")
        search.add_argument("input")

        start = sub_parsers.add_parser("start", help="Start a Minecraft version, default to the latest release.")
        start.add_argument("--dry", help="Simulate game starting.", default=False, action="store_true")
        start.add_argument("--demo", help="Start game in demo mode.", default=False, action="store_true")
        start.add_argument("--resol", help="Set a custom start resolution (<width>x<height>).", type=self._decode_resolution, dest="resolution")
        start.add_argument("--jvm", help="Set a custom JVM 'javaw' executable path.", default=JVM_EXEC_DEFAULT)
        start.add_argument("--jvm-args", help="Change the default JVM arguments.", default=JVM_ARGS_DEFAULT, dest="jvm_args")
        start.add_argument("--work-dir", help="Set the working directory where the game run and place for examples the saves (and resources for legacy versions).", dest="work_dir")
        start.add_argument("--work-dir-bin", help="Flag to force temporary binaries to be copied inside working directory, by default they are copied into main directory.", default=False, action="store_true", dest="work_dir_bin")
        start.add_argument("--no-better-logging", help="Disable the better logging configuration built by the launcher in order to improve the log readability in the console.", default=False, action="store_true", dest="no_better_logging")
        start.add_argument("-t", "--temp-login", help="Flag used with -l (--login) to tell launcher not to cache your session if not already cached, deactivated by default.", default=False, action="store_true", dest="templogin")
        start.add_argument("-l", "--login", help="Use a email or username (legacy) to authenticate using mojang servers (you will be asked for password, it override --username and --uuid).")
        start.add_argument("-u", "--username", help="Set a custom user name to play.")
        start.add_argument("-i", "--uuid", help="Set a custom user UUID to play.")
        start.add_argument("version", nargs="?", default="release")

        login = sub_parsers.add_parser("login", help="Login into your Mojang account, this will cache your tokens.")
        login.add_argument("email_or_username")

        logout = sub_parsers.add_parser("logout", help="Logout from your Mojang account.")
        logout.add_argument("email_or_username")

        sub_parsers.add_parser("listext", help="List extensions.")

        self.trigger_event("register_arguments", parser=parser)

    # Builtin subcommands

    def _cmd_search(self, args: Namespace):
        print("Searching for version '{}' ...".format(args.input))
        manifest = self.get_version_manifest()
        found = False
        for version_data in manifest.search_versions(args.input):
            found = True
            # Fix the wrong version id of 1.14 pre releases (just for displaying)
            vid = version_data["id"].replace(" Pre-Release ", "-pre")
            print("=> {:10s} {:16s} {}{}".format(
                version_data["type"],
                vid,
                self.format_iso_date(version_data["releaseTime"]),
                "  real id: {}".format(version_data["id"]) if version_data["id"] != vid else ""
            ))
        if not found:
            print("=> No version found")
            exit(EXIT_VERSION_SEARCH_NOT_FOUND)
        else:
            exit(0)

    def _cmd_login(self, args: Namespace):
        self.promp_password_and_authenticate(args.email_or_username, True)

    def _cmd_logout(self, args: Namespace):
        email_or_username = args.email_or_username
        print("Logging out from {}...".format(email_or_username))
        auth_db = self.get_auth_database()
        auth_db.load()
        entry = auth_db.get_entry(email_or_username)
        if entry is not None:
            entry.invalidate()
            auth_db.remove_entry(email_or_username)
            auth_db.save()
            print("=> Logged out.")
        else:
            print("=> This session is not cached.")

    def _cmd_listext(self, _args: Namespace):
        print("Extensions list ({}):".format(len(self._extensions)))
        for ext in self._extensions.values():
            print("- {}, version: {}, authors: {}".format(ext.name, ext.version, ", ".join(ext.authors)))

    def _cmd_start(self, args: Namespace):

        print("Welcome to PortableMC, the easy to use Python Minecraft Launcher.")

        # Get all arguments
        work_dir = self._main_dir if args.main_dir is None else path.realpath(args.main_dir)
        dry_run = args.dry
        uuid = None if args.uuid is None else args.uuid.replace("-", "")
        username = args.username

        # Login if needed
        if args.login is not None:
            auth_entry = self.promp_password_and_authenticate(args.login, not args.templogin)
            if auth_entry is None:
                exit(EXIT_AUTHENTICATION_FAILED)
            uuid = auth_entry.uuid
            username = auth_entry.username
        else:
            auth_entry = None

        # Setup defaut UUID and/or username if needed
        if uuid is None: uuid = uuid4().hex
        if username is None: username = uuid[:8]

        # Resolve version metadata
        try:
            version, version_alias = self.get_version_manifest().filter_latest(args.version)
            version_meta, version_dir = self.resolve_version_meta_recursive(version)
        except VersionNotFoundError:
            exit(EXIT_VERSION_NOT_FOUND)
            return  # Return to avoid alert for unknown variables

        # Starting version dependencies resolving
        version_type = version_meta["type"]
        print("Loading {} {}...".format(version_type, version))

        # JAR file loading
        print("Loading jar file...")
        version_jar_file = path.join(version_dir, "{}.jar".format(version))
        if not path.isfile(version_jar_file):
            version_downloads = version_meta["downloads"]
            if "client" not in version_downloads:
                print("=> Can't found client download in version meta")
                exit(EXIT_CLIENT_JAR_NOT_FOUND)
            if self.download_file_info_progress(version_downloads["client"], version_jar_file) is None:
                exit(EXIT_DOWNLOAD_FILE_CORRUPTED)

        # Assets loading
        print("Loading assets...")
        assets_dir = path.join(self._main_dir, "assets")
        assets_indexes_dir = path.join(assets_dir, "indexes")
        assets_index_version = version_meta["assets"]
        assets_index_file = path.join(assets_indexes_dir, "{}.json".format(assets_index_version))
        assets_index = None

        if path.isfile(assets_index_file):
            with open(assets_index_file, "rb") as assets_index_fp:
                try:
                    assets_index = json.load(assets_index_fp)
                except JSONDecodeError:
                    print("=> Failed to decode assets index, try updating...")

        if assets_index is None:
            asset_index_info = version_meta["assetIndex"]
            asset_index_url = asset_index_info["url"]
            print("=> Found asset index in metadata: {}".format(asset_index_url))
            assets_index = self.read_url_json(asset_index_url)
            if not path.isdir(assets_indexes_dir):
                os.makedirs(assets_indexes_dir, 0o777, True)
            with open(assets_index_file, "wt") as assets_index_fp:
                json.dump(assets_index, assets_index_fp)

        assets_objects_dir = path.join(assets_dir, "objects")
        assets_total_size = version_meta["assetIndex"]["totalSize"]
        assets_current_size = 0
        assets_virtual_dir = path.join(assets_dir, "virtual", assets_index_version)
        assets_mapped_to_resources = assets_index.get("map_to_resources", False)  # For version <= 13w23b
        assets_virtual = assets_index.get("virtual", False)  # For 13w23b < version <= 13w48b (1.7.2)

        if assets_mapped_to_resources:
            print("=> This version use lagacy assets, put in {}/resources".format(work_dir))
        if assets_virtual:
            print("=> This version use virtual assets, put in {}".format(assets_virtual_dir))

        print("=> Verifying assets...")
        for asset_id, asset_obj in assets_index["objects"].items():

            asset_hash = asset_obj["hash"]
            asset_hash_prefix = asset_hash[:2]
            asset_size = asset_obj["size"]
            asset_hash_dir = path.join(assets_objects_dir, asset_hash_prefix)
            asset_file = path.join(asset_hash_dir, asset_hash)

            if not path.isfile(asset_file) or path.getsize(asset_file) != asset_size:
                os.makedirs(asset_hash_dir, 0o777, True)
                asset_url = ASSET_BASE_URL.format(asset_hash_prefix, asset_hash)
                assets_current_size = self.download_file_progress(asset_url, asset_size, asset_hash, asset_file,
                                                             start_size=assets_current_size,
                                                             total_size=assets_total_size,
                                                             name=asset_id)
            else:
                assets_current_size += asset_size

            if assets_mapped_to_resources:
                resources_asset_file = path.join(work_dir, "resources", asset_id)
                if not path.isfile(resources_asset_file):
                    os.makedirs(path.dirname(resources_asset_file), 0o777, True)
                    shutil.copyfile(asset_file, resources_asset_file)

            if assets_virtual:
                virtual_asset_file = path.join(assets_virtual_dir, asset_id)
                if not path.isfile(virtual_asset_file):
                    os.makedirs(path.dirname(virtual_asset_file), 0o777, True)
                    shutil.copyfile(asset_file, virtual_asset_file)

        # Logging configuration
        print("Loading logger config...")
        logging_arg = None
        if "logging" in version_meta:
            version_logging = version_meta["logging"]
            if "client" in version_logging:
                log_config_dir = path.join(assets_dir, "log_configs")
                os.makedirs(log_config_dir, 0o777, True)
                client_logging = version_logging["client"]
                logging_file_info = client_logging["file"]
                logging_file = path.join(log_config_dir, logging_file_info["id"])
                logging_dirty = False
                if not path.isfile(logging_file) or path.getsize(logging_file) != logging_file_info["size"]:
                    self.download_file_info_progress(logging_file_info, logging_file, name=logging_file_info["id"])
                    logging_dirty = True
                if not args.no_better_logging:
                    better_logging_file = path.join(log_config_dir, "portablemc-{}".format(logging_file_info["id"]))
                    if logging_dirty or not path.isfile(better_logging_file):
                        print("=> Generating custom logging configuration...")
                        with open(logging_file, "rt") as logging_fp:
                            with open(better_logging_file, "wt") as custom_logging_fp:
                                raw = logging_fp.read()\
                                    .replace("<XMLLayout />", LOGGING_CONSOLE_REPLACEMENT)\
                                    .replace("<LegacyXMLLayout />", LOGGING_CONSOLE_REPLACEMENT)
                                custom_logging_fp.write(raw)
                    logging_file = better_logging_file
                logging_arg = client_logging["argument"].replace("${path}", logging_file)

        # Libraries and natives loading
        print("Loading libraries and natives...")
        libraries_dir = path.join(self._main_dir, "libraries")
        classpath_libs = [version_jar_file]
        native_libs = []

        for lib_obj in version_meta["libraries"]:

            if "rules" in lib_obj:
                if not self.interpret_rule(lib_obj["rules"]):
                    continue

            lib_name = lib_obj["name"]  # type: str
            lib_type = None  # type: Optional[str]

            if "downloads" in lib_obj:

                lib_dl = lib_obj["downloads"]
                lib_dl_info = None

                if "natives" in lib_obj and "classifiers" in lib_dl:
                    lib_natives = lib_obj["natives"]
                    if self._mc_os in lib_natives:
                        lib_native_classifier = lib_natives[self._mc_os]
                        if self._mc_archbits is not None:
                            lib_native_classifier = lib_native_classifier.replace("${arch}", self._mc_archbits)
                        lib_name += ":{}".format(lib_native_classifier)
                        lib_dl_info = lib_dl["classifiers"][lib_native_classifier]
                        lib_type = "native"
                elif "artifact" in lib_dl:
                    lib_dl_info = lib_dl["artifact"]
                    lib_type = "classpath"

                if lib_dl_info is None:
                    print("=> Can't found library for {}".format(lib_name))
                    continue

                lib_path = path.join(libraries_dir, lib_dl_info["path"])
                lib_dir = path.dirname(lib_path)
                lib_size = lib_dl_info["size"]

                os.makedirs(lib_dir, 0o777, True)

                if not path.isfile(lib_path) or path.getsize(lib_path) != lib_size:
                    self.download_file_info_progress(lib_dl_info, lib_path, name=lib_name)

            else:

                # If no 'downloads' trying to parse the maven dependency string "<group>:<product>:<version>
                # to directory path. This may be used by custom configuration that do not provide download
                # links like Optifine.

                lib_name_parts = lib_name.split(":")
                lib_path = path.join(libraries_dir, *lib_name_parts[0].split("."), lib_name_parts[1],
                                        lib_name_parts[2], "{}-{}.jar".format(lib_name_parts[1], lib_name_parts[2]))
                lib_type = "classpath"

                if not path.isfile(lib_path):
                    print("=> Can't found cached library for {} at {}".format(lib_name, lib_path))
                    continue

            if lib_type == "classpath":
                classpath_libs.append(lib_path)
            elif lib_type == "native":
                native_libs.append(lib_path)

        # Don't run if dry run
        if dry_run:
            print("Dry run, stopping.")
            exit(0)

        # Start game
        print("Starting game...")

        # Extracting binaries
        bin_dir = path.join(work_dir if args.work_dir_bin else self._main_dir, "bin", str(uuid4()))

        @atexit.register
        def _bin_dir_cleanup():
            if path.isdir(bin_dir):
                shutil.rmtree(bin_dir)

        print("=> Extracting natives...")
        for native_lib in native_libs:
            with ZipFile(native_lib, 'r') as native_zip:
                for native_zip_info in native_zip.infolist():
                    if self.can_extract_native(native_zip_info.filename):
                        native_zip.extract(native_zip_info, bin_dir)

        # Decode arguments
        custom_resol = args.resolution
        if custom_resol is not None and len(custom_resol) != 2:
            custom_resol = None

        features = {
            "is_demo_user": args.demo,
            "has_custom_resolution": custom_resol is not None
        }

        legacy_args = version_meta.get("minecraftArguments")

        raw_args = []
        raw_args.extend(self.interpret_args(version_meta["arguments"]["jvm"] if legacy_args is None else LEGACY_JVM_ARGUMENTS, features))
        raw_args.extend(args.jvm_args.split(" "))

        if logging_arg is not None:
            raw_args.append(logging_arg)

        main_class = version_meta["mainClass"]
        main_class_launchwrapper = (main_class == "net.minecraft.launchwrapper.Launch")

        if main_class_launchwrapper:
            raw_args.append("-Dminecraft.client.jar={}".format(version_jar_file))

        raw_args.append(main_class)
        raw_args.extend(self.interpret_args(version_meta["arguments"]["game"], features) if legacy_args is None else legacy_args.split(" "))

        # Arguments replacements
        start_args_replacements = {
            # Game
            "auth_player_name": username,
            "version_name": version,
            "game_directory": work_dir,
            "assets_root": assets_dir,
            "assets_index_name": assets_index_version,
            "auth_uuid": uuid,
            "auth_access_token": "" if auth_entry is None else auth_entry.format_token_argument(False),
            "user_type": "mojang",
            "version_type": version_type,
            # Game (legacy)
            "auth_session": "notok" if auth_entry is None else auth_entry.format_token_argument(True),
            "game_assets": assets_virtual_dir,
            "user_properties": "{}",
            # JVM
            "natives_directory": bin_dir,
            "launcher_name": LAUNCHER_NAME,
            "launcher_version": LAUNCHER_VERSION,
            "classpath": self.get_classpath_separator().join(classpath_libs)
        }

        if custom_resol is not None:
            start_args_replacements["resolution_width"] = str(custom_resol[0])
            start_args_replacements["resolution_height"] = str(custom_resol[1])

        start_args = [args.jvm]
        for arg in raw_args:
            for repl_id, repl_val in start_args_replacements.items():
                arg = arg.replace("${{{}}}".format(repl_id), repl_val)
            start_args.append(arg)

        print("=> Running...")
        print("=> Username: {}, UUID: {}".format(username, uuid))
        print("=> Command line: {}".format(" ".join(start_args)))
        print("================================================")
        os.makedirs(work_dir, 0o777, True)
        subprocess.run(start_args, cwd=work_dir)
        print("================================================")
        print("=> Game stopped, removing bin directory...")
        exit(0)

    # Getters

    def get_main_dir(self) -> str:
        return self._main_dir

    def get_version_manifest(self) -> 'VersionManifest':
        if self._version_manifest is None:
            self._version_manifest = VersionManifest.load_from_url()
        return self._version_manifest

    def get_auth_database(self) -> 'AuthDatabase':
        if self._auth_database is None:
            self._auth_database = AuthDatabase(path.join(self._main_dir, "portablemc_tokens"))
        return self._auth_database

    def get_download_buffer(self) -> bytearray:
        if self._download_buffer is None:
            self._download_buffer = bytearray(32768)
        return self._download_buffer

    # Event listeners

    def add_event_listener(self, event: str, listener: Callable):
        listeners = self._event_listeners.get(event)
        if listeners is None:
            listeners = self._event_listeners[event] = set()
        listeners.add(listener)

    def remove_event_listener(self, event: str, listener: Callable):
        listeners = self._event_listeners.get(event)
        if listeners is not None:
            try:
                listeners.remove(listener)
            except KeyError:
                pass

    def trigger_event(self, event: str, *args, **kwargs):
        listeners = self._event_listeners.get(event)
        if listeners is not None:
            for listener in listeners:
                listener(*args, **kwargs)

    # Utils

    def promp_password_and_authenticate(self, email_or_username: str, cache_in_db: bool) -> Optional['AuthEntry']:

        print("Authenticating {}...".format(email_or_username))

        auth_db = self.get_auth_database()
        auth_db.load()

        auth_entry = auth_db.get_entry(email_or_username)
        if auth_entry is not None:
            print("=> Session already cached, validating...")
            if not auth_entry.validate():
                print("=> Session failed to valide, refreshing...")
                try:
                    auth_entry.refresh()
                    auth_db.save()
                    print("=> Session refreshed.")
                    return auth_entry
                except AuthError as auth_err:
                    print("=> {}".format(str(auth_err)))
            else:
                print("=> Session validated.")
                return auth_entry

        import getpass
        password = getpass.getpass("=> Enter {} password: ".format(email_or_username))

        try:
            auth_entry = AuthEntry.authenticate(email_or_username, password)
            if cache_in_db:
                print("=> Caching your session...")
                auth_db.add_entry(email_or_username, auth_entry)
                auth_db.save()
            print("=> Logged in")
            return auth_entry
        except AuthError as auth_err:
            print("=> {}".format(str(auth_err)))
            return None

    def resolve_version_meta(self, name: str) -> Tuple[dict, str]:

        version_dir = path.join(self._main_dir, "versions", name)
        version_meta_file = path.join(version_dir, "{}.json".format(name))
        content = None

        print("Resolving version {}".format(name))

        if path.isfile(version_meta_file):
            print("=> Found cached metadata, loading...")
            with open(version_meta_file, "rb") as version_meta_fp:
                try:
                    content = json.load(version_meta_fp)
                    print("=> Version loaded.")
                except JSONDecodeError:
                    print("=> Failed to decode cached metadata, try updating...")

        if content is None:
            version_data = self.get_version_manifest().get_version(name)
            if version_data is not None:
                version_url = version_data["url"]
                print("=> Found metadata in manifest, caching...")
                content = self.read_url_json(version_url)
                os.makedirs(version_dir, 0o777, True)
                with open(version_meta_file, "wt") as version_meta_fp:
                    json.dump(content, version_meta_fp, indent=2)
            else:
                print("=> Not found in manifest.")
                raise VersionNotFoundError()

        return content, version_dir

    def resolve_version_meta_recursive(self, name: str) -> Tuple[dict, str]:
        version_meta, version_dir = self.resolve_version_meta(name)
        while "inheritsFrom" in version_meta:
            print("=> Parent version: {}".format(version_meta["inheritsFrom"]))
            parent_meta, _ = self.resolve_version_meta(version_meta["inheritsFrom"])
            if parent_meta is None:
                print("=> Failed to find parent version {}".format(version_meta["inheritsFrom"]))
                raise VersionNotFoundError()
            del version_meta["inheritsFrom"]
            self.dict_merge(parent_meta, version_meta)
            version_meta = parent_meta
        return version_meta, version_dir

    def download_file_progress(self, url: str, size: int, sha1: str, dst: str, *, start_size: int = 0, total_size: int = 0, name: Optional[str] = None) -> Optional[int]:

        base_message = "Downloading {}... ".format(url if name is None else name)
        print(base_message, end='')

        success = False

        from urllib import request
        with request.urlopen(url) as req:
            with open(dst, "wb") as dst_fp:

                dl_sha1 = hashlib.sha1()
                dl_size = 0

                buffer = self.get_download_buffer()

                last_time = time.monotonic()

                while True:

                    read_len = req.readinto(buffer)
                    if not read_len:
                        break

                    buffer_view = buffer[:read_len]
                    dl_size += read_len
                    dl_sha1.update(buffer_view)
                    dst_fp.write(buffer_view)
                    progress = dl_size / size * 100
                    print("\r{}{:6.2f}%".format(base_message, progress), end='')

                    if total_size != 0:
                        start_size += read_len
                        progress = start_size / total_size * 100
                        print("    {:6.2f}% of total".format(progress), end='')

                    now_time = time.monotonic()
                    if now_time != last_time:
                        print("    {}/s   ".format(self.format_bytes(read_len / (now_time - last_time))), end='')
                    last_time = now_time

                if dl_size != size:
                    print(" => Invalid size")
                elif dl_sha1.hexdigest() != sha1:
                    print(" => Invalid SHA1")
                else:
                    print()
                    success = True

        return start_size if success else None

    def download_file_info_progress(self, info: dict, dst: str, *, start_size: int = 0, total_size: int = 0, name: Optional[str] = None) -> Optional[int]:
        return self.download_file_progress(info["url"], info["size"], info["sha1"], dst, start_size=start_size, total_size=total_size, name=name)

    def interpret_rule(self, rules: list, features: Optional[dict] = None) -> bool:
        allowed = False
        for rule in rules:
            if "os" in rule:
                ros = rule["os"]
                if "name" in ros and ros["name"] != self._mc_os:
                    continue
                elif "arch" in ros and ros["arch"] != self._mc_arch:
                    continue
                elif "version" in ros and re.compile(ros["version"]).search(platform.version()) is None:
                    continue
            if "features" in rule:
                feature_valid = True
                for feat_name, feat_value in rule["features"].items():
                    if feat_name not in features or feat_value != features[feat_name]:
                        feature_valid = False
                        break
                if not feature_valid:
                    continue
            act = rule["action"]
            if act == "allow":
                allowed = True
            elif act == "disallow":
                allowed = False
        return allowed

    def interpret_args(self, args: list, features: dict) -> list:
        ret = []
        for arg in args:
            if isinstance(arg, str):
                ret.append(arg)
            else:
                if "rules" in arg:
                    if not self.interpret_rule(arg["rules"], features):
                        continue
                arg_value = arg["value"]
                if isinstance(arg_value, list):
                    ret.extend(arg_value)
                elif isinstance(arg_value, str):
                    ret.append(arg_value)
        return ret

    @staticmethod
    def _decode_resolution(raw: str):
        return tuple(int(size) for size in raw.split("x"))

    @staticmethod
    def get_minecraft_dir() -> str:
        pf = sys.platform
        home = path.expanduser("~")
        if pf.startswith("freebsd") or pf.startswith("linux") or pf.startswith("aix") or pf.startswith("cygwin"):
            return path.join(home, ".minecraft")
        elif pf == "win32":
            return path.join(home, "AppData", "Roaming", ".minecraft")
        elif pf == "darwin":
            return path.join(home, "Library", "Application Support", "minecraft")

    @staticmethod
    def get_minecraft_os() -> str:
        pf = sys.platform
        if pf.startswith("freebsd") or pf.startswith("linux") or pf.startswith("aix") or pf.startswith("cygwin"):
            return "linux"
        elif pf == "win32":
            return "windows"
        elif pf == "darwin":
            return "osx"

    @staticmethod
    def get_minecraft_arch() -> str:
        machine = platform.machine().lower()
        return "x86" if machine == "i386" else "x86_64" if machine in ("x86_64", "amd64") else "unknown"

    @staticmethod
    def get_minecraft_archbits() -> Optional[str]:
        raw_bits = platform.architecture()[0]
        return "64" if raw_bits == "64bit" else "32" if raw_bits == "32bit" else None

    @staticmethod
    def get_classpath_separator() -> str:
        return ";" if sys.platform == "win32" else ":"

    @staticmethod
    def read_url_json(url: str) -> dict:
        from urllib import request
        return json.load(request.urlopen(url))

    @staticmethod
    def format_iso_date(raw: str):
        return datetime.strptime(raw.rsplit("+", 2)[0], "%Y-%m-%dT%H:%M:%S").strftime("%c")

    @classmethod
    def dict_merge(cls, dst: dict, other: dict):
        for k, v in other.items():
            if k in dst:
                if isinstance(dst[k], dict) and isinstance(other[k], dict):
                    cls.dict_merge(dst[k], other[k])
                    continue
                elif isinstance(dst[k], list) and isinstance(other[k], list):
                    dst[k].extend(other[k])
                    continue
            dst[k] = other[k]

    @staticmethod
    def format_bytes(n: float) -> str:
        if n < 1000:
            return "{:4.0f}B".format(int(n))
        elif n < 1000000:
            return "{:4.0f}kB".format(n // 1000)
        elif n < 1000000000:
            return "{:4.0f}MB".format(n // 1000000)
        else:
            return "{:4.0f}GB".format(n // 1000000000)

    @staticmethod
    def can_extract_native(filename: str) -> bool:
        return not filename.startswith("META-INF") and not filename.endswith(".git") and not filename.endswith(".sha1")


class VersionManifest:

    def __init__(self, data: dict):
        self._data = data

    @classmethod
    def load_from_url(cls):
        return cls(PortableMC.read_url_json(VERSION_MANIFEST_URL))

    def filter_latest(self, version: str) -> Tuple[Optional[str], bool]:
        return (self._data["latest"][version], True) if version in self._data["latest"] else (version, False)

    def get_version(self, version: str) -> Optional[dict]:
        version, _alias = self.filter_latest(version)
        for version_data in self._data["versions"]:
            if version_data["id"] == version:
                return version_data
        return None

    def search_versions(self, inp: str) -> Generator[dict, None, None]:
        inp, alias = self.filter_latest(inp)
        for version_data in self._data["versions"]:
            if alias and version_data["id"] == inp:
                yield version_data
            elif not alias and inp in version_data["id"]:
                yield version_data


class AuthEntry:

    def __init__(self, client_token: str, username: str, uuid: str, access_token: str):
        self.client_token = client_token
        self.username = username
        self.uuid = uuid  # No dashes
        self.access_token = access_token

    def format_token_argument(self, legacy: bool) -> str:
        if legacy:
            return "token:{}:{}".format(self.access_token, self.uuid)
        else:
            return self.access_token

    def validate(self) -> bool:
        return self.auth_request("validate", {
            "accessToken": self.access_token,
            "clientToken": self.client_token
        }, False)[0] == 204

    def refresh(self):

        _, res = self.auth_request("refresh", {
            "accessToken": self.access_token,
            "clientToken": self.client_token
        })

        self.access_token = res["accessToken"]

    def invalidate(self):
        self.auth_request("invalidate", {
            "accessToken": self.access_token,
            "clientToken": self.client_token
        }, False)

    @classmethod
    def authenticate(cls, email_or_username: str, password: str) -> 'AuthEntry':

        _, res = cls.auth_request("authenticate", {
            "agent": {
                "name": "Minecraft",
                "version": 1
            },
            "username": email_or_username,
            "password": password,
            "clientToken": uuid4().hex
        })

        return AuthEntry(
            res["clientToken"],
            res["selectedProfile"]["name"],
            res["selectedProfile"]["id"],
            res["accessToken"]
        )

    @staticmethod
    def auth_request(req: str, payload: dict, error: bool = True) -> (int, dict):

        from http.client import HTTPResponse
        from urllib.request import Request
        from urllib.error import HTTPError
        from urllib import request

        req_url = AUTHSERVER_URL.format(req)
        data = json.dumps(payload).encode("ascii")
        req = Request(req_url, data, headers={
            "Content-Type": "application/json",
            "Content-Length": len(data)
        }, method="POST")

        try:
            res = request.urlopen(req)  # type: HTTPResponse
        except HTTPError as err:
            res = cast(HTTPResponse, err.fp)

        try:
            res_data = json.load(res)
        except JSONDecodeError:
            res_data = {}

        if error and res.status != 200:
            raise AuthError(res_data["errorMessage"])

        return res.status, res_data


class AuthDatabase:

    def __init__(self, filename: str):
        self._filename = filename
        self._entries = {}  # type: Dict[str, AuthEntry]

    def load(self):
        self._entries.clear()
        if path.isfile(self._filename):
            with open(self._filename, "rt") as fp:
                for line in fp.readlines():
                    parts = line.split(" ")
                    if len(parts) == 5:
                        self._entries[parts[0]] = AuthEntry(
                            parts[1],
                            parts[2],
                            parts[3],
                            parts[4]
                        )

    def save(self):
        with open(self._filename, "wt") as fp:
            fp.writelines(("{} {} {} {} {}".format(
                email_or_username,
                entry.client_token,
                entry.username,
                entry.uuid,
                entry.access_token
            ) for email_or_username, entry in self._entries.items()))

    def get_entry(self, email_or_username: str) -> Optional[AuthEntry]:
        return self._entries.get(email_or_username, None)

    def add_entry(self, email_or_username: str, entry: AuthEntry):
        self._entries[email_or_username] = entry

    def remove_entry(self, email_or_username: str):
        if email_or_username in self._entries:
            del self._entries[email_or_username]


class AuthError(Exception):
    pass


class VersionNotFoundError(Exception):
    pass


class PortableExtension:

    def __init__(self, module: Any, name: str):

        self.module = module
        self.name = str(module.NAME) if hasattr(module, "NAME") else name
        self.version = str(module.VERSION) if hasattr(module, "VERSION") else "unknown"
        self.authors = module.AUTHORS if hasattr(module, "AUTHORS") else tuple()

        if not isinstance(self.authors, tuple):
            self.authors = (str(self.authors),)

    def load(self) -> bool:
        try:
            self.module.load(self)
            return True
        except (Exception,):
            import traceback
            print("Failed to load extension '{}' (contact extension authors):".format(self.name))
            traceback.print_exc()
            return False


if __name__ == '__main__':
    PortableMC().start()


LEGACY_JVM_ARGUMENTS = [
    {
        "rules": [
            {
                "action": "allow",
                "os": {
                    "name": "osx"
                }
            }
        ],
        "value": [
            "-XstartOnFirstThread"
        ]
    },
    {
        "rules": [
            {
                "action": "allow",
                "os": {
                    "name": "windows"
                }
            }
        ],
        "value": "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"
    },
    {
        "rules": [
            {
                "action": "allow",
                "os": {
                    "name": "windows",
                    "version": "^10\\."
                }
            }
        ],
        "value": [
            "-Dos.name=Windows 10",
            "-Dos.version=10.0"
        ]
    },
    "-Djava.library.path=${natives_directory}",
    "-Dminecraft.launcher.brand=${launcher_name}",
    "-Dminecraft.launcher.version=${launcher_version}",
    "-cp",
    "${classpath}"
]
