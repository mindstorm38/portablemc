"""Main 
"""

from argparse import Namespace
import sys

from .output import Output, HumanOutput
from .cmd import register_arguments
from .lang import get as _

from ..manifest import VersionManifest
from ..standard import Context

from typing import Optional, List, Union, Dict, Callable, Any


EXIT_OK = 0
EXIT_FAILURE = 1


def main(args: Optional[List[str]] = None):
    """Main entry point of the CLI. This function parses the input arguments and try to
    find a command handler to dispatch to. These command handlers are specified by the
    `get_command_handlers` function.
    """

    parser = register_arguments()
    ns = parser.parse_args(args or sys.argv[1:])

    cli = Cli(ns, HumanOutput())

    command_handlers = get_command_handlers()
    command_attr = "subcommand"
    while True:
        command = getattr(ns, command_attr)
        handler = command_handlers.get(command)
        if handler is None:
            parser.print_help()
            sys.exit(EXIT_FAILURE)
        elif callable(handler):
            handler(cli)
        elif isinstance(handler, dict):
            command_attr = f"{command}_{command_attr}"
            command_handlers = handler
            continue
        sys.exit(EXIT_OK)


CommandFunc = Callable[[Cli], Any]
CommandDict = Dict[str, Union[CommandFunc, "CommandDict"]]


class Cli:
    """A bundle of runtime properties for the CLI. This contains the namespace of parsed
    CLI arguments with the output handle used to print things to the user.
    """

    def __init__(self, ns: Namespace, out: Output) -> None:
        self.ns = ns
        self.out = out


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


def cmd_search(cli: Cli):

    table = []
    search = cli.ns.input
    no_version = search is None

    if cli.ns.local:
        raise NotImplementedError
        # for version_id, mtime in ctx.list_versions():
        #     if no_version or search in version_id:
        #         table.append((version_id, format_locale_date(mtime)))
    else:

        manifest = VersionManifest()

        manifest = new_version_manifest(ctx)
        search, alias = manifest.filter_latest(search)
        try:
            for version_data in manifest.all_versions():
                version_id = version_data["id"]
                if no_version or (alias and search == version_id) or (not alias and search in version_id):
                    table.append((
                        version_data["type"],
                        version_id,
                        format_locale_date(version_data["releaseTime"]),
                        _("search.flags.local") if ctx.has_version_metadata(version_id) else ""
                    ))
        except VersionManifestError as err:
            print_task("FAILED", f"version_manifest.error.{err.code}", done=True)
            sys.exit(EXIT_VERSION_NOT_FOUND)

    if len(table):
        table.insert(0, (
            _("search.name"),
            _("search.last_modified")
        ) if ns.local else (
            _("search.type"),
            _("search.name"),
            _("search.release_date"),
            _("search.flags")
        ))
        print_table(table, header=0)
        sys.exit(EXIT_OK)
    else:
        print_message("search.not_found")
        sys.exit(EXIT_VERSION_NOT_FOUND)


# def cmd_start(ns: Namespace, out: Output):
    
#     from portablemc.standard import make_standard_sequence

#     seq = make_standard_sequence()













# def cmd(handler: CommandHandler, ns: Namespace):
#     try:
#         handler(ns)
#     except JsonRequestError as err:
#         print_task("FAILED", f"json_request.error.{err.code}", {
#             "url": err.url,
#             "method": err.method,
#             "status": err.status,
#             "data": err.data,
#         }, done=True, keep_previous=True)
#         sys.exit(EXIT_JSON_REQUEST_ERROR)
#     except KeyboardInterrupt:
#         print_task(None, "error.keyboard_interrupt", done=True, keep_previous=True)
#         sys.exit(EXIT_FAILURE)
#     except Exception as err:
#         import ssl
#         key = "error.generic"
#         if isinstance(err, URLError) and isinstance(err.reason, ssl.SSLCertVerificationError):
#             key = "error.cert"
#         elif isinstance(err, (URLError, socket.gaierror, socket.timeout)):
#             key = "error.socket"
#         print_task("FAILED", key, done=True, keep_previous=True)
#         import traceback
#         traceback.print_exc()
#         sys.exit(EXIT_FAILURE)
