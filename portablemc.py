#!/usr/bin/env python
from urllib.request import Request as URLRequest
from http.client import HTTPResponse
from urllib import request as urlreq
from urllib.error import HTTPError

from json.decoder import JSONDecodeError
from argparse import ArgumentParser
from os import path as os_path
from datetime import datetime
from zipfile import ZipFile
import subprocess
import platform
import hashlib
import getpass
import shutil
import uuid
import json
import time
import sys
import re
import os

from typing import cast, Optional, Tuple, List, Dict


VERSION_MANIFEST_URL = "https://launchermeta.mojang.com/mc/game/version_manifest.json"
ASSET_BASE_URL = "https://resources.download.minecraft.net/{}/{}"
AUTHSERVER_URL = "https://authserver.mojang.com/{}"
SPECIAL_VERSIONS = {"snapshot", "release"}

LAUNCHER_NAME = "portablemc"
LAUNCHER_VERSION = "1.0.0"

JVM_EXEC_DEFAULT = "java"
JVM_ARGS_DEFAULT = "-Xmx2G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M"
DOWNLOAD_BUFFER_SIZE = 32768

EXIT_VERSION_NOT_FOUND = 10
EXIT_CLIENT_JAR_NOT_FOUND = 11
EXIT_NATIVES_DIR_ALREADY_EXITS = 12
EXIT_DOWNLOAD_FILE_CORRUPTED = 13
EXIT_AUTHENTICATION_FAILED = 14
EXIT_VERSION_SEARCH_NOT_FOUND = 15
EXIT_DEPRECATED_ARGUMENT = 15

DECODE_RESOLUTION = lambda raw: tuple(int(size) for size in raw.split("x"))


