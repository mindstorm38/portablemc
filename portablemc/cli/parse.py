from argparse import ArgumentParser, HelpFormatter, ArgumentTypeError
from pathlib import Path

from ..standard import Context, VersionManifest
from ..auth import AuthDatabase

from .util import LibrarySpecifierFilter
from .output import Output
from .lang import get as _

from typing import Optional, Type, Tuple, List


# The following classes are only used for type checking and represent a typed namespace
# as produced by the arguments registered to the argument parser.

class RootNs:
    main_dir: Optional[Path]
    work_dir: Optional[Path]
    timeout: float
    out_kind: str
    verbose: int
    # Initialized by main function after argument parsing.
    out: Output
    context: Context
    version_manifest: VersionManifest
    auth_database: AuthDatabase

class SearchNs(RootNs):
    kind: str
    input: str

class StartNs(RootNs):
    dry: bool
    disable_mp: bool
    disable_chat: bool
    demo: bool
    resolution: Optional[Tuple[int, int]]
    jvm: Optional[str]
    jvm_args: Optional[str]
    no_fix: bool
    fabric_prefix: str
    quilt_prefix: str
    forge_prefix: str
    lwjgl: Optional[str]
    exclude_lib: Optional[List[LibrarySpecifierFilter]]
    include_bin: Optional[List[str]]
    temp_login: bool
    login: str
    auth_service: str
    auth_anonymize: bool
    username: Optional[str]
    uuid: Optional[str]
    server: Optional[str]
    server_port: Optional[int]
    version: str

class LoginNs(RootNs):
    auth_service: str
    email_or_username: str

class LogoutNs(RootNs):
    auth_service: str
    email_or_username: str


def register_arguments() -> ArgumentParser:
    parser = ArgumentParser(allow_abbrev=False, prog="portablemc", description=_("args"))
    parser.add_argument("--main-dir", help=_("args.main_dir"), type=Path)
    parser.add_argument("--work-dir", help=_("args.work_dir"), type=Path)
    parser.add_argument("--timeout", help=_("args.timeout"), type=float)
    parser.add_argument("--output", help=_("args.output"), dest="out_kind", choices=get_outputs(), default="human-color")
    parser.add_argument("-v", dest="verbose", help=_("args.verbose"), action="count", default=0)
    register_subcommands(parser.add_subparsers(title="subcommands", dest="subcommand"))
    return parser


def register_subcommands(subparsers):
    register_search_arguments(subparsers.add_parser("search", help=_("args.search")))
    register_start_arguments(subparsers.add_parser("start", help=_("args.start")))
    register_login_arguments(subparsers.add_parser("login", help=_("args.login")))
    register_logout_arguments(subparsers.add_parser("logout", help=_("args.logout")))
    register_show_arguments(subparsers.add_parser("show", help=_("args.show")))
    register_addon_arguments(subparsers.add_parser("addon", help=_("args.addon")))


def register_search_arguments(parser: ArgumentParser):
    parser.add_argument("-k", "--kind", help=_("args.search.kind"), default="mojang", choices=get_search_kinds())
    parser.add_argument("input", nargs="?")


def register_common_auth_service(parser: ArgumentParser):
    parser.add_argument("--auth-service", help=_("args.common.auth_service"), default="microsoft", choices=get_auth_services())


def register_start_arguments(parser: ArgumentParser):
    parser.formatter_class = new_help_formatter_class(40)
    parser.add_argument("--dry", help=_("args.start.dry"), action="store_true")
    parser.add_argument("--disable-mp", help=_("args.start.disable_multiplayer"), action="store_true")
    parser.add_argument("--disable-chat", help=_("args.start.disable_chat"), action="store_true")
    parser.add_argument("--demo", help=_("args.start.demo"), action="store_true")
    parser.add_argument("--resolution", help=_("args.start.resolution"), type=resolution_from_str)
    parser.add_argument("--jvm", help=_("args.start.jvm"))
    parser.add_argument("--jvm-args", help=_("args.start.jvm_args"), metavar="ARGS")
    parser.add_argument("--no-fix", help=_("args.start.no_fix"), action="store_true")
    parser.add_argument("--fabric-prefix", help=_("args.start.fabric_prefix"), default="fabric", metavar="PREFIX")
    parser.add_argument("--quilt-prefix", help=_("args.start.quilt_prefix"), default="quilt", metavar="PREFIX")
    parser.add_argument("--forge-prefix", help=_("args.start.forge_prefix"), default="forge", metavar="PREFIX")
    parser.add_argument("--lwjgl", help=_("args.start.lwjgl"), choices=["3.2.3", "3.3.0", "3.3.1", "3.3.2"])
    parser.add_argument("--exclude-lib", help=_("args.start.exclude_lib"), action="append", metavar="SPEC", type=LibrarySpecifierFilter.from_str)
    parser.add_argument("--include-bin", help=_("args.start.include_bin"), action="append", metavar="PATH")
    parser.add_argument("--auth-anonymize", help=_("args.start.auth_anonymize"), action="store_true")
    register_common_auth_service(parser)
    parser.add_argument("-t", "--temp-login", help=_("args.start.temp_login"), action="store_true")
    parser.add_argument("-l", "--login", help=_("args.start.login"))
    parser.add_argument("-u", "--username", help=_("args.start.username"), metavar="NAME")
    parser.add_argument("-i", "--uuid", help=_("args.start.uuid"))
    parser.add_argument("-s", "--server", help=_("args.start.server"))
    parser.add_argument("-p", "--server-port", type=int, help=_("args.start.server_port"), metavar="PORT")
    parser.add_argument("version", nargs="?", default="release", help=_("args.start.version", formats=", ".join(map(lambda s: _(f"args.start.version.{s}"), ("standard", "fabric", "quilt", "forge")))))


def register_login_arguments(parser: ArgumentParser):
    register_common_auth_service(parser)
    parser.add_argument("email_or_username")


def register_logout_arguments(parser: ArgumentParser):
    register_common_auth_service(parser)
    parser.add_argument("email_or_username")


def register_show_arguments(parser: ArgumentParser):
    subparsers = parser.add_subparsers(title="subcommands", dest="show_subcommand")
    subparsers.required = True
    subparsers.add_parser("about", help=_("args.show.about"))
    subparsers.add_parser("auth", help=_("args.show.auth"))
    subparsers.add_parser("lang", help=_("args.show.lang"))


def register_addon_arguments(parser: ArgumentParser):
    subparsers = parser.add_subparsers(title="subcommands", dest="addon_subcommand")
    subparsers.required = True
    subparsers.add_parser("list", help=_("args.addon.list"))
    show_parser = subparsers.add_parser("show", help=_("args.addon.show"))
    show_parser.add_argument("addon_id")


def new_help_formatter_class(max_help_position: int) -> Type[HelpFormatter]:

    class CustomHelpFormatter(HelpFormatter):
        def __init__(self, prog):
            super().__init__(prog, max_help_position=max_help_position)

    return CustomHelpFormatter


def get_outputs() -> List[str]:
    return ["human-color", "human", "machine"]


def get_search_kinds() -> List[str]:
    return ["mojang", "local", "forge", "fabric", "quilt"]


def get_auth_services() -> List[str]:
    return ["microsoft", "yggdrasil"]


def resolution_from_str(s: str) -> Tuple[int, int]:
    parts = s.split("x")
    if len(parts) == 2:
        return (int(parts[0]), int(parts[1]))
    else:
        raise ArgumentTypeError(_("args.start.resolution.invalid", given=s))
