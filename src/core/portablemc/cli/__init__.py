"""Main 
"""

from argparse import Namespace
import sys

from .parse import register_arguments
from .util import format_locale_date
from .output import Table
from .lang import get as _

from ..manifest import VersionManifest, VersionManifestError
from ..standard import Context

from typing import TYPE_CHECKING, Optional, List, Union, Dict, Callable, Any

if TYPE_CHECKING:
    from .parse import RootCmd, SearchCmd


EXIT_OK = 0
EXIT_FAILURE = 1


def main(args: Optional[List[str]] = None):
    """Main entry point of the CLI. This function parses the input arguments and try to
    find a command handler to dispatch to. These command handlers are specified by the
    `get_command_handlers` function.
    """

    parser = register_arguments()
    ns = parser.parse_args(args or sys.argv[1:])

    command_handlers = get_command_handlers()
    command_attr = "subcommand"
    while True:
        command = getattr(ns, command_attr)
        handler = command_handlers.get(command)
        if handler is None:
            parser.print_help()
            sys.exit(EXIT_FAILURE)
        elif callable(handler):
            handler(ns)
        elif isinstance(handler, dict):
            command_attr = f"{command}_{command_attr}"
            command_handlers = handler
            continue
        sys.exit(EXIT_OK)


CommandFunc = Callable[[Namespace], Any]
CommandDict = Dict[str, Union[CommandFunc, "CommandDict"]]


def get_command_handlers() -> CommandDict:
    """This internal function returns the tree of command handlers for each subcommand
    of the CLI argument parser.
    """

    return {
        "search": cmd_search,
        # "start": cmd_start,
        # "login": cmd_login,
        # "logout": cmd_logout,
        # "show": {
        #     "about": cmd_show_about,
        #     "auth": cmd_show_auth,
        #     "lang": cmd_show_lang,
        # },
        # "addon": {
        #     "list": cmd_addon_list,
        #     "show": cmd_addon_show
        # }
    }


def cmd_search(ns: "SearchCmd"):

    def cmd_search_manifest(search: Optional[str], table: Table):
        
        table.add(
            _("search.type"),
            _("search.name"),
            _("search.release_date"),
            _("search.flags"))
        table.separator()

        manifest = new_version_manifest(ns)
        search, alias = manifest.filter_latest(search)
        try:
            for version_data in manifest.all_versions():
                version_id = version_data["id"]
                if search is None or (alias and search == version_id) or (not alias and search in version_id):
                    table.add(
                        version_data["type"], 
                        version_id, 
                        format_locale_date(version_data["releaseTime"]),
                        "")

                    # table.append((
                    #     version_data["type"],
                    #     version_id,
                    #     format_locale_date(version_data["releaseTime"]),
                    #     _("search.flags.local") if ctx.has_version_metadata(version_id) else ""
                    # ))

        except VersionManifestError as err:
            pass
            # print_task("FAILED", f"version_manifest.error.{err.code}", done=True)
            # sys.exit(EXIT_VERSION_NOT_FOUND)

    def cmd_search_local(search: Optional[str], table: Table):

        table.add(
            _("search.name"),
            _("search.last_modified"))
        table.separator()

        raise NotImplementedError

    search_handler = {
        "manifest": cmd_search_manifest,
        "local": cmd_search_local
    }[ns.kind]

    table = ns.out.table()
    search_handler(ns.input, table)

    table.print()
    sys.exit(EXIT_OK)


def new_version_manifest(ns: "RootCmd") -> VersionManifest:
    return VersionManifest()
