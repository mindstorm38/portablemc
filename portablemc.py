from urllib.request import Request as URLRequest
from http.client import HTTPResponse
from urllib import request as urlreq
from urllib.error import HTTPError

from json.decoder import JSONDecodeError
from argparse import ArgumentParser
from os import path as os_path
from zipfile import ZipFile
import subprocess
import platform
import hashlib
import getpass
import shutil
import uuid
import json
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

JAVA_EXEC_DEFAULT = "java"

EXIT_VERSION_NOT_FOUND = 10
EXIT_CLIENT_JAR_NOT_FOUND = 11
EXIT_NATIVES_DIR_ALREADY_EXITS = 12
EXIT_DOWNLOAD_FILE_CORRUPTED = 13
EXIT_AUTHENTICATION_FAILED = 14

DECODE_RESOLUTION = lambda raw: tuple(int(size) for size in raw.split("x"))


def main():

    player_uuid = str(uuid.uuid4())

    parser = ArgumentParser()
    parser.add_argument("-v", "--version", help="Specify Minecraft version ('snapshot' for latest snapshot, 'release' for latest release)", default="release")
    parser.add_argument("--nostart", help="Only download Minecraft required data, but does not launch the game", default=False, action="store_true")
    parser.add_argument("--demo", help="Start game in demo mode", default=False, action="store_true")
    parser.add_argument("--resol", help="Set a custom start resolution (<width>x<height>)", type=DECODE_RESOLUTION, dest="resolution")
    parser.add_argument("--java", help="Set a custom javaw executable path", default=JAVA_EXEC_DEFAULT)
    parser.add_argument("--logout", help="Override all other arguments, does not start the game, and logout from this specific session")
    parser.add_argument("-t", "--temp-login", help="Flag used with -l (--login) to tell launcher not to cache your session if not already cached, deactivated by default", default=False, action="store_true", dest="templogin")
    parser.add_argument("-l", "--login", help="Use a email or username (legacy) to authenticate using mojang servers (you will be asked for password, it override --username and --uuid)")
    parser.add_argument("-u", "--username", help="Set a custom user name to play", default=player_uuid.split("-")[0])
    parser.add_argument("-i", "--uuid", help="Set a custom user UUID to play", default=player_uuid)
    args = parser.parse_args()

    mc_dir = get_minecraft_dir("minecraft")
    auth_db_file = os_path.join(mc_dir, "portablemc_tokens")

    print()
    print("==== COMMON INFO ====")
    print("Minecraft directory: {}".format(mc_dir))
    print("Version manifest url: {}".format(VERSION_MANIFEST_URL))
    print("=====================")
    print()

    # Logging in
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

    # Version meta file caching
    version_dir = os_path.join(mc_dir, "versions", version_name)
    version_meta_file = os_path.join(version_dir, "{}.json".format(version_name))
    version_meta = None

    if os_path.isfile(version_meta_file):
        print("=> Found cached version meta: {}".format(version_meta_file))
        with open(version_meta_file, "rb") as version_meta_fp:
            try:
                version_meta = json.load(version_meta_fp)
            except JSONDecodeError:
                print("=> Failed to decode cached version meta, try updating ...")

    if version_meta is None:
        for manifest_version in version_manifest["versions"]:
            if manifest_version["id"] == version_name:
                version_url = manifest_version["url"]
                print("=> Found version meta in manifest to cache: {}".format(version_url))
                version_meta = read_url_json(version_url)
                if not os_path.isdir(version_dir):
                    os.makedirs(version_dir, 0o777, True)
                with open(version_meta_file, "wt") as version_meta_fp:
                    json.dump(version_meta, version_meta_fp, indent=2)

    if version_meta is None:
        print("=> Failed to find version '{}'".format(args.version))
        exit(EXIT_VERSION_NOT_FOUND)

    # Loading version dependencies
    version_type = version_meta["type"]
    print("Loading {} {}...".format(version_type, version_name))

    # JAR file loading
    print("Loading jar file...")
    version_jar_file = os_path.join(version_dir, "{}.jar".format(version_name))
    if not os_path.isfile(version_jar_file):
        version_downloads = version_meta["downloads"]
        if "client" not in version_downloads:
            print("=> Can't found client download in version meta")
            exit(EXIT_CLIENT_JAR_NOT_FOUND)
        download_file_info_progress(version_downloads["client"], version_jar_file)

    # Assets loading
    print("Loading assets...")
    assets_dir = os_path.join(mc_dir, "assets")
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
    assets_mapped_to_resources = assets_index.get("map_to_resources", False)

    if assets_mapped_to_resources:
        print("=> This version use lagacy assets placed in {}/resources".format(mc_dir))

    for asset_id, asset_obj in assets_index["objects"].items():

        asset_hash = asset_obj["hash"]
        asset_hash_prefix = asset_hash[:2]
        asset_size = asset_obj["size"]
        asset_url = ASSET_BASE_URL.format(asset_hash_prefix, asset_hash)
        asset_hash_dir = os_path.join(assets_objects_dir, asset_hash_prefix)
        asset_file = os_path.join(asset_hash_dir, asset_hash)

        if not os_path.isfile(asset_file) or os_path.getsize(asset_file) != asset_size:
            if not os_path.isdir(asset_hash_dir):
                os.makedirs(asset_hash_dir, 0o777, True)
            assets_current_size = download_file_progress(asset_url, asset_size, asset_hash, asset_file,
                                                         start_size=assets_current_size,
                                                         total_size=assets_total_size,
                                                         name=asset_id)
        else:
            assets_current_size += asset_size

        if assets_mapped_to_resources:
            resources_asset_file = os_path.join(mc_dir, "resources", asset_id)
            resources_asset_dir = os_path.dirname(resources_asset_file)
            if not os_path.isdir(resources_asset_dir):
                os.makedirs(resources_asset_dir, 0o777, True)
            shutil.copyfile(asset_file, resources_asset_file)

    # Logging setup
    print("Loading logger config...")
    logging_arg = None
    if "logging" in version_meta:
        version_logging = version_meta["logging"]
        if "client" in version_logging:
            log_config_dir = os_path.join(assets_dir, "log_configs")
            if not os_path.isdir(log_config_dir):
                os.makedirs(log_config_dir, 0x777, True)
            client_logging = version_logging["client"]
            logging_file_info = client_logging["file"]
            logging_file = os_path.join(log_config_dir, logging_file_info["id"])
            if not os_path.isfile(logging_file) or os_path.getsize(logging_file) != logging_file_info["size"]:
                download_file_info_progress(logging_file_info, logging_file, name=logging_file_info["id"])
            logging_arg = client_logging["argument"].replace("${path}", logging_file)

    # Libraries and natives loading
    print("Loading libraries and natives...")
    libraries_dir = os_path.join(mc_dir, "libraries")

    main_class = version_meta["mainClass"]
    main_class_launchwrapper = (main_class == "net.minecraft.launchwrapper.Launch")
    classpath_libs = [version_jar_file]
    native_libs = []

    for lib_obj in version_meta["libraries"]:

        if "rules" in lib_obj:
            if not interpret_rule(lib_obj["rules"], mc_os):
                continue

        lib_dl = lib_obj["downloads"]
        lib_name = lib_obj["name"]
        lib_dl_info = None
        lib_type = None

        if "natives" in lib_obj and "classifiers" in lib_dl:
            lib_natives = lib_obj["natives"]
            if mc_os in lib_natives:
                lib_native_classifier = lib_natives[mc_os]
                lib_name += ":{}".format(lib_native_classifier)
                lib_dl_info = lib_dl["classifiers"][lib_native_classifier]
                lib_type = "native"
        elif "artifact" in lib_dl:
            lib_dl_info = lib_dl["artifact"]
            lib_type = "classpath"

        if lib_dl_info is None:
            print("=> Can't found library for {}".format(lib_name))
            continue

        lib_dl_path = os_path.join(libraries_dir, lib_dl_info["path"])
        lib_dl_dir = os_path.dirname(lib_dl_path)
        lib_dl_size = lib_dl_info["size"]

        if not os_path.isdir(lib_dl_dir):
            os.makedirs(lib_dl_dir, 0x777, True)

        if not os_path.isfile(lib_dl_path) or os_path.getsize(lib_dl_path) != lib_dl_size:
            download_file_info_progress(lib_dl_info, lib_dl_path, name=lib_name)

        if lib_type == "classpath":
            classpath_libs.append(lib_dl_path)
        elif lib_type == "native":
            native_libs.append(lib_dl_path)

    if args.nostart:
        print("Not starting")
        exit(0)

    # Start game
    print("Starting game ...")

    # Extracting binaries
    bin_dir = os_path.join(mc_dir, "bin", str(uuid.uuid4()))

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
        "game_directory": mc_dir,
        "assets_root": assets_dir,
        "assets_index_name": assets_index_version,
        "auth_uuid": args.uuid,
        "auth_access_token": auth_access_token,
        "user_type": "mojang",
        "version_type": version_type,
        # Game (legacy)
        "auth_session": "token:{}:{}".format(auth_access_token, args.uuid) if len(auth_access_token) else "",
        "game_assets": os_path.join(assets_dir, "virtual", assets_index_version),
        # JVM
        "natives_directory": bin_dir,
        "launcher_name": LAUNCHER_NAME,
        "launcher_version": LAUNCHER_VERSION,
        "classpath": ";".join(classpath_libs)
    }

    if custom_resol is not None:
        start_args_replacements["resolution_width"] = str(custom_resol[0])
        start_args_replacements["resolution_height"] = str(custom_resol[1])

    start_args = [args.java]
    for arg in raw_args:
        for repl_id, repl_val in start_args_replacements.items():
            arg = arg.replace("${{{}}}".format(repl_id), repl_val)
        start_args.append(arg)

    print("=> Running...")
    print("=> Command line: {}".format(" ".join(start_args)))
    subprocess.run(start_args, stdout=subprocess.PIPE, cwd=mc_dir)

    print("=> Game stopped, removing bin directory...")
    shutil.rmtree(bin_dir)