def main():

    player_uuid = str(uuid.uuid4())

    parser = ArgumentParser(
        allow_abbrev=False,
        description="[PortableMC] "
                    "An easy to use portable Minecraft launcher in only one Python script ! "
                    "This single-script launcher is still compatible with the official (Mojang) Minecraft "
                    "Launcher stored in .minecraft and use it."
    )
    parser.add_argument("-v", "--version", help="Specify Minecraft version (exact version, 'snapshot' for latest snapshot, 'release' for latest release).", default="release")
    parser.add_argument("-s", "--search", help="A flag that exit the launcher after searching version, with this flag the version argument can be inexact.", default=False, action="store_true")
    parser.add_argument("--nostart", help="Only download Minecraft required data, but does not launch the game.", default=False, action="store_true")
    parser.add_argument("--demo", help="Start game in demo mode.", default=False, action="store_true")
    parser.add_argument("--resol", help="Set a custom start resolution (<width>x<height>).", type=DECODE_RESOLUTION, dest="resolution")
    parser.add_argument("--java", help="Set a custom JVM javaw executable path (deprecated, use --jvm).")
    parser.add_argument("--jvm", help="Set a custom JVM javaw executable path.", default=JVM_EXEC_DEFAULT)
    parser.add_argument("--jvm-args", help="Change the default JVM arguments.", default=JVM_ARGS_DEFAULT, dest="jvm_args")
    parser.add_argument("--logout", help="Override all other arguments, does not start the game, and logout from this specific session.")
    parser.add_argument("-md", "--main-dir", help="Set the main directory where libraries, assets, versions and binaries (at runtime) are stored.", dest="main_dir")
    parser.add_argument("-wd", "--work-dir", help="Set the working directory where the game run and place for examples the saves (and resources for legacy versions).", dest="work_dir")
    parser.add_argument("-t", "--temp-login", help="Flag used with -l (--login) to tell launcher not to cache your session if not already cached, deactivated by default.", default=False, action="store_true", dest="templogin")
    parser.add_argument("-l", "--login", help="Use a email or username (legacy) to authenticate using mojang servers (you will be asked for password, it override --username and --uuid).")
    parser.add_argument("-u", "--username", help="Set a custom user name to play.", default=player_uuid.split("-")[0])
    parser.add_argument("-i", "--uuid", help="Set a custom user UUID to play.", default=player_uuid)
    args = parser.parse_args()

    if args.java is not None:
        print("The '--java' argument is deprecated, please use --jvm and --jvm-args")
        exit(EXIT_DEPRECATED_ARGUMENT)

    main_dir = get_minecraft_dir("minecraft") if args.main_dir is None else os_path.realpath(args.main_dir)
    work_dir = main_dir if args.work_dir is None else os_path.realpath(args.work_dir)
    os.makedirs(main_dir, 0o777, True)

    auth_db_file = os_path.join(main_dir, "portablemc_tokens")

    print("Welcome to PortableMC, the easy to use Python Minecraft Launcher.")
    print("=> Main directory: {}".format(main_dir))
    print("=> Working directory: {}".format(work_dir))
    print("=> Version manifest url: {}".format(VERSION_MANIFEST_URL))

    if not os_path.isfile(auth_db_file):
        if input("Continue using this main directory? (y/N) ") != "y":
            print("=> Abort")
            exit(0)

    # Logging out
    if args.logout is not None:

        print("=> Logging out...")
        username = args.logout
        auth_db = AuthDatabase(auth_db_file)
        auth_db.load()
        auth_entry = auth_db.get_entry(username)

        if auth_entry is None:
            print("=> Session {} is not cached".format(username))
            exit(0)

        auth_invalidate(auth_entry)
        auth_db.remove_entry(username)
        auth_db.save()
        print("=> Session {} is no longer valid".format(username))
        exit(0)

    # Searching version
    mc_os = get_minecraft_os()
    version_manifest = read_version_manifest()
    version_name = args.version
    print("Searching for version '{}' ...".format(version_name))

    # Aliasing version
    if version_name in SPECIAL_VERSIONS and version_name in version_manifest["latest"]:
        version_type = version_name
        version_name = version_manifest["latest"][version_type]
        print("=> Latest {} is '{}'".format(version_type, version_name))

    # If searching flag is True
    if args.search:
        found = False
        for manifest_version in version_manifest["versions"]:
            if manifest_version["id"].startswith(version_name):
                found = True
                print("=> {:10s} {:16s} {}".format(
                    manifest_version["type"],
                    manifest_version["id"],
                    format_manifest_date(manifest_version["releaseTime"])
                ))
        if not found:
            print("=> No version found")
            exit(EXIT_VERSION_SEARCH_NOT_FOUND)
        else:
            exit(0)

    # Login in
    auth_access_token = ""
    if args.login is not None:

        print("=> Logging in...")
        login = args.login
        auth_db = AuthDatabase(auth_db_file)
        auth_db.load()
        auth_entry = auth_db.get_entry(login)

        if auth_entry is not None and not auth_validate_request(auth_entry):
            print("=> Session {} is not validated, refreshing...".format(login))
            try:
                auth_refresh_request(auth_entry)
                auth_db.save()
            except AuthError as auth_err:
                print("=> {}".format(str(auth_err)))
                auth_entry = None

        if auth_entry is None:
            client_uuid = uuid.uuid4().hex
            password = getpass.getpass("=> Enter {} password: ".format(login))
            try:
                auth_entry = auth_authenticate_request(login, password, client_uuid)
                if not args.templogin:
                    print("=> Caching your session...")
                    auth_db.add_entry(login, auth_entry)
                    auth_db.save()
            except AuthError as auth_err:
                print("=> {}".format(str(auth_err)))
                exit(EXIT_AUTHENTICATION_FAILED)

        args.username = auth_entry.username
        args.uuid = auth_entry.uuid
        auth_access_token = auth_entry.access_token
        print("=> Logged in")

    elif args.uuid is not None:  # Remove dashes from UUID
        args.uuid = args.uuid.replace("-", "")

    # Version meta file caching

    def ensure_version_meta(name: str) -> Tuple[dict, str]:

        version_dir = os_path.join(main_dir, "versions", name)
        version_meta_file = os_path.join(version_dir, "{}.json".format(name))
        content = None

        if os_path.isfile(version_meta_file):
            print("=> Found cached version meta: {}".format(version_meta_file))
            with open(version_meta_file, "rb") as version_meta_fp:
                try:
                    content = json.load(version_meta_fp)
                except JSONDecodeError:
                    print("=> Failed to decode cached version meta, try updating ...")

        if content is None:
            for mf_version in version_manifest["versions"]:
                if mf_version["id"] == name:
                    version_url = mf_version["url"]
                    print("=> Found version meta in manifest, caching: {}".format(version_url))
                    content = read_url_json(version_url)
                    os.makedirs(version_dir, 0o777, True)
                    with open(version_meta_file, "wt") as version_meta_fp:
                        json.dump(content, version_meta_fp, indent=2)

        return content, version_dir

    version_meta, version_dir = ensure_version_meta(version_name)

    if version_meta is None:
        print("=> Failed to find version '{}'".format(args.version))
        exit(EXIT_VERSION_NOT_FOUND)

    while "inheritsFrom" in version_meta:
        print("=> Version '{}' inherits version '{}'...".format(version_meta["id"], version_meta["inheritsFrom"]))
        parent_meta, _ = ensure_version_meta(version_meta["inheritsFrom"])
        if parent_meta is None:
            print("=> Failed to find parent version '{}'".format(version_meta["inheritsFrom"]))
            exit(EXIT_VERSION_NOT_FOUND)
        del version_meta["inheritsFrom"]
        dict_merge(parent_meta, version_meta)
        version_meta = parent_meta

    # Loading version dependencies
    version_type = version_meta["type"]
    print("Loading {} {}...".format(version_type, version_name))

    # Common buffer to avoid realloc
    buffer = bytearray(DOWNLOAD_BUFFER_SIZE)

    # JAR file loading
    print("Loading jar file...")
    version_jar_file = os_path.join(version_dir, "{}.jar".format(version_name))
    if not os_path.isfile(version_jar_file):
        version_downloads = version_meta["downloads"]
        if "client" not in version_downloads:
            print("=> Can't found client download in version meta")
            exit(EXIT_CLIENT_JAR_NOT_FOUND)
        download_file_info_progress(version_downloads["client"], version_jar_file, buffer=buffer)

    # Assets loading
    print("Loading assets...")
    assets_dir = os_path.join(main_dir, "assets")
    assets_indexes_dir = os_path.join(assets_dir, "indexes")
    assets_index_version = version_meta["assets"]
    assets_index_file = os_path.join(assets_indexes_dir, "{}.json".format(assets_index_version))
    assets_index = None

    if os_path.isfile(assets_index_file):
        print("=> Found cached assets index: {}".format(assets_index_file))
        with open(assets_index_file, "rb") as assets_index_fp:
            try:
                assets_index = json.load(assets_index_fp)
            except JSONDecodeError:
                print("=> Failed to decode assets index, try updating...")

    if assets_index is None:
        asset_index_info = version_meta["assetIndex"]
        asset_index_url = asset_index_info["url"]
        print("=> Found asset index in version meta: {}".format(asset_index_url))
        assets_index = read_url_json(asset_index_url)
        if not os_path.isdir(assets_indexes_dir):
            os.makedirs(assets_indexes_dir, 0o777, True)
        with open(assets_index_file, "wt") as assets_index_fp:
            json.dump(assets_index, assets_index_fp)

    assets_objects_dir = os_path.join(assets_dir, "objects")
    assets_total_size = version_meta["assetIndex"]["totalSize"]
    assets_current_size = 0
    assets_virtual_dir = os_path.join(assets_dir, "virtual", assets_index_version)
    assets_mapped_to_resources = assets_index.get("map_to_resources", False)  # For version <= 13w23b
    assets_virtual = assets_index.get("virtual", False)  # For 13w23b < version <= 13w48b (1.7.2)

    if assets_mapped_to_resources:
        print("=> This version use lagacy assets, put in {}/resources".format(work_dir))
    if assets_virtual:
        print("=> This version use virtual assets, put in {}".format(assets_virtual_dir))

    for asset_id, asset_obj in assets_index["objects"].items():

        asset_hash = asset_obj["hash"]
        asset_hash_prefix = asset_hash[:2]
        asset_size = asset_obj["size"]
        asset_url = ASSET_BASE_URL.format(asset_hash_prefix, asset_hash)
        asset_hash_dir = os_path.join(assets_objects_dir, asset_hash_prefix)
        asset_file = os_path.join(asset_hash_dir, asset_hash)

        if not os_path.isfile(asset_file) or os_path.getsize(asset_file) != asset_size:
            os.makedirs(asset_hash_dir, 0o777, True)
            assets_current_size = download_file_progress(asset_url, asset_size, asset_hash, asset_file,
                                                         start_size=assets_current_size,
                                                         total_size=assets_total_size,
                                                         name=asset_id,
                                                         buffer=buffer)
        else:
            assets_current_size += asset_size

        if assets_mapped_to_resources:
            resources_asset_file = os_path.join(work_dir, "resources", asset_id)
            if not os_path.isfile(resources_asset_file):
                os.makedirs(os_path.dirname(resources_asset_file), 0o777, True)
                shutil.copyfile(asset_file, resources_asset_file)

        if assets_virtual:
            virtual_asset_file = os_path.join(assets_virtual_dir, asset_id)
            if not os_path.isfile(virtual_asset_file):
                os.makedirs(os_path.dirname(virtual_asset_file), 0o777, True)
                shutil.copyfile(asset_file, virtual_asset_file)

    # Logging setup
    print("Loading logger config...")
    logging_arg = None
    if "logging" in version_meta:
        version_logging = version_meta["logging"]
        if "client" in version_logging:
            log_config_dir = os_path.join(assets_dir, "log_configs")
            os.makedirs(log_config_dir, 0o777, True)
            client_logging = version_logging["client"]
            logging_file_info = client_logging["file"]
            logging_file = os_path.join(log_config_dir, logging_file_info["id"])
            if not os_path.isfile(logging_file) or os_path.getsize(logging_file) != logging_file_info["size"]:
                download_file_info_progress(logging_file_info, logging_file, name=logging_file_info["id"], buffer=buffer)
            logging_arg = client_logging["argument"].replace("${path}", logging_file)

    # Libraries and natives loading
    print("Loading libraries and natives...")
    libraries_dir = os_path.join(main_dir, "libraries")

    main_class = version_meta["mainClass"]
    main_class_launchwrapper = (main_class == "net.minecraft.launchwrapper.Launch")
    classpath_libs = [version_jar_file]
    native_libs = []
    
    archbits = get_minecraft_archbits()

    for lib_obj in version_meta["libraries"]:

        if "rules" in lib_obj:
            if not interpret_rule(lib_obj["rules"], mc_os):
                continue

        lib_name = lib_obj["name"]  # type: str
        lib_type = None  # type: Optional[str]

        if "downloads" in lib_obj:

            lib_dl = lib_obj["downloads"]
            lib_dl_info = None

            if "natives" in lib_obj and "classifiers" in lib_dl:
                lib_natives = lib_obj["natives"]
                if mc_os in lib_natives:
                    lib_native_classifier = lib_natives[mc_os]
                    if archbits is not None:
                        lib_native_classifier = lib_native_classifier.replace("${arch}", archbits)
                    lib_name += ":{}".format(lib_native_classifier)
                    lib_dl_info = lib_dl["classifiers"][lib_native_classifier]
                    lib_type = "native"
            elif "artifact" in lib_dl:
                lib_dl_info = lib_dl["artifact"]
                lib_type = "classpath"

            if lib_dl_info is None:
                print("=> Can't found library for {}".format(lib_name))
                continue

            lib_path = os_path.join(libraries_dir, lib_dl_info["path"])
            lib_dir = os_path.dirname(lib_path)
            lib_size = lib_dl_info["size"]

            os.makedirs(lib_dir, 0o777, True)

            if not os_path.isfile(lib_path) or os_path.getsize(lib_path) != lib_size:
                download_file_info_progress(lib_dl_info, lib_path, name=lib_name, buffer=buffer)

        else:

            # If no 'downloads' trying to parse the maven dependency string "<group>:<product>:<version>
            # to directory path. This may be used by custom configuration that do not provide download
            # links like Optifine.

            lib_name_parts = lib_name.split(":")
            lib_path = os_path.join(libraries_dir, *lib_name_parts[0].split("."), lib_name_parts[1], lib_name_parts[2], "{}-{}.jar".format(lib_name_parts[1], lib_name_parts[2]))
            lib_type = "classpath"

            if not os_path.isfile(lib_path):
                print("=> Can't found cached library for {} at {}".format(lib_name, lib_path))
                continue

        if lib_type == "classpath":
            classpath_libs.append(lib_path)
        elif lib_type == "native":
            native_libs.append(lib_path)

    if args.nostart:
        print("Not starting")
        exit(0)

    # Start game
    print("Starting game ...")

    # Extracting binaries
    bin_dir = os_path.join(main_dir, "bin", str(uuid.uuid4()))

    if os_path.isdir(bin_dir):
        print("=> Natives directory already exists at: {}".format(bin))
        exit(EXIT_NATIVES_DIR_ALREADY_EXITS)
    else:
        os.makedirs(bin_dir, 0o777, True)

    print("=> Extracting natives...")
    for native_lib in native_libs:
        with ZipFile(native_lib, 'r') as native_zip:
            for native_zip_info in native_zip.infolist():
                if is_native_zip_info_valid(native_zip_info.filename):
                    native_zip.extract(native_zip_info, bin_dir)

    # Decode arguments
    custom_resol = args.resolution
    if custom_resol is None or len(custom_resol) != 2:
        custom_resol = None

    raw_args = []  # type: List[str]
    features = {
        "is_demo_user": args.demo,
        "has_custom_resolution": custom_resol is not None
    }

    legacy_args = version_meta.get("minecraftArguments")  # type: Optional[str]

    if legacy_args is not None:
        raw_args.extend(interpret_args(LEGACY_JVM_ARGUMENTS, mc_os, features))
    else:
        raw_args.extend(interpret_args(version_meta["arguments"]["jvm"], mc_os, features))

    raw_args.extend(args.jvm_args.split(" "))

    # Default JVM arguments :
    # -Xmx2G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M
    # raw_args.append("-Xmx2G")
    # raw_args.append("-XX:+UnlockExperimentalVMOptions")
    # raw_args.append("-XX:+UseG1GC")
    # raw_args.append("-XX:G1NewSizePercent=20")
    # raw_args.append("-XX:G1ReservePercent=20")
    # raw_args.append("-XX:MaxGCPauseMillis=50")
    # raw_args.append("-XX:G1HeapRegionSize=32M")

    if logging_arg is not None:
        raw_args.append(logging_arg)

    if main_class_launchwrapper:
        raw_args.append("-Dminecraft.client.jar={}".format(version_jar_file))

    raw_args.append(main_class)

    if legacy_args is not None:
        raw_args.extend(legacy_args.split(" "))
    else:
        raw_args.extend(interpret_args(version_meta["arguments"]["game"], mc_os, features))

    # Arguments replacements
    start_args_replacements = {
        # Game
        "auth_player_name": args.username,
        "version_name": version_name,
        "game_directory": work_dir,
        "assets_root": assets_dir,
        "assets_index_name": assets_index_version,
        "auth_uuid": args.uuid,
        "auth_access_token": auth_access_token,
        "user_type": "mojang",
        "version_type": version_type,
        # Game (legacy)
        "auth_session": "token:{}:{}".format(auth_access_token, args.uuid) if len(auth_access_token) else "notok",
        "game_assets": assets_virtual_dir,
        "user_properties": "{}",
        # JVM
        "natives_directory": bin_dir,
        "launcher_name": LAUNCHER_NAME,
        "launcher_version": LAUNCHER_VERSION,
        "classpath": get_classpath_separator().join(classpath_libs)
    }

    if custom_resol is not None:
        start_args_replacements["resolution_width"] = str(custom_resol[0])
        start_args_replacements["resolution_height"] = str(custom_resol[1])

    jvm_path = args.jvm
    start_args = [*jvm_path.split(" ")]
    for arg in raw_args:
        for repl_id, repl_val in start_args_replacements.items():
            arg = arg.replace("${{{}}}".format(repl_id), repl_val)
        start_args.append(arg)

    print("=> Running...")
    print("=> Command line: {}".format(" ".join(start_args)))
    print("================================================")
    os.makedirs(work_dir, 0o777, True)
    subprocess.run(start_args, cwd=work_dir)
    print("================================================")
    print("=> Game stopped, removing bin directory...")
    shutil.rmtree(bin_dir)


