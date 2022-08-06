from argparse import ArgumentParser, Namespace
from typing import Dict, List
from os import path
import subprocess
import sys
import os

from portablemc import Version, \
    DownloadList, DownloadEntry, DownloadReport, \
    BaseError, \
    http_request, json_simple_request, Context


def load():

    from portablemc.cli import CliContext
    from portablemc import cli as pmc

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
            print("===================")
            print(err.output)
            print("===================")
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

            manifest = pmc.new_version_manifest(ctx)
            game_version, game_version_alias = manifest.filter_latest(game_version)

            forge_version = None

            # If the version is an alias, we know that the version needs to be resolved from the forge
            # promotion metadata. It's also the case if the version ends with '-recommended' or '-latest',
            # or if the version doesn't contains a "-".
            if game_version_alias or game_version.endswith(("-recommended", "-latest")) or "-" not in game_version:
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
                # If the game version came from an alias, we know for sure that no forge
                # version is currently supporting the latest release/snapshot.
                if game_version_alias:
                    raise ForgeVersionNotFound(ForgeVersionNotFound.MINECRAFT_VERSION_NOT_SUPPORTED, game_version)
                # Test if the user has given the full forge version
                forge_version = game_version

            installer = ForgeVersionInstaller(ctx, forge_version, prefix=ctx.ns.forge_prefix)

            if installer.needed():

                pmc.print_task("", "start.forge.resolving", {"version": forge_version})
                installer.prepare()
                pmc.print_task("OK", "start.forge.resolved", {"version": forge_version}, done=True)

                installer.check_download(pmc.pretty_download(installer.dl))

                pmc.print_task("", "start.forge.wrapper.running")
                installer.install()
                pmc.print_task("OK", "start.forge.wrapper.done", done=True)

            pmc.print_task("INFO", "start.forge.consider_support", done=True)
            return installer.version

        return old(ctx, version_id)

    # Messages

    pmc.messages.update({
        "args.start.forge_prefix": "Change the prefix of the version ID when starting with Forge.",
        "start.forge.resolving": "Resolving forge {version}...",
        "start.forge.resolved": "Resolved forge {version}, downloading installer and parent version.",
        "start.forge.wrapper.running": "Running installer (can take few minutes)...",
        "start.forge.wrapper.done": "Forge installation done.",
        "start.forge.consider_support": "Consider supporting the forge project through https://www.patreon.com/LexManos/.",
        "start.forge.error.invalid_main_dir": "The main directory cannot be determined, because version directory "
                                              "and libraries directory must have the same parent directory.",
        "start.forge.error.installer_1": "Invalid command to start forge installer wrapper (should not happen, contact "
                                         "maintainers, this can also happen if the installer fails).",
        "start.forge.error.installer_2": "Invalid main directory to start forge installer wrapper (should not happen, "
                                         "contact maintainers).",
        "start.forge.error.installer_3": "This forge installer is currently not supported.",
        "start.forge.error.installer_4": "This forge installer is missing something to run (internal).",
        "start.forge.error.installer_5": "This forge installer failed to install forge (internal).",
        f"start.forge.error.{ForgeVersionNotFound.INSTALLER_NOT_FOUND}": "No installer found for forge {version}.",
        f"start.forge.error.{ForgeVersionNotFound.MINECRAFT_VERSION_NOT_FOUND}": "Parent Minecraft version not found "
                                                                                 "{version}.",
        f"start.forge.error.{ForgeVersionNotFound.MINECRAFT_VERSION_NOT_SUPPORTED}": "Minecraft version {version} is not "
                                                                                     "currently supported by forge."
    })


class ForgeVersion(Version):

    def __init__(self, context: Context, forge_version: str, *, prefix: str = "forge"):
        super().__init__(context, f"{prefix}-{forge_version}")
        self.forge_version = forge_version

    def _validate_version_meta(self, version_id: str, version_dir: str, version_meta_file: str, version_meta: dict) -> bool:
        if version_id == self.id:
            return True
        else:
            return super()._validate_version_meta(version_id, version_dir, version_meta_file, version_meta)

    def _fetch_version_meta(self, version_id: str, version_dir: str, version_meta_file: str) -> dict:
        if version_id == self.id:
            # If the underlying class call this for THIS version, it means that the version hasn't been installed yet.
            # This should not happen if the installer has been run before.
            raise ForgeVersionNotFound(ForgeVersionNotFound.NOT_INSTALLED, self.forge_version)
        else:
            return super()._fetch_version_meta(version_id, version_dir, version_meta_file)


