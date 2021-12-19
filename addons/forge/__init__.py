from argparse import ArgumentParser
from typing import Dict, List
from os import path
import os

from portablemc import Version, DownloadList, DownloadEntry, http_request, json_simple_request, cli as pmc
from portablemc.cli import CliContext


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
            promo_versions = request_promo_versions()
            for suffix in ("", "-recommended", "-latest"):
                tmp_forge_version = promo_versions.get(f"{game_version}{suffix}")
                if tmp_forge_version is not None:
                    if game_version.endswith("-recommended"):
                        game_version = game_version[:-12]
                    elif game_version.endswith("-latest"):
                        game_version = game_version[:-7]
                    forge_version = f"{game_version}-{tmp_forge_version}"
                    break

            if forge_version is None:
                # Test if the user has given the full forge version
                forge_version = game_version

            version_id = f"{ctx.ns.forge_prefix}-{forge_version}"
            version_dir = ctx.get_version_dir(version_id)
            os.makedirs(version_dir, exist_ok=True)

            installer_url = f"https://maven.minecraftforge.net/net/minecraftforge/forge/{forge_version}/forge-{forge_version}-installer.jar"
            installer_file = path.join(version_dir, "installer.jar")

            dl_list = DownloadList()
            dl_list.append(DownloadEntry(installer_url, installer_file, name=f"{forge_version}-installer"))
            if pmc.pretty_download(dl_list):
                pass

            return old(ctx, version_id)

        return old(ctx, version_id)

    # Messages

    pmc.messages.update({
        "args.start.forge_prefix": "Change the prefix of the version ID when starting with Forge.",
    })


# Forge API

def request_promo_versions() -> Dict[str, str]:
    raw = json_simple_request("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")
    return raw["promos"]


def request_maven_versions() -> List[str]:

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