#############
##  Utils  ##
#############

def read_url_json(url: str) -> dict:
    return json.load(urlreq.urlopen(url))


def dict_merge(dst: dict, other: dict):
    for k, v in other.items():
        if k in dst:
            if isinstance(dst[k], dict) and isinstance(other[k], dict):
                dict_merge(dst[k], other[k])
                continue
            elif isinstance(dst[k], list) and isinstance(other[k], list):
                dst[k].extend(other[k])
                continue
        dst[k] = other[k]


def read_version_manifest() -> dict:
    return read_url_json(VERSION_MANIFEST_URL)


def get_version_info(version_name: str) -> Optional[Tuple[str, str]]:
    version_manifest = read_version_manifest()
    if version_name in SPECIAL_VERSIONS and version_name in version_manifest["latest"]:
        version_name = version_manifest["latest"][version_name]
    for version in version_manifest["versions"]:
        if version["id"] == version_name:
            return version["type"], version["url"]


def get_minecraft_dir(dirname: str) -> str:
    pf = sys.platform
    home = os_path.expanduser("~")
    if pf.startswith("freebsd") or pf.startswith("linux") or pf.startswith("aix") or pf.startswith("cygwin"):
        return os_path.join(home, ".{}".format(dirname))
    elif pf == "win32":
        return os_path.join(home, "AppData", "Roaming", ".{}".format(dirname))
    elif pf == "darwin":
        return os_path.join(home, "Library", "Application Support", dirname)


