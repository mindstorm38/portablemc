from argparse import ArgumentParser, Namespace
from datetime import datetime
from typing import Optional
from os import path
import json
import os


FABRIC_META_URL = "https://meta.fabricmc.net/{}"
FABRIC_VERSIONS_LOADER = "v2/versions/loader/{}"
FABRIC_VERSIONS_LOADER_VERSIONED = "v2/versions/loader/{}/{}"

# https://github.com/NikkyAI/CurseProxy
# https://curse.nikky.moe/
# NIKKYAI_CURSE_URL = "https://curse.nikky.moe/graphql"

# FORGESVC_API_URL = "https://addons-ecs.forgesvc.net/api/{}"
# FORGESVC_GAME_ID = 432       # Minecraft
# FORGESVC_SECTION_ID = 6      # Mods
# FORGESVC_CATEGORY_ID = 4780  # Fabric mods
# FORGESVC_MOD_SEARCH = "v2/addon/search?{}"
# FORGESVC_MOD_META = "v2/addon/{}"
# FORGESVC_MOD_FILE_META = "v2/addon/{}/file/{}"

# FORGESVC_DEPENDENCY_REQUIRED = 3
# FORGESVC_SORT_POPULARITY = 1

class FabricAddon:

    def __init__(self, pmc):

        self.pmc = pmc

        self.pmc.add_message("start.fabric.invalid_format", "To launch fabric, use 'fabric:<mc-version>[:<loader-version>]'.")
        self.pmc.add_message("start.fabric.resolving_loader", "Resolving latest version of fabric loader for {}...")
        self.pmc.add_message("start.fabric.resolving_loader_with_version", "Resolving fabric loader {} for {}...")
        self.pmc.add_message("start.fabric.game_version_not_found", "=> Game version not found.")
        self.pmc.add_message("start.fabric.loader_version_not_found", "=> Loader version not found.")
        self.pmc.add_message("start.fabric.found_cached", "=> Found cached fabric metadata, loading...")
        self.pmc.add_message("start.fabric.generating", "=> The version is not cached, generating...")
        self.pmc.add_message("start.fabric.generated", "=> Generated!")
        self.pmc.add_message("args.start.fabric_prefix", "Change the prefix of the version ID when starting with Fabric.")

        self.pmc.mixin("register_start_arguments", self.register_start_arguments)
        self.pmc.mixin("start_mc_from_cmd", self.start_mc_from_cmd)

    # COMMANDS #

    def register_start_arguments(self, old, parser: ArgumentParser):
        # mixin
        parser.add_argument("--fabric-prefix", default="fabric", help=self.pmc.get_message("args.start.fabric_prefix"), metavar="PREFIX")
        old(parser)

    # START #

    def start_mc_from_cmd(self, old, *, version: str, cmd_args: Namespace, main_dir: Optional[str] = None, **kwargs) -> None:

        if version.startswith("fabric:"):

            version_split = version.split(":")

            if len(version_split) > 3:
                self.pmc.print("start.fabric.invalid_format")
                return

            mc_version = version_split[1]
            loader_version = version_split[2] if len(version_split) == 3 else None

            if not len(mc_version):
                mc_version = "release"

            mc_version, _mc_version_alias = self.pmc.get_version_manifest().filter_latest(mc_version)

            if loader_version is not None and not len(loader_version):
                self.pmc.print("start.fabric.invalid_format")
                return

            try:

                if loader_version is None:
                    self.pmc.print("start.fabric.resolving_loader", mc_version)
                    loader_meta = self.request_version_loader(mc_version, None)
                    loader_version = loader_meta["loader"]["version"]
                else:
                    loader_meta = None
                    self.pmc.print("start.fabric.resolving_loader_with_version", loader_version, mc_version)

                version = "{}-{}-{}".format(cmd_args.fabric_prefix, mc_version, loader_version)
                version_dir = self.pmc.get_version_dir(main_dir, version)
                version_meta_file = path.join(version_dir, "{}.json".format(version))

                if not path.isdir(version_dir) or not path.isfile(version_meta_file):

                    if loader_meta is None:
                        # Loader meta can be None if loader version is set, in this case the version is not
                        # needed to check if the directory already exists.
                        loader_meta = self.request_version_loader(mc_version, loader_version)

                    self.pmc.print("start.fabric.generating")

                    loader_launcher_meta = loader_meta["launcherMeta"]

                    iso_time = datetime.now().isoformat()

                    version_libraries = loader_launcher_meta["libraries"]["common"]
                    version_meta = {
                        "id": version,
                        "inheritsFrom": mc_version,
                        "releaseTime": iso_time,
                        "time": iso_time,
                        "type": self.get_version_type(mc_version),
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

            except GameVersionNotFoundError:
                self.pmc.print("start.fabric.game_version_not_found")
                raise self.pmc.VersionNotFoundError
            except LoaderVersionNotFoundError:
                self.pmc.print("start.fabric.loader_version_not_found")
                raise self.pmc.VersionNotFoundError

        old(cmd_args=cmd_args, version=version, main_dir=main_dir, **kwargs)

    # FABRIC API #

    def request_meta(self, method: str) -> dict:
        return self.pmc.json_simple_request(FABRIC_META_URL.format(method), ignore_error=True)

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

    def get_version_type(self, version: str) -> str:
        version_obj = self.pmc.get_version_manifest().get_version(version)
        return "release" if version_obj is None else version_obj.get("type", "release")



class GameVersionNotFoundError(Exception): ...
class LoaderVersionNotFoundError(Exception): ...
