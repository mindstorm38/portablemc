from argparse import ArgumentParser, Namespace
from os import path
import sys

from portablemc import Version, Context, DownloadEntry, json_simple_request, json_request


ARCHIVE_ITEMS = {
    "beta": "Minecraft-JE-Beta",
    "alpha": "Minecraft-JE-Alpha",
    "infdev": "Minecraft-JE-Infdev",
    "indev": "Minecraft-JE-Indev",
    "classic": "Minecraft-JE-Classic",
    "rubydung": "Minecraft-JE-Pre-Classic",
}


def load():

    from portablemc import cli as pmc
    from portablemc.cli import CliContext

    # Private mixins

    @pmc.mixin()
    def register_search_arguments(old, parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("-a", "--archives", help=_("args.search.archives"), action="store_true")
        old(parser)

    @pmc.mixin()
    def cmd_search(old, ns: Namespace, ctx: CliContext):

        if not ns.archives:
            return old(ns, ctx)

        _ = pmc.get_message
        table = []
        search = ns.input
        item_id_single = ARCHIVE_ITEMS.get(search)
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
                            pmc.format_locale_date(float(file["mtime"])),
                            _("search.flags.local") if ctx.has_version_metadata(f"archive-{version_id}") else ""
                        ))

        if item_id_single is not None:
            internal_search(search, item_id_single)
        else:
            for version_type_, item_id_ in ARCHIVE_ITEMS.items():
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
            return ArchivedVersion(ctx, version_id[4:])

        return old(ctx, version_id)

    # Messages

    pmc.messages.update({
        "args.search.archives": "Search in archives versions (this disable the --local argument).",
        "start.archives.version_not_found": "Archives version '{version}' not found."
    })


class ArchivedVersion(Version):

    def __init__(self, context: Context, real_id: str, *, prefix: str = "archive"):

        self.real_id = real_id

        if real_id.startswith("rd"):
            version_type = "rubydung"
        elif real_id.startswith("c"):
            version_type = "classic"
        elif real_id.startswith("in"):
            version_type = "indev"
        elif real_id.startswith("inf"):
            version_type = "infdev"
        elif real_id.startswith("a"):
            version_type = "alpha"
        elif real_id.startswith("b") or real_id.startswith("1.0.0"):
            version_type = "beta"
        else:
            raise ArchivesVersionNotFoundError(real_id)

        self.archives_item_id = ARCHIVE_ITEMS[version_type]

        super().__init__(context, f"{prefix}-{real_id}")


    def _validate_version_meta(self, version_id: str, version_dir: str, version_meta_file: str, version_meta: dict) -> bool:
        if version_id == self.id:
            return path.isfile(path.join(version_dir, f"{version_id}.jar"))
        else:
            return super()._validate_version_meta(version_id, version_dir, version_meta_file, version_meta)

    def _fetch_version_meta(self, version_id: str, version_dir: str, version_meta_file: str) -> dict:

        if version_id != self.id:
            return super()._fetch_version_meta(version_id, version_dir, version_meta_file)

        version_meta_url = get_archive_item_file_url(self.archives_item_id, f"{self.real_id}/{self.real_id}.json")

        status, version_meta = json_request(version_meta_url, "GET", ignore_error=True)
        if status != 200:
            raise ArchivesVersionNotFoundError(self.real_id)

        # Update the meta id to the real directory name.
        version_meta["id"] = self.real_id

        return version_meta

    def prepare_jar(self):

        self._check_version_meta()
        self.version_jar_file = path.join(self.version_dir, f"{self.id}.jar")

        if not path.isfile(self.version_jar_file):
            version_jar_url = get_archive_item_file_url(self.archives_item_id, f"{self.real_id}/{self.real_id}.jar")
            self.dl.append(DownloadEntry(version_jar_url, self.version_jar_file, name=f"{self.id}.jar"))


def request_archive_item_files(item_id: str) -> list:
    return json_simple_request(f"https://archive.org/metadata/{item_id}/files")["result"]


def get_archive_item_file_url(item_id: str, item_path: str) -> str:
    return f"https://archive.org/download/{item_id}/{item_path}"


# Errors

class ArchivesVersionNotFoundError(Exception):
    def __init__(self, version_id: str):
        self.version = version_id
