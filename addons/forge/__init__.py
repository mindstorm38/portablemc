from argparse import ArgumentParser, Namespace
from typing import Dict, List
from os import path
import subprocess
import sys
import os

from portablemc import Version, \
    DownloadList, DownloadEntry, DownloadError, \
    BaseError, \
    http_request, json_simple_request, cli as pmc

from portablemc.cli import CliContext


def load(_pmc):

    # Private mixins

    @pmc.mixin()
    def register_start_arguments(old, parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("--forge-prefix", help=_("args.start.forge_prefix"), default="forge", metavar="PREFIX")
        old(parser)

    @pmc.mixin()
    def cmd_start(old, ns: Namespace, ctx: CliContext):
        try:
            return old(ns, ctx)
        except ForgeInvalidMainDirectory:
            pmc.print_task("FAILED", "start.forge.error.invalid_main_dir", done=True)
            sys.exit(pmc.EXIT_FAILURE)
        except ForgeInstallerFailed as err:
            pmc.print_task("FAILED", f"start.forge.error.installer_{err.return_code}", done=True)
            sys.exit(pmc.EXIT_FAILURE)
        except ForgeVersionNotFound as err:
            pmc.print_task("FAILED", f"start.forge.error.{err.code}", {"version": err.version}, done=True)
            sys.exit(pmc.EXIT_VERSION_NOT_FOUND)

    @pmc.mixin()
    def new_version(old, ctx: CliContext, version_id: str) -> Version:

        if version_id.startswith("forge:"):

            main_dir = path.dirname(ctx.versions_dir)
            if main_dir != path.dirname(ctx.libraries_dir):
                raise ForgeInvalidMainDirectory()

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
            version_meta_file = path.join(version_dir, f"{version_id}.json")

            version = Version(ctx, version_id)

            if not path.isfile(version_meta_file):

                # Extract minecraft version from the full forge version
                mc_version_id = forge_version[:max(0, forge_version.find("-"))]

                os.makedirs(version_dir, exist_ok=True)

                # 1.7 used to have an additional suffix with minecraft version.
                installer_file = path.join(version_dir, "installer.jar")
                installer_urls = [
                    f"https://maven.minecraftforge.net/net/minecraftforge/forge/{forge_version}/forge-{forge_version}-installer.jar",
                    f"https://maven.minecraftforge.net/net/minecraftforge/forge/{forge_version}-{mc_version_id}/forge-{forge_version}-{mc_version_id}-installer.jar"
                ]

                pmc.print_task("", "start.forge.installer.resolving", {"version": forge_version})

                found_installer = False
                for installer_url in installer_urls:
                    try:
                        dl_list = DownloadList()
                        dl_list.append(DownloadEntry(installer_url, installer_file))
                        dl_list.download_files()
                        pmc.print_task("OK", "start.forge.installer.found", {"version": forge_version}, done=True)
                        found_installer = True
                        break
                    except DownloadError:
                        pass

                if not found_installer:
                    raise ForgeVersionNotFound(ForgeVersionNotFound.INSTALLER_NOT_FOUND, forge_version)

                # We ensure that the parent Minecraft version JAR and metadata are
                # downloaded because it's needed by installers.
                if len(mc_version_id):
                    try:
                        pmc.print_task("", "start.forge.vanilla.resolving", {"version": mc_version_id})
                        mc_version = Version(ctx, mc_version_id)
                        mc_version.prepare_meta()
                        mc_version.prepare_jar()
                        mc_version.download()
                        pmc.print_task("OK", "start.forge.vanilla.found", {"version": mc_version_id}, done=True)
                    except DownloadError:
                        raise ForgeVersionNotFound(ForgeVersionNotFound.MINECRAFT_VERSION_NOT_FOUND, mc_version_id)

                pmc.print_task("", "start.forge.wrapper.running")
                wrapper_jar_file = path.join(path.dirname(__file__), "wrapper", "target", "wrapper.jar")
                wrapper_completed = subprocess.run([
                    "java",
                    "-cp", path.pathsep.join([wrapper_jar_file, installer_file]),
                    "portablemc.wrapper.Main",
                    main_dir,
                    version_id
                ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                os.remove(installer_file)
                pmc.print_task("OK", "start.forge.wrapper.done", done=True)

                if wrapper_completed.returncode != 0:
                    raise ForgeInstallerFailed(wrapper_completed.returncode)

            return version

        return old(ctx, version_id)

    # Messages

    pmc.messages.update({
        "args.start.forge_prefix": "Change the prefix of the version ID when starting with Forge.",
        "start.forge.installer.resolving": "Resolving forge {version}...",
        "start.forge.installer.found": "Found installer for forge {version}.",
        "start.forge.vanilla.resolving": "Preparing parent Minecraft version {version}...",
        "start.forge.vanilla.found": "Found parent Minecraft version {version}.",
        "start.forge.wrapper.running": "Running installer (can take few minutes)...",
        "start.forge.wrapper.done": "Forge installation done.",
        "start.forge.error.invalid_main_dir": "The main directory cannot be determined, because version directory "
                                              "and libraries directory must have the same parent directory.",
        "start.forge.error.installer_3": "This forge installer is currently not supported.",
        "start.forge.error.installer_4": "This forge installer is missing something to run (internal).",
        "start.forge.error.installer_5": "This forge installer failed to install forge (internal).",
        f"start.forge.error.{ForgeVersionNotFound.INSTALLER_NOT_FOUND}": "No installer found for forge {version}.",
        f"start.forge.error.{ForgeVersionNotFound.MINECRAFT_VERSION_NOT_FOUND}": "Parent Minecraft version not found "
                                                                                 "{version}.",
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


# Errors

class ForgeInvalidMainDirectory(Exception):
    pass


class ForgeInstallerFailed(Exception):
    def __init__(self, return_code: int):
        self.return_code = return_code


class ForgeVersionNotFound(BaseError):

    INSTALLER_NOT_FOUND = "installer_not_found"
    MINECRAFT_VERSION_NOT_FOUND = "minecraft_version_not_found"

    def __init__(self, code: str, version: str):
        super().__init__(code)
        self.version = version