def read_url_json(url: str) -> dict:
    return json.load(urlreq.urlopen(url))


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


def download_file_progress(url: str, size: int, sha1: str, dst: str, *, start_size: int = 0, total_size: int = 0, name: Optional[str] = None) -> int:

    base_message = "Downloading {} ... ".format(url if name is None else name)
    print(base_message, end='')

    success = False

    with urlreq.urlopen(url) as req:
        with open(dst, "wb") as dst_fp:

            dl_sha1 = hashlib.sha1()
            dl_size = 0

            while True:

                chunk = req.read(32768)
                chunk_len = len(chunk)
                if not chunk_len:
                    break

                dl_size += chunk_len
                dl_sha1.update(chunk)
                dst_fp.write(chunk)
                progress = dl_size / size * 100
                print("\r{}{:6.2f}%".format(base_message, progress), end='')

                if total_size != 0:
                    start_size += chunk_len
                    progress = start_size / total_size * 100
                    print("    {:6.2f}% of total".format(progress), end='')

            print("\r{}100.00%".format(base_message), end='')
            if total_size != 0:
                print("    {:6.2f}% of total".format(start_size / total_size * 100), end='')

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


def download_file_info_progress(info: dict, dst: str, *, start_size: int = 0, total_size: int = 0, name: Optional[str] = None) -> int:
    return download_file_progress(info["url"], info["size"], info["sha1"], dst, start_size=start_size, total_size=total_size, name=name)


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


# Used for versions <= 1.12.2
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
