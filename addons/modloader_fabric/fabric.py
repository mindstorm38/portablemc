from urllib import parse as url_parse, request as url_request
from argparse import ArgumentParser, Namespace
from urllib.request import Request
from urllib.error import HTTPError
from typing import Optional, Union
from datetime import datetime
from os import path
import json
import os


# This addon relies on multiples APIs, this is not really safe on the long-term.


FABRIC_META_URL = "https://meta.fabricmc.net/{}"
FABRIC_VERSIONS_LOADER = "v2/versions/loader/{}"
FABRIC_VERSIONS_LOADER_VERSIONED = "v2/versions/loader/{}/{}"

# This constant is currently used to disable unstable or W.I.P. features.
# E.g. Mods installation
DEV = False

# https://github.com/NikkyAI/CurseProxy
# https://curse.nikky.moe/
NIKKYAI_CURSE_URL = "https://curse.nikky.moe/graphql"

FORGESVC_API_URL = "https://addons-ecs.forgesvc.net/api/{}"
FORGESVC_GAME_ID = 432       # Minecraft
FORGESVC_SECTION_ID = 6      # Mods
FORGESVC_CATEGORY_ID = 4780  # Fabric mods
FORGESVC_MOD_SEARCH = "v2/addon/search?{}"
FORGESVC_MOD_META = "v2/addon/{}"
FORGESVC_MOD_FILE_META = "v2/addon/{}/file/{}"

FORGESVC_DEPENDENCY_REQUIRED = 3
FORGESVC_SORT_POPULARITY = 1

