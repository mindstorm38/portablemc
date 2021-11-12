from argparse import ArgumentParser, Namespace
import urllib.parse as urlparse
from typing import List
from os import path
import sys


def load(pmc):

    CliContext = pmc.CliContext
    DownloadList = pmc.DownloadList
    DownloadEntry = pmc.DownloadEntry

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
        _ = pmc.get_message
        # parser.add_argument("--type", help=_("args.modrinth.install."), default="release")
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

    def cmd_modr_install(ns: Namespace, ctx: CliContext):

        specifiers: List[str] = ns.specifier
        mods_dir = path.join(ctx.work_dir, "mods")
        dl_list = DownloadList()

        for specifier in specifiers:

            # Specifier parsing
            at_split = specifier.split("@", maxsplit=1)
            artifact_split = at_split[0].split("/", maxsplit=1)

            mod_slug_or_id = artifact_split[0]
            mod_artifact_id = artifact_split[1] if len(artifact_split) == 2 else None
            mod_game_version = None
            mod_loader = None

            if len(at_split) == 2:
                mod_game_version = at_split[1]
                if mod_game_version in ("fabric", "forge"):
                    mod_loader = mod_game_version
                    mod_game_version = None
                else:
                    dash_index = mod_game_version.rfind("-")
                    if dash_index != -1:
                        mod_loader_raw = mod_game_version[(dash_index + 1):]
                        if mod_loader_raw in ("fabric", "forge"):
                            mod_loader = mod_loader_raw
                            mod_game_version = mod_game_version[:dash_index]

            task_msg_args = {"specifier": specifier}
            pmc.print_task("", "modrinth.install.working", task_msg_args)

            # Requesting API for all available versions
            mod_data = request_api_v1(f"mod/{mod_slug_or_id}")
            mod_id = mod_data["id"]
            mod_slug = mod_data["slug"]  # The user can use the raw ID, so we set the real slug here
            versions_data = request_api_v1(f"mod/{mod_id}/version")

            selected_version_data = None

            # Iterating over available versions to find the best matching one
            for version_data in versions_data:

                valid_game_version = mod_game_version is None or mod_game_version in version_data["game_versions"]
                valid_loader = mod_loader is None or mod_loader in version_data["loaders"]

                if mod_artifact_id is not None:

                    if version_data["version_number"] == mod_artifact_id:
                        selected_version_data = version_data
                        if not valid_game_version:
                            pmc.print_task("FAILED", "modrinth.install.requested_version_not_supported", {
                                "artifact": mod_artifact_id,
                                "slug": mod_slug,
                                "supported_versions": ", ".join(version_data["game_versions"]),
                                "requested_version": mod_game_version
                            }, done=True)
                            sys.exit(pmc.EXIT_FAILURE)
                        elif not valid_loader:
                            pmc.print_task("FAILED", "modrinth.install.requested_loader_not_supported", {
                                "artifact": mod_artifact_id,
                                "slug": mod_slug,
                                "supported_loaders": ", ".join(version_data["loaders"]),
                                "requested_loader": mod_loader
                            }, done=True)
                            sys.exit(pmc.EXIT_FAILURE)

                else:

                    if valid_game_version and valid_loader:
                        selected_version_data = version_data
                        mod_artifact_id = version_data["version_number"]
                        break

            if selected_version_data is None:
                pmc.print_task("FAILED", "modrinth.install.not_found", task_msg_args, done=True)
                sys.exit(pmc.EXIT_FAILURE)

            # Fill missing info from selected version is needed
            if mod_game_version is None:
                mod_game_version = selected_version_data["game_versions"][-1]  # Last version seems to be the latest
            if mod_loader is None:
                mod_loader = selected_version_data["loaders"][0]

            # Get download information and append download entry to the list
            selected_file = selected_version_data["files"][0]
            selected_file_sha1 = selected_file["hashes"].get("sha1")

            dst_file_name = f"{mod_slug}-{mod_loader}-{mod_game_version}-{mod_artifact_id}.jar"
            dst_file_path = path.join(mods_dir, f"{mod_game_version}-{mod_loader}", dst_file_name)

            dl_entry = DownloadEntry(selected_file["url"], dst_file_path, sha1=selected_file_sha1, name=dst_file_name)
            dl_list.append(dl_entry)

            pmc.print_task("OK", "modrinth.install.resolved", {
                "specifier": f"{mod_slug}/{mod_artifact_id}@{mod_game_version}-{mod_loader}"
            }, done=True)

        # Start download
        pmc.pretty_download(dl_list)

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

    def request_api_v1(path: str) -> dict:
        # print(f"https://api.modrinth.com/api/v1/{path}")
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
        "modrinth.install.working": "Fetching mod {specifier}...",
        "modrinth.install.resolved": "Resolved mod {specifier}.",
        "modrinth.install.not_found": "Mod {specifier} not found.",
        "modrinth.install.requested_version_not_supported": "Found artifact {artifact} for mod {slug}, expected "
                                                            "{requested_version} version but {supported_versions} "
                                                            "are supported by the mod.",
        "modrinth.install.requested_loader_not_supported": "Found artifact {artifact} for mod {slug}, expected "
                                                           "{requested_loader} loader but {supported_loaders} "
                                                           "are supported by the mod.",
    })
