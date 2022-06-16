from argparse import ArgumentParser, Namespace
from typing import Optional
import sys

from portablemc import Version, BaseError, Context, JsonRequestError, json_simple_request


FABRIC_META_URL = "https://meta.fabricmc.net/{}"
FABRIC_VERSIONS_LOADER = "v2/versions/loader/{}"
FABRIC_VERSIONS_LOADER_PROFILE = "v2/versions/loader/{}/{}/profile/json"


def load():

    from portablemc.cli import CliContext
    from portablemc import cli as pmc

    # Private mixins

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

            if loader_version is not None and not len(loader_version):
                raise FabricInvalidFormatError()

            manifest = pmc.new_version_manifest(ctx)
            game_version, _game_version_alias = manifest.filter_latest(game_version)

            if loader_version is None:
                pmc.print_task("OK", "start.fabric.resolving_loader", {
                    "game_version": game_version
                }, done=True)
            else:
                pmc.print_task("OK", "start.fabric.resolving_loader_specific", {
                    "loader_version": loader_version,
                    "game_version": game_version
                }, done=True)

            ver = FabricVersion(ctx, game_version, loader_version, prefix=ctx.ns.fabric_prefix)
            ver.manifest = manifest
            return ver

        return old(ctx, version_id)

    # Messages

    pmc.messages.update({
        "args.start.fabric_prefix": "Change the prefix of the version ID when starting with Fabric.",
        "start.fabric.resolving_loader": "Resolving fabric loader for {game_version}...",
        "start.fabric.resolving_loader_specific": "Resolving fabric loader {loader_version} for {game_version}...",
        "start.fabric.error.invalid_format": "To launch fabric, use 'fabric:[<mc-version>[:<loader-version>]]'.",
        f"start.fabric.error.{FabricVersionNotFound.GAME_VERSION_NOT_FOUND}": "Game version {version} not found.",
        f"start.fabric.error.{FabricVersionNotFound.LOADER_VERSION_NOT_FOUND}": "Loader version {version} not found."
    })


class FabricVersion(Version):

    def __init__(self, context: Context, game_version: str, loader_version: Optional[str], *, prefix: str = "fabric"):

        """
        Construct a new fabric version, such version are specified by a game version and an optional loader-version,
        if loader-version is not specified, the latest version is used and fetched when first calling `prepare_meta`.
        """

        # If the loader version is unknown, we temporarily use a version ID without
        id_ = f"{prefix}-{game_version}" if loader_version is None else f"{prefix}-{game_version}-{loader_version}"

        super().__init__(context, id_)

        self.game_version = game_version
        self.loader_version = loader_version

    # The function 'prepare_meta' might throw 'FabricVersionNotFound' either
    # '_prepare_id' or from the inner calls to '_fetch_version_meta'.

    def _prepare_id(self):
        # If the loader version is unknown, the version's id is not fully defined,
        # so we add the loader version to the id.
        if self.loader_version is None:
            self.loader_version = request_loader_version(self.game_version)
            self.id += f"-{self.loader_version}"

    def _validate_version_meta(self, version_id: str, version_dir: str, version_meta_file: str, version_meta: dict) -> bool:
        if version_id == self.id:
            # If the version is installed, it is always valid, because we don't have any metadata to check its validity.
            return True
        else:
            return super()._validate_version_meta(version_id, version_dir, version_meta_file, version_meta)

    def _fetch_version_meta(self, version_id: str, version_dir: str, version_meta_file: str) -> dict:
        if version_id != self.id:
            return super()._fetch_version_meta(version_id, version_dir, version_meta_file)
        return request_version_loader_profile(self.game_version, self.loader_version)


# FabricMC API

def request_meta(method: str) -> dict:
    return json_simple_request(FABRIC_META_URL.format(method))


def request_loader_version(game_version: str) -> str:
    try:
        ret = request_meta(FABRIC_VERSIONS_LOADER.format(game_version))[0].get("loader", {}).get("version")
    except (JsonRequestError, IndexError):
        ret = None
    if ret is None:
        raise FabricVersionNotFound(FabricVersionNotFound.GAME_VERSION_NOT_FOUND, game_version)
    return ret


def request_version_loader_profile(game_version: str, loader_version: str) -> dict:
    try:
        return request_meta(FABRIC_VERSIONS_LOADER_PROFILE.format(game_version, loader_version))
    except JsonRequestError as err:
        if b"no mappings" in err.data:
            raise FabricVersionNotFound(FabricVersionNotFound.GAME_VERSION_NOT_FOUND, game_version)
        raise FabricVersionNotFound(FabricVersionNotFound.LOADER_VERSION_NOT_FOUND, loader_version)


# Errors

class FabricInvalidFormatError(Exception):
    pass


class FabricVersionNotFound(BaseError):

    GAME_VERSION_NOT_FOUND = "game_version_not_found"
    LOADER_VERSION_NOT_FOUND = "loader_version_not_found"

    def __init__(self, code: str, version: str):
        super().__init__(code)
        self.version = version