def get_classpath_separator() -> str:
    return ";" if sys.platform == "win32" else ":"


def get_minecraft_os() -> str:
    pf = sys.platform
    if pf.startswith("freebsd") or pf.startswith("linux") or pf.startswith("aix") or pf.startswith("cygwin"):
        return "linux"
    elif pf == "win32":
        return "windows"
    elif pf == "darwin":
        return "osx"


def get_minecraft_arch() -> str:
    machine = platform.machine().lower()
    return "x86" if machine == "i386" else "x86_64" if machine in ("x86_64", "amd64") else "unknown"


def get_minecraft_archbits() -> Optional[str]:
    raw_bits = platform.architecture()[0]
    return "64" if raw_bits == "64bit" else "32" if raw_bits == "32bit" else None


def interpret_rule(rules: list, mc_os: str, features: Optional[dict] = None) -> bool:
    allowed = False
    for rule in rules:
        if "os" in rule:
            ros = rule["os"]
            if "name" in ros and ros["name"] != mc_os:
                continue
            elif "arch" in ros and ros["arch"] != get_minecraft_arch():
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


def interpret_args(args: list, mc_os: str, features: dict) -> list:
    ret = []
    for arg in args:
        if isinstance(arg, str):
            ret.append(arg)
        else:
            if "rules" in arg:
                if not interpret_rule(arg["rules"], mc_os, features):
                    continue
            arg_value = arg["value"]
            if isinstance(arg_value, list):
                ret.extend(arg_value)
            elif isinstance(arg_value, str):
                ret.append(arg_value)
    return ret


