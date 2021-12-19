from argparse import ArgumentParser
from typing import Dict, List

from portablemc import Version, http_request, json_simple_request, cli as pmc
from portablemc.cli import CliContext


# Forge installer types:
# Since 1.12 -> ClientInstall action



def load(_pmc):

    # Private mixins

    @pmc.mixin()
    def register_start_arguments(old, parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("--forge-prefix", help=_("args.start.forge_prefix"), default="forge", metavar="PREFIX")
        old(parser)

    @pmc.mixin()
    def new_version(old, ctx: CliContext, version_id: str) -> Version:

        if version_id.startswith("forge:"):

            game_version = version_id[6:]
            if not len(game_version):
                game_version = "release"

            manifest = pmc.load_version_manifest(ctx)
            game_version, _game_version_alias = manifest.filter_latest(game_version)

            forge_version = None

            promo_versions = request_promotions()
            for suffix in ("", "-recommended", "-latest"):
                forge_version = promo_versions.get(f"{game_version}{suffix}")
                if forge_version is not None:
                    if game_version.endswith("-recommended"):
                        game_version = game_version[:-12]
                    elif game_version.endswith("-latest"):
                        game_version = game_version[:-7]
                    forge_version = f"{game_version}-{forge_version}"
                    break

            if forge_version is None:
                # Test if the user has given the full forge version
                forge_version = game_version

            version_id = f"{ctx.ns.forge_prefix}-{forge_version}"

            print(f"{forge_version=}")
            print(f"{version_id=}")

            return old(ctx, version_id)

        return old(ctx, version_id)

    # Messages

    pmc.messages.update({
        "args.start.forge_prefix": "Change the prefix of the version ID when starting with Forge.",
    })


# Forge API

def request_promotions() -> Dict[str, str]:
    raw = json_simple_request("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")
    return raw["promos"]

def request_versions() -> List[str]:

    status, raw = http_request("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml", "GET", headers={
        "Accept": "application/xml"
    })

    text = raw.decode()

    versions = []
    last_idx = 0

    while True:
        start_idx = text.find("<version>", last_idx)
        if start_idx == -1:
            break
        end_idx = text.find("</version>", start_idx + 9)
        if end_idx == -1:
            break
        versions.append(text[(start_idx + 9):end_idx])
        last_idx = end_idx + 10

    return versions
