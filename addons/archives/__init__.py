from argparse import ArgumentParser, Namespace
from http.client import HTTPResponse
import urllib.request
from os import path
import shutil
import json
import sys
import os


def load(pmc):

    Version = pmc.Version
    StartOptions = pmc.StartOptions
    Start = pmc.Start
    CliContext = pmc.CliContext

    archive_items = {
        "beta": "Minecraft-JE-Beta",
        "alpha": "Minecraft-JE-Alpha",
        "infdev": "Minecraft-JE-Infdev",
        "indev": "Minecraft-JE-Indev",
        "classic": "Minecraft-JE-Classic",
        "rubydung": "Minecraft-JE-Pre-Classic",
    }

    @pmc.mixin()
    def register_search_arguments(old, parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("-a", "--archives", help=_("args.search.archives"), action="store_true")
        old(parser)

    @pmc.mixin()
    def register_start_arguments(old, parser: ArgumentParser):
        _ = pmc.get_message
        # parser.add_argument("--archives-prefix", help=_("args.start.archives_prefix"), default="archive", metavar="PREFIX")
        parser.add_argument("--no-old-fix", help=_("args.start.no_old_fix"), action="store_true")
        old(parser)

    @pmc.mixin()
    def cmd_search(old, ns: Namespace, ctx: CliContext):

        if not ns.archives:
            return old(ns, ctx)

        _ = pmc.get_message
        table = []
        search = ns.input
        item_id_single = archive_items.get(search)
        no_version = (item_id_single is not None or search is None)

        def internal_search(version_type: str, item_id: str):
            nonlocal table, search, no_version
            files = request_archive_item_files(item_id)
            for file in files:
                path_raw = file["name"]
                version_id = file["name"].split("/")[0]
                if path_raw == f"{version_id}/{version_id}.json":
                    if no_version or search in version_id:
                        table.append((
                            version_type,
                            version_id,
                            pmc.format_iso_date(float(file["mtime"])),
                            _("search.flags.local") if ctx.has_version_metadata(get_archive_version_id(version_id)) else ""
                        ))

        if item_id_single is not None:
            internal_search(search, item_id_single)
        else:
            for version_type_, item_id_ in archive_items.items():
                internal_search(version_type_, item_id_)

        if len(table):
            table.insert(0, (
                _("search.type"),
                _("search.name"),
                _("search.last_modified"),
                _("search.flags")
            ))
            pmc.print_table(table, header=0)
            sys.exit(pmc.EXIT_OK)
        else:
            pmc.print_message("search.not_found")
            sys.exit(pmc.EXIT_VERSION_NOT_FOUND)

    @pmc.mixin()
    def cmd_start(old, ns: Namespace, ctx: CliContext):
        try:
            return old(ns, ctx)
        except ArchivesVersionNotFoundError as err:
            pmc.print_task("FAILED", "start.archives.version_not_found", {"version": err.version}, done=True)
            sys.exit(pmc.EXIT_VERSION_NOT_FOUND)

    @pmc.mixin()
    def new_version(old, ctx: CliContext, version_id: str) -> Version:

        if version_id.startswith("arc:"):

            arc_version_id = version_id[4:]
            version_id = get_archive_version_id(arc_version_id)

            version_dir = ctx.get_version_dir(version_id)
            version_meta_file = path.join(version_dir, f"{version_id}.json")
            version_jar_file = path.join(version_dir, f"{version_id}.jar")

            if not path.isfile(version_meta_file) or not path.isfile(version_jar_file):

                pmc.print_task("", "start.archives.fetching")

                if arc_version_id.startswith("rd"):
                    version_type = "rubydung"
                elif arc_version_id.startswith("c"):
                    version_type = "classic"
                elif arc_version_id.startswith("in"):
                    version_type = "indev"
                elif arc_version_id.startswith("inf"):
                    version_type = "infdev"
                elif arc_version_id.startswith("a"):
                    version_type = "alpha"
                elif arc_version_id.startswith("b") or arc_version_id.startswith("1.0.0"):
                    version_type = "beta"
                else:
                    raise ArchivesVersionNotFoundError(arc_version_id)

                item_id = archive_items[version_type]
                pmc.print_task("", "start.archives.fetching_archives_org", {"item": item_id})

                version_meta_url = get_archive_item_file_url(item_id, f"{arc_version_id}/{arc_version_id}.json")
                version_jar_url = get_archive_item_file_url(item_id, f"{arc_version_id}/{arc_version_id}.jar")

                status, version_meta = pmc.json_request(version_meta_url, "GET", ignore_error=True)
                if status != 200:
                    raise ArchivesVersionNotFoundError(arc_version_id)

                os.makedirs(version_dir, exist_ok=True)
                with open(version_meta_file, "wt") as version_meta_fh:
                    json.dump(version_meta, version_meta_fh, indent=2)

                pmc.print_task("", "start.archives.downloading_jar")
                res: HTTPResponse = urllib.request.urlopen(version_jar_url)
                with open(version_jar_file, "wb") as version_jar_fh:
                    shutil.copyfileobj(res, version_jar_fh)
                pmc.print_task("OK", "start.archives.downloaded", {"version": arc_version_id}, done=True)

            return Version(ctx, version_id)

        return old(ctx, version_id)

    @pmc.mixin()
    def new_start(old, ctx: CliContext, version: Version) -> Start:

        start = old(ctx, version)

        is_alpha = version.id.startswith("a")

        if is_alpha:
            @pmc.mixin(into=start)
            def prepare(old_prepare, opts: StartOptions):
                old_prepare(opts)
                start.jvm_args.append("-Djava.util.Arrays.useLegacyMergeSort=true")
                start.jvm_args.append("-Dhttp.proxyHost=betacraft.pl")

        return start

    def request_archive_item_files(item_id: str) -> list:
        return pmc.json_simple_request(f"https://archive.org/metadata/{item_id}/files")["result"]

    def get_archive_item_file_url(item_id: str, item_path: str) -> str:
        return f"https://archive.org/download/{item_id}/{item_path}"

    def get_archive_version_id(version_id: str) -> str:
        return f"archive-{version_id}"

    # Errors

    class ArchivesVersionNotFoundError(Exception):
        def __init__(self, version_id: str):
            self.version = version_id

    # Messages

    pmc.messages.update({
        "args.search.archives": "Search in archives versions (this disable the --local argument).",
        "args.start.archives_prefix": "Change the prefix of the version ID when starting archives versions.",
        "args.start.no_old_fix": "Put this flag to disable the fixes for old versions (legacy merge sort, betacraft proxy).",
        "start.archives.fetching": "Fetching archives for version '{version}'...",
        "start.archives.fetching_archives_org": "Fetching archive.org for item '{item}'...",
        "start.archives.downloading_jar": "Downloading version JAR from archives...",
        "start.archives.downloaded": "Archives version '{version}' is ready.",
        "start.archives.version_not_found": "Archives version '{version}' not found."
    })