def is_native_zip_info_valid(filename: str) -> bool:
    return not filename.startswith("META-INF") and not filename.endswith(".git") and not filename.endswith(".sha1")


def download_file_progress(url: str, size: int, sha1: str, dst: str, *, start_size: int = 0, total_size: int = 0, name: Optional[str] = None, buffer: Optional[bytearray] = None) -> int:

    base_message = "Downloading {} ... ".format(url if name is None else name)
    print(base_message, end='')

    success = False

    with urlreq.urlopen(url) as req:
        with open(dst, "wb") as dst_fp:

            dl_sha1 = hashlib.sha1()
            dl_size = 0

            if buffer is None:
                buffer = bytearray(DOWNLOAD_BUFFER_SIZE)

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
                    print("    {}/s   ".format(format_bytes(read_len / (now_time - last_time))), end='')
                last_time = now_time

            if dl_size != size:
                print(" => Invalid size")
            elif dl_sha1.hexdigest() != sha1:
                print(" => Invalid SHA1")
            else:
                print()
                success = True

    if not success:
        exit(EXIT_DOWNLOAD_FILE_CORRUPTED)
    else:
        return start_size


def download_file_info_progress(info: dict, dst: str, *, start_size: int = 0, total_size: int = 0, name: Optional[str] = None, buffer: Optional[bytearray] = None) -> int:
    return download_file_progress(info["url"], info["size"], info["sha1"], dst, start_size=start_size, total_size=total_size, name=name, buffer=buffer)


