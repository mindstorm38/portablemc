from argparse import ArgumentParser, Namespace
from typing import Generator, Tuple, Optional
from datetime import datetime
from os import path
import json
import os


FABRIC_META_URL = "https://meta.fabricmc.net/{}"
FABRIC_VERSIONS_GAME = "v2/versions/game"
FABRIC_VERSIONS_LOADER = "v2/versions/loader/{}"
FABRIC_VERSIONS_LOADER_VERSIONED = "v2/versions/loader/{}/{}"


class FabricAddon:

    def __init__(self, pmc):

        self.pmc = pmc

        self.pmc.add_message("start.fabric.invalid_format", "To launch fabric, use 'fabric:<mc-version>[:<loader-version>]'.")
        self.pmc.add_message("start.fabric.resolving_loader", "Resolving latest version of fabric loader for Minecraft {}...")
        self.pmc.add_message("start.fabric.resolving_loader_with_version", "Resolving fabric loader {} for Minecraft {}...")
        self.pmc.add_message("start.fabric.game_version_not_found", "=> Game version not found.")
        self.pmc.add_message("start.fabric.loader_version_not_found", "=> Loader version not found.")
        self.pmc.add_message("start.fabric.found_cached", "=> Found cached metadata, loading...")
        self.pmc.add_message("start.fabric.generating", "=> The version is not cached, generating...")

        self.pmc.mixin("register_start_arguments", self.register_start_arguments)
        self.pmc.mixin("game_start", self.game_start)

    def register_start_arguments(self, old, parser: ArgumentParser):
        parser.add_argument("--fabric-prefix", default="fabric")
        old(parser)

    def game_start(self, old, *, version: str, cmd_args: Namespace, **kwargs) -> None:

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
            version_dir = self.pmc.get_version_dir(version)
            version_meta_file = path.join(version_dir, "{}.json".format(version))

            if not path.isdir(version_dir) or not path.isfile(version_meta_file):

                self.pmc.print("start.fabric.generating")

                if loader_meta is None:
                    loader_meta = self.request_version_loader(mc_version, loader_version)

                loader_launcher_meta = loader_meta["launcherMeta"]

                # NOTE: We are ignoring the JAR because the launcher should download it since
                #  this meta define a "inheritsFrom".
                # version_jar_file = path.join(version_dir, "{}.jar".format(version))

                iso_time = datetime.now().isoformat()

                version_libraries = loader_launcher_meta["libraries"]["common"]
                version_meta = {
                    "id": version,
                    "inheritsFrom": mc_version,
                    "releaseTime": iso_time,
                    "time": iso_time,
                    "type": "release",
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

        else:
            self.pmc.print("start.fabric.found_cached")

        old(cmd_args=cmd_args, version=version, **kwargs)

    def request_meta(self, method: str) -> dict:
        return self.pmc.read_url_json(FABRIC_META_URL.format(method), ignore_error=True)

    def request_versions(self) -> Generator[Tuple[str, bool], None, None]:
        for version in self.request_meta(FABRIC_VERSIONS_GAME):
            yield version["version"], version["stable"]

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


class GameVersionNotFoundError(Exception): ...
class LoaderVersionNotFoundError(Exception): ...