class FabricAddon:

    def __init__(self, pmc):

        self.pmc = pmc

        self.pmc.add_message("start.fabric.invalid_format", "To launch fabric, use 'fabric:<mc-version>[:<loader-version>]'.")
        self.pmc.add_message("start.fabric.resolving_loader", "Resolving latest version of fabric loader for Minecraft {}...")
        self.pmc.add_message("start.fabric.resolving_loader_with_version", "Resolving fabric loader {} for Minecraft {}...")
        self.pmc.add_message("start.fabric.game_version_not_found", "=> Game version not found.")
        self.pmc.add_message("start.fabric.loader_version_not_found", "=> Loader version not found.")
        self.pmc.add_message("start.fabric.found_cached", "=> Found cached fabric metadata, loading...")
        self.pmc.add_message("start.fabric.generating", "=> The version is not cached, generating...")
        self.pmc.add_message("start.fabric.generated", "=> Generated!")
        self.pmc.add_message("args.start.fabric_prefix", "Change the prefix of the version ID when starting with Fabric.")

        if DEV:

            self.pmc.add_message("args.fabric", "Subcommands for managing your FabricMC mods.")
            self.pmc.add_message("args.fabric.search", "Search for Fabric mods.")
            self.pmc.add_message("args.fabric.search.version", "Search for mods with a specific version of Minecraft.")
            self.pmc.add_message("args.fabric.search.offset", "The offset of the first mod.")
            self.pmc.add_message("args.fabric.search.count", "The number of displayed mods.")
            self.pmc.add_message("args.fabric.install", "Install mods.")

            self.pmc.add_message("fabric.search.pending", "Searching for Fabric mods...")
            self.pmc.add_message("fabric.search.result", "=> {:30s} [{}] [{}] ({})")

            self.pmc.add_message("fabric.install.resolving_mod", "Resolving mod '{}' for Minecraft '{}'...")
            self.pmc.add_message("fabric.install.invalid_mod", "Invalid mod '{}', please specify the version like this: '<mod_name>:<mc_version>'.")
            self.pmc.add_message("fabric.install.mod_not_found", "=> The mod was not found.")
            self.pmc.add_message("fabric.install.version_not_found", "=> The mod was not found for Minecraft version '{}'.")
            self.pmc.add_message("fabric.install.resolving_dependency", "=> Resolving dependencies...")
            self.pmc.add_message("fabric.install.no_required_dependency", "=> There is no required dependency, ignoring.")
            self.pmc.add_message("fabric.install.dependency_not_found", "=> Abording installation because of unknown dependencies.")

            self.pmc.mixin("register_subcommands", self.register_subcommands)
            self.pmc.mixin("start_subcommand", self.start_subcommand)

        self.pmc.mixin("register_start_arguments", self.register_start_arguments)
        self.pmc.mixin("start_mc_from_cmd", self.start_mc_from_cmd)

    # COMMANDS #

    if DEV:

        def register_subcommands(self, old, subcommands):
            # mixin
            old(subcommands)
            self.register_fabric_arguments(subcommands.add_parser("fabric", help=self.pmc.get_message("args.fabric")))

        def register_fabric_arguments(self, parser: ArgumentParser):
            self.register_fabric_subcommands(parser.add_subparsers(title="fabric subcommands", dest="fabric_subcommand", required=True))

        def register_fabric_subcommands(self, subcommands):
            self.register_search_arguments(subcommands.add_parser("search", help=self.pmc.get_message("args.fabric.search")))
            self.register_install_arguments(subcommands.add_parser("install", help=self.pmc.get_message("args.fabric.install")))

        def register_search_arguments(self, parser: ArgumentParser):
            parser.add_argument("-v", "--version", help=self.pmc.get_message("args.fabric.search.version"))
            parser.add_argument("-o", "--offset", help=self.pmc.get_message("args.fabric.search.offset"))
            parser.add_argument("-c", "--count", help=self.pmc.get_message("args.fabric.search.count"), default=25)
            parser.add_argument("search", nargs="?")

        def register_install_arguments(self, parser: ArgumentParser):
            parser.add_argument("mod", nargs="+")

    def register_start_arguments(self, old, parser: ArgumentParser):
        # mixin
        parser.add_argument("--fabric-prefix", default="fabric", help=self.pmc.get_message("args.start.fabric_prefix"), metavar="PREFIX")
        old(parser)

    if DEV:

        def start_subcommand(self, old, subcommand: str, args: Namespace) -> int:
            # mixin
            if subcommand == "fabric":
                return self.cmd_fabric(args)
            else:
                return old(subcommand, args)

        def cmd_fabric(self, args: Namespace) -> int:
            if args.fabric_subcommand == "search":
                return self.cmd_search(args)
            elif args.fabric_subcommand == "install":
                return self.cmd_install(args)
            return 0

    if DEV:

        # SEARCH #

        def cmd_search(self, args: Namespace) -> int:

            self.pmc.print("fabric.search.pending")

            for result in self.request_forge_search(
                    game_version=args.version,
                    offset=args.offset,
                    count=args.count,
                    search=args.search):

                author = result["authors"][0]["name"] if len(result["authors"]) else "unknown"

                self.pmc.print("fabric.search.result",
                               result["slug"],
                               self.format_downloads(result["downloadCount"]),
                               result["id"],
                               author)

            return 0

        # INSTALL #

        def cmd_install(self, args: Namespace) -> int:

            downloads = []
            for mod in args.mod:
                mod_split = mod.split(":")
                if len(mod_split) != 2 or not len(mod_split[0]) or not len(mod_split[1]):
                    self.pmc.print("fabric.install.invalid_mod", mod)
                    continue
                else:
                    self.cmd_install_inner(mod_split[0], mod_split[1], downloads)

            mods_dir = path.join(self.pmc.get_arg_main_dir(), "mods")

            for download in downloads:

                version_mod_dir = path.join(mods_dir, download["version"])
                os.makedirs(version_mod_dir, 0o777, True)
                mod_file = path.join(version_mod_dir, download["name"])

                self.pmc.download_file(self.pmc.DownloadEntry(download["url"], mod_file, size=download["size"], name=download["name"]))

            return 0

        def cmd_install_inner(self, id_or_slug: Union[str, int], mc_version: str, downloads: list) -> bool:

            self.pmc.print("fabric.install.resolving_mod", id_or_slug, mc_version)

            # mod_data = self.request_forge_addon(id_or_slug)
            mod_data = self.request_nikkyai_addon(id_or_slug)
            if mod_data is None:
                self.pmc.print("fabric.install.mod_not_found")
                return False

            for game_version_file in mod_data["gameVersionLatestFiles"]:
                if game_version_file["gameVersion"] == mc_version:
                    file_id = game_version_file["projectFileId"]
                    break
            else:
                self.pmc.print("fabric.install.version_not_found", mc_version)
                return False

            file_data = self.request_forge_api(FORGESVC_MOD_FILE_META.format(mod_data["id"], file_id))

            if len(file_data["dependencies"]):
                self.pmc.print("fabric.install.resolving_dependency")
                required_deps = False
                for dependency in file_data["dependencies"]:
                    if dependency["type"] == FORGESVC_DEPENDENCY_REQUIRED:
                        required_deps = True
                        if not self.cmd_install_inner(dependency["addonId"], mc_version, downloads):
                            self.pmc.print("fabric.install.dependency_not_found")
                            return False
                if not required_deps:
                    self.pmc.print("fabric.install.no_required_dependency")

            downloads.append({
                "name": file_data["fileName"],
                "url": file_data["downloadUrl"],
                "size": file_data["fileLength"],
                "version": mc_version
            })

            return True

    # START #

    def start_mc_from_cmd(self, old, *, version: str, cmd_args: Namespace, main_dir: Optional[str] = None, **kwargs) -> None:

        if version.startswith("fabric:"):

            version_split = version.split(":")

            if len(version_split) > 3:
                self.pmc.print("start.fabric.invalid_format")
                return

            mc_version = version_split[1]
            loader_version = version_split[2] if len(version_split) == 3 else None

            if not len(mc_version) or (loader_version is not None and not len(loader_version)):
                self.pmc.print("start.fabric.invalid_format")
                return

            try:

                loader_meta = None
                if loader_version is None:
                    self.pmc.print("start.fabric.resolving_loader", mc_version)
                    loader_meta = self.request_version_loader(mc_version, loader_version)
                    loader_version = loader_meta["loader"]["version"]
                else:
                    self.pmc.print("start.fabric.resolving_loader_with_version", loader_version, mc_version)

            except GameVersionNotFoundError:
                self.pmc.print("start.fabric.game_version_not_found")
                return
            except LoaderVersionNotFoundError:
                self.pmc.print("start.fabric.loader_version_not_found")
                return

            version = "{}-{}-{}".format(cmd_args.fabric_prefix, mc_version, loader_version)
            version_dir = self.pmc.get_version_dir(main_dir, version)
            version_meta_file = path.join(version_dir, "{}.json".format(version))

            if not path.isdir(version_dir) or not path.isfile(version_meta_file):

                self.pmc.print("start.fabric.generating")

                if loader_meta is None:
                    loader_meta = self.request_version_loader(mc_version, loader_version)

                loader_launcher_meta = loader_meta["launcherMeta"]

                # Resolving parent metadata to get the type of version
                parent_version_meta, parent_version_dir = self.pmc.resolve_version_meta_recursive(main_dir, mc_version)

                iso_time = datetime.now().isoformat()

                version_libraries = loader_launcher_meta["libraries"]["common"]
                version_meta = {
                    "id": version,
                    "inheritsFrom": mc_version,
                    "releaseTime": iso_time,
                    "time": iso_time,
                    "type": parent_version_meta.get("type", "release"),
                    "mainClass": loader_launcher_meta["mainClass"]["client"],
                    "arguments": {
                        "game": []
                    },
                    "libraries": version_libraries
                }

                version_libraries.append({
                    "name": loader_meta["loader"]["maven"],
                    "url": "https://maven.fabricmc.net/"
                })

                version_libraries.append({
                    "name": loader_meta["intermediary"]["maven"],
                    "url": "https://maven.fabricmc.net/"
                })

                os.makedirs(version_dir, exist_ok=True)
                with open(version_meta_file, "wt") as fp:
                    json.dump(version_meta, fp, indent=2)

                self.pmc.print("start.fabric.generated")

            else:
                self.pmc.print("start.fabric.found_cached")

        old(cmd_args=cmd_args, version=version, main_dir=main_dir, **kwargs)

    # FABRIC API #

    def request_meta(self, method: str) -> dict:
        return self.pmc.read_url_json(FABRIC_META_URL.format(method), ignore_error=True)

    def request_version_loader(self, mc_version: str, loader_version: Optional[str]) -> Optional[dict]:
        if loader_version is None:
            ret = self.request_meta(FABRIC_VERSIONS_LOADER.format(mc_version))
            if not len(ret):
                raise GameVersionNotFoundError
            return ret[0]
        else:
            ret = self.request_meta(FABRIC_VERSIONS_LOADER_VERSIONED.format(mc_version, loader_version))
            if isinstance(ret, str):
                if ret.startswith("no mappings"):
                    raise GameVersionNotFoundError
                raise LoaderVersionNotFoundError
            return ret

    if DEV:

        # NIKKYAI CURSE API #

        def request_nikkyai(self, graph: str):

            req = Request(NIKKYAI_CURSE_URL, json.dumps({
                "query": graph
            }).encode("utf-8"), headers={
                "Content-Type": "application/json",
                "Accept": "application/json"
            })

            try:
                with url_request.urlopen(req) as res:
                    return json.load(res)
            except HTTPError as e:
                print(json.load(e))

        def request_nikkyai_addon(self, id_or_slug: Union[str, int]) -> Optional[dict]:

            if isinstance(id_or_slug, str):
                id_or_slug_param = 'slug: "{}"'.format(id_or_slug)
            elif isinstance(id_or_slug, str):
                id_or_slug_param = 'id: {}'.format(id_or_slug)
            else:
                raise ValueError

            res = self.request_nikkyai('{{\n'
                                 '  addons(gameId: 432, section: "Mods", category: "Fabric", {}) {{\n'
                                 '    id\n'
                                 '    name\n'
                                 '    slug\n'
                                 '    gameVersionLatestFiles {{\n'
                                 '      gameVersion\n'
                                 '      projectFileId\n'
                                 '    }}\n'
                                 '  }}\n'
                                 '}}\n'.format(id_or_slug_param))

            return res["data"]["addons"][0] if len(res["data"]["addons"]) else None

        # FORGE API #

        def request_forge_api(self, method: str) -> Union[dict, list]:
            # print(FORGESVC_API_URL.format(method))
            return self.pmc.read_url_json(FORGESVC_API_URL.format(method), ignore_error=True)

        def request_forge_search(self, *,
                                 game_version: Optional[str] = None,
                                 offset: Optional[int] = None,
                                 count: Optional[int] = None,
                                 search: Optional[str] = None) -> list:

            options = {
                "gameId": FORGESVC_GAME_ID,
                "sectionId": FORGESVC_SECTION_ID,
                "categoryId": FORGESVC_CATEGORY_ID,
                "sort": FORGESVC_SORT_POPULARITY
            }

            if game_version is not None: options["gameVersion"] = game_version
            if offset is not None: options["index"] = offset
            if count is not None: options["pageSize"] = count
            if search is not None: options["searchFilter"] = search

            return self.request_forge_api(FORGESVC_MOD_SEARCH.format(url_parse.urlencode(options)))

        # FIXME: Deprecated
        def request_forge_addon(self, id_or_slug: Union[str, int]) -> Optional[dict]:
            if isinstance(id_or_slug, str):
                for result in self.request_forge_search(search=id_or_slug, count=9999):
                    if result["slug"] == id_or_slug:
                        return result
                return None
            elif isinstance(id_or_slug, int):
                return self.request_forge_api(FORGESVC_MOD_META.format(id_or_slug))

    # UTILS #

    @staticmethod
    def format_downloads(n: int) -> str:
        n = int(n)
        if n >= 1000000:
            return "{:3}M".format(n // 1000000)
        elif n >= 1000:
            return "{:3}K".format(n // 1000)
        else:
            return " {:3}".format(n)

    """@staticmethod
    def elide_groups(t: str, start: str, end: str) -> str:
        i = t.find(start)
        if i != -1:
            j = t.find(end, i)
            if j != -1 and (j - i) > 2:
                before = t[:(i + (-1 if i > 0 and t[i - 1] == " " else 0))]
                after = t[(j + (2 if j < len(t) - 1 and t[j + 1] == " " else 1)):]
                return "{}{}".format(before, after)
        return t"""


class GameVersionNotFoundError(Exception): ...
class LoaderVersionNotFoundError(Exception): ...