def format_manifest_date(raw: str):
    return datetime.strptime(raw.rsplit("+", 2)[0], "%Y-%m-%dT%H:%M:%S").strftime("%c")


def format_bytes(n: float) -> str:
    if n < 1000:
        return "{:4.0f}B".format(int(n))
    elif n < 1000000:
        return "{:4.0f}kB".format(n // 1000)
    elif n < 1000000000:
        return "{:4.0f}MB".format(n // 1000000)
    else:
        return "{:4.0f}GB".format(n // 1000000000)


####################
## Authentication ##
####################

class AuthEntry:
    def __init__(self, client_token: str, username: str, _uuid: str, access_token: str):
        self.client_token = client_token
        self.username = username
        self.uuid = _uuid
        self.access_token = access_token


class AuthDatabase:

    def __init__(self, filename: str):
        self._filename = filename
        self._entries = {}  # type: Dict[str, AuthEntry]

    def load(self):
        self._entries.clear()
        if os_path.isfile(self._filename):
            with open(self._filename, "rt") as fp:
                line = fp.readline()
                if line is not None:
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
                login,
                entry.client_token,
                entry.username,
                entry.uuid,
                entry.access_token
            ) for login, entry in self._entries.items()))

    def get_entry(self, login: str) -> Optional[AuthEntry]:
        return self._entries.get(login, None)

    def add_entry(self, login: str, entry: AuthEntry):
        self._entries[login] = entry

    def remove_entry(self, login: str):
        if login in self._entries:
            del self._entries[login]