class ForgeVersionInstaller:

    def __init__(self, context: Context, forge_version: str, *, prefix: str = "forge"):

        # The real version object being installed by this installer.
        self.version = ForgeVersion(context, forge_version, prefix=prefix)

        self.version_dir = self.version.context.get_version_dir(self.version.id)
        self.installer_file = path.join(self.version_dir, "installer.jar")
        self.dl = DownloadList()
        self.main_dir = None
        self.jvm_exec = None

        # Extract minecraft version from the full forge version
        self.parent_version_id = forge_version[:max(0, forge_version.find("-")) or len(forge_version)]

        # List of possible artifacts names-version, some versions (e.g. 1.7) have the minecraft
        # version in suffix of the version in addition to the suffix.
        self.possible_artifact_versions = [forge_version, f"{forge_version}-{self.parent_version_id}"]

    def needed(self) -> bool:

        """ Return True if this forge version needs to be installed. """

        if not path.isfile(path.join(self.version_dir, f"{self.version.id}.json")):
            # If the version's metadata is not found.
            return True

        local_artifact_path = path.join(self.version.context.libraries_dir, "net", "minecraftforge", "forge")
        for possible_version in self.possible_artifact_versions:
            for possible_classifier in (possible_version, f"{possible_version}-client"):
                artifact_jar = path.join(local_artifact_path, possible_version, f"forge-{possible_classifier}.jar")
                if path.isfile(artifact_jar):
                    # If we found at least one valid forge artifact, the version is valid.
                    return False

        return True

    def prepare(self):

        # The main dir specific to forge, it needs to be
        self.main_dir = path.dirname(self.version.context.versions_dir)
        if not path.samefile(self.main_dir, path.dirname(self.version.context.libraries_dir)):
            raise ForgeInvalidMainDirectory()

        last_dl_entry = None
        for possible_version in self.possible_artifact_versions:
            installer_url = f"https://maven.minecraftforge.net/net/minecraftforge/forge/{possible_version}/forge-{possible_version}-installer.jar"
            possible_entry = DownloadEntry(installer_url, self.installer_file, name=f"installer:{possible_version}")
            if last_dl_entry is None:
                last_dl_entry = possible_entry
                self.dl.append(possible_entry)
            else:
                last_dl_entry.add_fallback(possible_entry)

        parent_version = Version(self.version.context, self.parent_version_id)
        parent_version.dl = self.dl
        parent_version.prepare_meta()
        parent_version.prepare_jar()
        # If no JVM exec is set, download the default JVM for the parent MC version.
        if self.jvm_exec is None:
            parent_version.prepare_jvm()
            self.jvm_exec = parent_version.jvm_exec

    def download(self):

        if self.main_dir is None:
            raise ValueError()

        self.check_download(self.dl.download_files())

    def check_download(self, report: DownloadReport):
        for entry, entry_fail in report.fails.items():
            if entry.dst == self.installer_file:
                raise ForgeVersionNotFound(ForgeVersionNotFound.INSTALLER_NOT_FOUND, self.version.forge_version)

    def install(self):

        if self.main_dir is None:
            raise ValueError()

        wrapper_jar_file = path.join(path.dirname(__file__), "wrapper", "target", "wrapper.jar")
        wrapper_completed = subprocess.run([
            self.jvm_exec,
            "-cp", path.pathsep.join([wrapper_jar_file, self.installer_file]),
            "portablemc.wrapper.Main",
            self.main_dir,
            self.version.id
        ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        os.remove(self.installer_file)

        if wrapper_completed.returncode != 0:
            raise ForgeInstallerFailed(wrapper_completed.returncode, wrapper_completed.stdout.decode("utf-8"))


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
    def __init__(self, return_code: int, output: str):
        self.return_code = return_code
        self.output = output


class ForgeVersionNotFound(BaseError):

    NOT_INSTALLED = "not_installed"
    INSTALLER_NOT_FOUND = "installer_not_found"
    MINECRAFT_VERSION_NOT_FOUND = "minecraft_version_not_found"  # DEPRECATED
    MINECRAFT_VERSION_NOT_SUPPORTED = "minecraft_version_not_supported"

    def __init__(self, code: str, version: str):
        super().__init__(code)
        self.version = version
