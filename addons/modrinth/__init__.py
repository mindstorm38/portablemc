from argparse import ArgumentParser, Namespace
from typing import List, Dict, Optional
import urllib.parse as urlparse
from os import path
import sys
import os

from json import JSONDecodeError
import json


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
        register_modr_install_arguments(subparsers.add_parser("install", help=_("args.modrinth.install")))
        register_modr_status_arguments(subparsers.add_parser("status", help=_("args.modrinth.status")))
        register_modr_link_arguments(subparsers.add_parser("link", help=_("args.modrinth.link")))
        register_modr_unlink_arguments(subparsers.add_parser("unlink", help=_("args.modrinth.unlink")))
        register_modr_purge_arguments(subparsers.add_parser("purge", help=_("args.modrinth.purge")))

    def register_modr_search_arguments(parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("-o", "--offset", help=_("args.modrinth.search.offset"), default=0)
        parser.add_argument("query", nargs="?")

    def register_modr_install_arguments(parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("-y", "--yes", help=_("args.modrinth.install.yes"), action="store_true")
        parser.add_argument("specifier", nargs="+")

    def register_modr_status_arguments(_parser: ArgumentParser):
        pass

    def register_modr_link_arguments(parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("-o", "--override", help=_("args.modrinth.link.override"), action="store_true")
        parser.add_argument("pack_id", nargs="+", help=_("args.modrinth.link.pack_id"))

    def register_modr_unlink_arguments(parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("pack_id", nargs="+", help=_("args.modrinth.link.pack_id"))

    def register_modr_purge_arguments(parser: ArgumentParser):
        pass

    @pmc.mixin()
    def get_command_handlers(old):
        handlers = old()
        handlers["modr"] = {
            "search": cmd_modr_search,
            "install": cmd_modr_install,
            "status": cmd_modr_status,
            "link": cmd_modr_link,
            "unlink": cmd_modr_unlink,
            "purge": cmd_modr_purge
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
                pmc.ellipsis_str(hit["title"], 24),
                hit["author"],
                pmc.format_number(hit["downloads"]).lstrip()
            ))

        pmc.print_table(lines, header=0)

        sys.exit(pmc.EXIT_OK)

    def cmd_modr_install(ns: Namespace, ctx: CliContext):

        specifiers: List[str] = ns.specifier
        yes: bool = ns.yes

        mods_dir = path.join(ctx.work_dir, "mods")
        dl_list = DownloadList()

        groups = set()

        metadata = read_meta_file(ctx)

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
            pmc.print_task("", "modrinth.install.searching", task_msg_args)

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

            dst_dir = f"{mod_game_version}-{mod_loader}"
            dst_file_name = f"{mod_slug}-{mod_artifact_id}-{mod_game_version}-{mod_loader}.jar"
            dst_file_path = path.join(mods_dir, dst_dir, dst_file_name)

            mod_status = ""
            mod_must_install = True
            metadata_mod_file = metadata.get_mod(dst_dir, mod_slug)
            if metadata_mod_file is not None:
                if metadata_mod_file == dst_file_name:
                    mod_status = pmc.get_message("modrinth.install.already_installed")
                    mod_must_install = False
                else:
                    mod_status = pmc.get_message("modrinth.install.already_installed_another_version")
                    old_file_path = path.join(mods_dir, dst_dir, metadata_mod_file)
                    os.remove(old_file_path)

            pmc.print_task("OK", "modrinth.install.found", {
                "specifier": mod_slug,
                "artifact_id": mod_artifact_id,
                "loader": mod_loader,
                "game_version": mod_game_version,
                "status": mod_status
            }, done=True)

            if mod_must_install:

                metadata.add_mod(dst_dir, mod_slug, dst_file_name)

                dl_entry = DownloadEntry(selected_file["url"], dst_file_path, sha1=selected_file_sha1, name=dst_file_name)
                dl_list.append(dl_entry)

                # Add the game version and mod loader to groups set
                groups.add((mod_loader, mod_game_version))

                # If the pack (dest. directory) is currently activated
                if metadata.is_linked(dst_dir):
                    mod_link = path.join(mods_dir, dst_file_name)
                    def _link_mod():
                        os.symlink(dst_file_path, mod_link)
                    dl_list.add_callback(_link_mod)

        if len(groups) == 0:
            pmc.print_task(None, "modrinth.install.everything_already_installed", done=True)
            sys.exit(pmc.EXIT_OK)
        if not yes:
            if len(groups) > 1:
                # If there is more that one
                pmc.print_task(None, "modrinth.install.confirm_download_multiple_game_versions")
                print(" [y/N] ", end="")
                if pmc.prompt().lower() != "y":
                    pmc.print_task(None, "modrinth.install.abort", done=True)
                    sys.exit(pmc.EXIT_FAILURE)
            else:
                pmc.print_task(None, "modrinth.install.confirm_download")
                print(" [Y/n] ", end="")
                if pmc.prompt().lower() == "n":
                    pmc.print_task(None, "modrinth.install.abort", done=True)
                    sys.exit(pmc.EXIT_FAILURE)

        # Start download
        pmc.pretty_download(dl_list)

        # Update meta
        write_meta_file(ctx, metadata)

        sys.exit(pmc.EXIT_OK)

    def cmd_modr_status(_ns: Namespace, ctx: CliContext):

        _ = pmc.get_message

        metadata = read_meta_file(ctx)

        packs_table = [(
            _("modrinth.status.packs.id"),
            _("modrinth.status.packs.mods_count"),
            _("modrinth.status.packs.linked"),
        )]

        for pack_id in metadata.list_packs():
            packs_table.append((
                pack_id,
                str(len(metadata.get_mods(pack_id))),
                "yes" if metadata.is_linked(pack_id) else "no"
            ))

        pmc.print_table(packs_table, header=0)
        sys.exit(pmc.EXIT_OK)

    def cmd_modr_link(ns: Namespace, ctx: CliContext):

        pack_ids: List[str] = ns.pack_id
        metadata = read_meta_file(ctx)

        if ns.override:
            unlink_all_packs(ctx, metadata)

        for pack_id in pack_ids:
            if metadata.is_linked(pack_id):
                pmc.print_message("modrinth.link.already_linked", {"pack_id": pack_id})
            else:
                metadata_pack = metadata.get_mods(pack_id)
                if metadata_pack is None:
                    pmc.print_message("modrinth.link.not_found", {"pack_id": pack_id})
                else:
                    metadata.set_linked(pack_id, True)
                    for mod_id, mod_file in metadata_pack.items():
                        pmc.print_message("modrinth.link.linking", {"file_name": mod_file})
                        mod_path = path.join(ctx.work_dir, "mods", pack_id, mod_file)
                        mod_link_path = path.join(ctx.work_dir, "mods", mod_file)
                        os.symlink(mod_path, mod_link_path)
                    pmc.print_message("modrinth.link.success", {"pack_id": pack_id})

        write_meta_file(ctx, metadata)
        sys.exit(pmc.EXIT_OK)

    def cmd_modr_unlink(ns: Namespace, ctx: CliContext):

        pack_ids: List[str] = ns.pack_id
        metadata = read_meta_file(ctx)

        for pack_id in pack_ids:
            if metadata.is_linked(pack_id):
                unlink_pack(ctx, metadata, pack_id)
            elif metadata.has_pack(pack_id):
                pmc.print_message("modrinth.unlink.already_unlinked", {"pack_id": pack_id})
            else:
                pmc.print_message("modrinth.unlink.not_found", {"pack_id": pack_id})

        write_meta_file(ctx, metadata)
        sys.exit(pmc.EXIT_OK)

    def cmd_modr_purge(_ns: Namespace, ctx: CliContext):
        metadata = read_meta_file(ctx)
        unlink_all_packs(ctx, metadata)
        write_meta_file(ctx, metadata)

    def unlink_all_packs(ctx: CliContext, metadata: 'StatusMeta'):
        # Internal purge function
        for pack_id in metadata.list_packs():
            if metadata.is_linked(pack_id):
                unlink_pack(ctx, metadata, pack_id)

    def unlink_pack(ctx: CliContext, metadata: 'StatusMeta', pack_id: str):
        # Internal function
        for mod_file in metadata.get_mods(pack_id).values():
            pmc.print_message("modrinth.unlink.unlinking", {"file_name": mod_file})
            try:
                os.unlink(path.join(ctx.work_dir, "mods", mod_file))
            except OSError:
                pass
        metadata.set_linked(pack_id, False)
        pmc.print_message("modrinth.unlink.success", {"pack_id": pack_id})

    def request_api_v1(pth: str) -> dict:
        return pmc.json_simple_request(f"https://api.modrinth.com/api/v1/{pth}")

    class StatusMeta:

        def __init__(self, data):
            self.data = data

        def add_mod(self, pack_id: str, mod_id: str, mod_file: str):
            packs = self.data.get("packs")
            if packs is None:
                packs = self.data["packs"] = {}
            pack = packs.get(pack_id)
            if pack is None:
                pack = packs[pack_id] = {}
            mods = pack.get("mods")
            if mods is None:
                mods = pack["mods"] = {}
            mods[mod_id] = mod_file

        def has_pack(self, pack_id: str) -> bool:
            return pack_id in self.data.get("packs", {})

        def list_packs(self) -> List[str]:
            return list(self.data.get("packs", {}).keys())

        def get_mods(self, pack_id: str) -> Optional[Dict[str, str]]:
            pack = self.data.get("packs", {}).get(pack_id)
            return None if pack is None else pack.get("mods", {})

        def get_mod(self, pack_id: str, mod_id: str) -> Optional[str]:
            mods = self.get_mods(pack_id)
            return None if mods is None else mods.get(mod_id)

        def is_linked(self, pack_id: str) -> bool:
            return self.data.get("packs", {}).get(pack_id, {}).get("linked", False)

        def set_linked(self, pack_id: str, active: bool):
            pack = self.data.get("packs", {}).get(pack_id)
            if pack is not None:
                pack["linked"] = active

    def get_meta_file(ctx: CliContext) -> str:
        return path.join(ctx.work_dir, "mods", "portablemc_modrinth.json")

    def read_meta_file(ctx: CliContext) -> StatusMeta:
        try:
            with open(get_meta_file(ctx), "rt") as meta_fp:
                return StatusMeta(json.load(meta_fp))
        except (OSError, JSONDecodeError):
            return StatusMeta({})

    def write_meta_file(ctx: CliContext, meta: StatusMeta):
        with open(get_meta_file(ctx), "wt") as meta_fp:
            json.dump(meta.data, meta_fp, indent=2)


    # Messages

    pmc.messages.update({
        "args.modrinth": "Modrinth mods manager for Fabric and Forge.",
        "args.modrinth.search": "Search for mods.",
        "args.modrinth.search.offset": "The offset within the results (defaults to 0).",
        "args.modrinth.install": "Install mods (a list of the following syntax: "
                                 "<mod_id>[/<mod_artifact_id>][@<game_version_id>[-<forge|fabric>]]).",
        "args.modrinth.install.yes": "Do not ask for confirmation of installation.",
        "args.modrinth.status": "Show current status of the mods directory and the list of installed versions or mod packs.",
        "args.modrinth.link": "Link a mod pack.",
        "args.modrinth.link.pack_id": "The pack id, you can find installed pack list using 'modr status'.",
        "args.modrinth.link.override": "Unlinking all previous packs before linking.",
        "args.modrinth.unlink": "Unlink a mod pack.",
        "args.modrinth.purge": "Unlink all mod packs.",
        "modrinth.searching.index": "N°",
        "modrinth.searching.id": "Identifier",
        "modrinth.searching.name": "Name",
        "modrinth.searching.author": "Author",
        "modrinth.searching.downloads": "Downloads",
        "modrinth.install.searching": "Searching mod {specifier}...",
        "modrinth.install.found": "Found mod {specifier} ({artifact_id}) for {loader} on {game_version}. {status}",
        "modrinth.install.already_installed": "Already installed.",
        "modrinth.install.already_installed_another_version": "Already installed another version, overriding.",
        "modrinth.install.not_found": "Mod {specifier} not found.",
        "modrinth.install.requested_version_not_supported": "Found artifact {artifact} for mod {slug}, expected "
                                                            "{requested_version} version but {supported_versions} "
                                                            "are supported by the mod.",
        "modrinth.install.requested_loader_not_supported": "Found artifact {artifact} for mod {slug}, expected "
                                                           "{requested_loader} loader but {supported_loaders} "
                                                           "are supported by the mod.",
        "modrinth.install.confirm_download": "Confirm download?",
        "modrinth.install.confirm_download_multiple_game_versions": "Found multiple loaders and/or games versions. "
                                                                    "Confirm download?",
        "modrinth.install.abort": "Abort install.",
        "modrinth.install.everything_already_installed": "Everything is already installed.",
        "modrinth.status.packs.id": "Pack ID",
        "modrinth.status.packs.mods_count": "Mods count",
        "modrinth.status.packs.linked": "Linked",
        "modrinth.link.already_linked": "Pack {pack_id} is already linked.",
        "modrinth.link.not_found": "Pack {pack_id} was not found.",
        "modrinth.link.linking": "Linking {file_name}",
        "modrinth.link.success": "Linked pack {pack_id}.",
        "modrinth.unlink.already_unlinked": "Pack {pack_id} is already unlinked.",
        "modrinth.unlink.not_found": "Pack {pack_id} was not found, it cannot be unlinked.",
        "modrinth.unlink.unlinking": "Unlinking {file_name}",
        "modrinth.unlink.success": "Unlinked pack {pack_id}.",
    })