def auth_request(req: str, payload: dict, error: bool = True) -> (int, dict):

    req_url = AUTHSERVER_URL.format(req)
    data = json.dumps(payload).encode("ascii")
    req = URLRequest(req_url, data, headers={
        "Content-Type": "application/json",
        "Content-Length": len(data)
    }, method="POST")

    try:
        res = urlreq.urlopen(req)  # type: HTTPResponse
    except HTTPError as err:
        res = cast(HTTPResponse, err.fp)

    try:
        res_data = json.load(res)
    except JSONDecodeError:
        res_data = {}

    if error and res.status != 200:
        raise AuthError(res_data["errorMessage"])

    return res.status, res_data


def auth_authenticate_request(login: str, password: str, client_token: str) -> AuthEntry:

    _, res = auth_request("authenticate", {
        "agent": {
            "name": "Minecraft",
            "version": 1
        },
        "username": login,
        "password": password,
        "clientToken": client_token
    })

    return AuthEntry(res["clientToken"], res["selectedProfile"]["name"], res["selectedProfile"]["id"], res["accessToken"])


def auth_validate_request(auth_entry: AuthEntry) -> bool:
    return auth_request("validate", {
        "accessToken": auth_entry.access_token,
        "clientToken": auth_entry.client_token
    }, False)[0] == 204


def auth_refresh_request(auth_entry: AuthEntry):

    _, res = auth_request("refresh", {
        "accessToken": auth_entry.access_token,
        "clientToken": auth_entry.client_token
    })

    auth_entry.access_token = res["accessToken"]


def auth_invalidate(auth_entry: AuthEntry):
    auth_request("invalidate", {
        "accessToken": auth_entry.access_token,
        "clientToken": auth_entry.client_token
    }, False)


class AuthError(Exception):
    pass


###############################
## Retro compatible JVM args ##
###############################

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


if __name__ == '__main__':
    main()
