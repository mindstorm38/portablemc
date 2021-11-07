from argparse import ArgumentParser, Namespace
import urllib.parse as urlparse
from typing import List
import sys


def load(pmc):

    CliContext = pmc.CliContext

    @pmc.mixin()
    def register_subcommands(old, subparsers):
        _ = pmc.get_message
        old(subparsers)
        register_modr_arguments(subparsers.add_parser("modr", help=_("args.modrinth")))

    def register_modr_arguments(parser: ArgumentParser):
        _ = pmc.get_message
        subparsers = parser.add_subparsers(title="subcommands", dest="modr_subcommand")
        subparsers.required = True
        register_modr_search_arguments(subparsers.add_parser("search", help=_("args.modrinth.search")))
        register_modr_install_arguments(subparsers.add_parser("install", help=_("args.modrinth.about")))

    def register_modr_search_arguments(parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("-o", "--offset", help=_("args.modrinth.search.offset"), default=0)
        parser.add_argument("query", nargs="?")

    def register_modr_install_arguments(parser: ArgumentParser):
        parser.add_argument("specifier", nargs="+")

    @pmc.mixin()
    def get_command_handlers(old):
        handlers = old()
        handlers["modr"] = {
            "search": cmd_modr_search,
            "install": cmd_modr_install
        }
        return handlers

    def cmd_modr_search(ns: Namespace, _ctx: CliContext):

        query = ns.query or ""

        try:
            offset = int(ns.offset)
        except ValueError:
            offset = 0

        _ = pmc.get_message

        data = request_api_v1(f"mod?query={urlparse.quote(query)}&offset={offset}")
        data_offset = data["offset"]
        data_total_count = data["total_hits"]

        lines = [(
            _("modrinth.searching.index", total_count=data_total_count),
            _("modrinth.searching.id"),
            _("modrinth.searching.name"),
            _("modrinth.searching.author"),
            _("modrinth.searching.downloads"),
        )]

        for i, hit in enumerate(data["hits"]):
            lines.append((
                f"{data_offset + i + 1}",
                hit["slug"],
                add_string_ellipsis(hit["title"], 24),
                hit["author"],
                pmc.format_number(hit["downloads"]).lstrip()
            ))

        pmc.print_table(lines, header=0)

        sys.exit(pmc.EXIT_OK)

    def cmd_modr_install(ns: Namespace, _ctx: CliContext):

        specifiers: List[str] = ns.specifier

        for specifier in specifiers:

            at_split = specifier.split("@", maxsplit=1)
            artifact_split = at_split[0].split("/", maxsplit=1)

            mod_slug = artifact_split[0]
            mod_artifact_id = artifact_split[1] if len(artifact_split) == 2 else None
            mod_game_version = None
            mod_loader = None

            if len(at_split) == 2:
                mod_game_version = at_split[1]
                dash_index = mod_game_version.rfind("-")
                if dash_index != -1:
                    mod_loader_raw = mod_game_version[(dash_index + 1):]
                    if mod_loader_raw in ("fabric", "forge"):
                        mod_loader = mod_loader_raw
                        mod_game_version = mod_game_version[:dash_index]

            pmc.print_task("", "modrinth.install.working", {"mod_slug": mod_slug})

            mod_data = request_api_v1(f"mod/{mod_slug}")
            mod_id = mod_data["id"]
            versions_data = request_api_v1(f"mod/{mod_id}/version")

            selected_version_data = None

            for version_data in versions_data:
                if mod_artifact_id is not None:
                    if version_data["version_number"] == mod_artifact_id:
                        selected_version_data = mod_artifact_id
                        break
                else:
                    pass  # TODO

            pmc.print_task("OK", "modrinth.install.resolved", {"mod_slug": mod_slug}, done=True)


        # TODO: Syntax
        # <mod_id>[/<mod_artifact_id>][@<game_version_id>[-<forge|fabric>]]
        #
        # sodium@1.17.1
        # sodium@1.17.1-fabric
        # sodium/mc1.17.1-0.3.2
        #
        # sodium/mc1.16.3-0.1.0
        # sodium/mc1.16.3-0.1.0@1.16.5
        # sodium/mc1.16.3-0.1.0@1.16.3
        # sodium/mc1.16.3-0.1.0@1.16.5-fabric
        #
        # File structure will be like this:
        # + mods
        #   + fabric-1.17.1
        #     + sodium-fabric-mc1.17.1-0.3.2.jar
        #   + fabric-1.16.5
        #     + sodium-fabric-mc1.16.3-0.1.0.jar
        #   + fabric-1.16.3
        #     + sodium-fabric-mc1.16.3-0.1.0.jar

        sys.exit(pmc.EXIT_OK)

    def request_api_v1(path: str) -> dict:
        print(f"https://api.modrinth.com/api/v1/{path}")
        return pmc.json_simple_request(f"https://api.modrinth.com/api/v1/{path}")

    def add_string_ellipsis(s: str, l: int) -> str:
        return f"{s[:(l - 3)]}..." if len(s) > l else s

    # Messages

    pmc.messages.update({
        "args.modrinth": "Modrinth mods manager for Fabric and Forge.",
        "args.modrinth.search": "Search for mods.",
        "args.modrinth.search.offset": "The offset within the results (defaults to 0).",
        "args.modrinth.install": "Install mods.",
        "modrinth.searching.index": "N°",
        "modrinth.searching.id": "Identifier",
        "modrinth.searching.name": "Name",
        "modrinth.searching.author": "Author",
        "modrinth.searching.downloads": "Downloads",
        "modrinth.install.working": "Installing {mod_slug}...",
        "modrinth.install.resolved": "Resolved {mod_slug}, scheduled for download."
    })
