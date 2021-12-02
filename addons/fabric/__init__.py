from argparse import ArgumentParser, Namespace
from datetime import datetime
from typing import Optional
from os import path
import json
import sys
import os


FABRIC_META_URL = "https://meta.fabricmc.net/{}"
FABRIC_VERSIONS_LOADER = "v2/versions/loader/{}"
FABRIC_VERSIONS_LOADER_VERSIONED = "v2/versions/loader/{}/{}"


def load(pmc):

    Version = pmc.Version
    VersionManifest = pmc.VersionManifest
    BaseError = pmc.BaseError
    CliContext = pmc.CliContext

    @pmc.mixin()
    def register_start_arguments(old, parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("--fabric-prefix", help=_("args.start.fabric_prefix"), default="fabric", metavar="PREFIX")
        old(parser)

    @pmc.mixin()
    def cmd_start(old, ns: Namespace, ctx: CliContext):
        try:
            return old(ns, ctx)
        except FabricInvalidFormatError:
            pmc.print_task("FAILED", "start.fabric.error.invalid_format", done=True)
            sys.exit(pmc.EXIT_WRONG_USAGE)
        except FabricVersionNotFound as err:
            pmc.print_task("FAILED", f"start.fabric.error.{err.code}", {"version": err.version}, done=True)
            sys.exit(pmc.EXIT_VERSION_NOT_FOUND)

    @pmc.mixin()
    def new_version(old, ctx: CliContext, version_id: str) -> Version:

        if version_id.startswith("fabric:"):

            version_split = version_id.split(":")
            if len(version_split) > 3:
                raise FabricInvalidFormatError()

            game_version = version_split[1]
            loader_version = version_split[2] if len(version_split) == 3 else None

            if not len(game_version):
                game_version = "release"

            manifest = pmc.load_version_manifest(ctx)
            game_version, _game_version_alias = manifest.filter_latest(game_version)

            if loader_version is not None and not len(loader_version):
                raise FabricInvalidFormatError()

            if loader_version is None:
                pmc.print_task("", "start.fabric.resolving_loader", {"game_version": game_version})
                loader_meta = request_version_loader(game_version, None)
                loader_version = loader_meta["loader"]["version"]
            else:
                pmc.print_task("", "start.fabric.resolving_loader_with_version", {
                    "loader_version": loader_version,
                    "game_version": game_version
                })
                # Loader meta is none because if the 'loader_version' is set, we do not need it to check if dir exists.
                loader_meta = None

            version_id = f"{ctx.ns.fabric_prefix}-{game_version}-{loader_version}"
            version_dir = ctx.get_version_dir(version_id)
            version_meta_file = path.join(version_dir, f"{version_id}.json")

            if not path.isdir(version_dir) or not path.isfile(version_meta_file):

                pmc.print_task("", "start.fabric.generating")

                if loader_meta is None:
                    # If the directory does not exists and the loader_version was provided, request meta.
                    loader_meta = request_version_loader(game_version, loader_version)

                loader_launcher_meta = loader_meta["launcherMeta"]

                iso_time = datetime.now().isoformat()

                version_libraries = loader_launcher_meta["libraries"]["common"]
                version_meta = {
                    "id": version_id,
                    "inheritsFrom": game_version,
                    "releaseTime": iso_time,
                    "time": iso_time,
                    "type": get_version_type(manifest, game_version),
                    "mainClass": loader_launcher_meta["mainClass"]["client"],
                    "arguments": {
                        "game": [],
                        "jvm": [
                            # TODO: Might add "-DFabricMcEmu= net.minecraft.client.main.Main " in the future.
                        ]
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

            pmc.print_task("OK", "start.fabric.resolved_loader", {
                "loader_version": loader_version,
                "game_version": game_version
            }, done=True)

            version = Version(ctx, version_id)
            version.manifest = manifest
            return version

        return old(ctx, version_id)

    # FabricMC API

    def request_meta(method: str) -> dict:
        return pmc.json_simple_request(FABRIC_META_URL.format(method), ignore_error=True)

    def request_version_loader(game_version: str, loader_version: Optional[str]) -> Optional[dict]:
        if loader_version is None:
            ret = request_meta(FABRIC_VERSIONS_LOADER.format(game_version))
            if not len(ret):
                raise FabricVersionNotFound(FabricVersionNotFound.GAME_VERSION_NOT_FOUND, game_version)
            return ret[0]
        else:
            ret = request_meta(FABRIC_VERSIONS_LOADER_VERSIONED.format(game_version, loader_version))
            if isinstance(ret, str):
                if ret.startswith("no mappings"):
                    raise FabricVersionNotFound(FabricVersionNotFound.GAME_VERSION_NOT_FOUND, game_version)
                raise FabricVersionNotFound(FabricVersionNotFound.LOADER_VERSION_NOT_FOUND, loader_version)
            return ret

    def get_version_type(manifest: VersionManifest, version: str) -> str:
        version_obj = manifest.get_version(version)
        return "release" if version_obj is None else version_obj.get("type", "release")

    # Errors

    class FabricInvalidFormatError(Exception):
        pass

    class FabricVersionNotFound(BaseError):

        GAME_VERSION_NOT_FOUND = "game_version_not_found"
        LOADER_VERSION_NOT_FOUND = "loader_version_not_found"

        def __init__(self, code: str, version: str):
            super().__init__(code)
            self.version = version

    # Messages

    pmc.messages.update({
        "args.start.fabric_prefix": "Change the prefix of the version ID when starting with Fabric.",
        "start.fabric.resolving_loader": "Resolving fabric loader for {game_version}...",
        "start.fabric.resolving_loader_with_version": "Resolving fabric loader {loader_version} for {game_version}...",
        "start.fabric.resolved_loader": "Resolved fabric loader {loader_version} for {game_version}.",
        "start.fabric.generating": "Generating fabric version meta...",
        "start.fabric.error.invalid_format": "To launch fabric, use 'fabric:[<mc-version>[:<loader-version>]]'.",
        f"start.fabric.error.{FabricVersionNotFound.GAME_VERSION_NOT_FOUND}": "Game version {version} not found.",
        f"start.fabric.error.{FabricVersionNotFound.LOADER_VERSION_NOT_FOUND}": "Loader version {version} not found."
    })
