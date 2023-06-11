from argparse import ArgumentParser, HelpFormatter, Namespace
from pathlib import Path

from .output import Output, HumanOutput, JsonOutput
from .util import LibrarySpecifierFilter
from .lang import get as _

from typing import TYPE_CHECKING, Optional, Type, Tuple, List


if TYPE_CHECKING:

    class RootCmd(Namespace):
        main_dir: Optional[Path]
        work_dir: Optional[Path]
        timeout: float
        out: Output
    
    class SearchCmd(RootCmd):
        kind: str
        input: str

    class StartCmd(RootCmd):
        dry: bool
        disable_mp: bool
        disable_chat: bool
        demo: bool
        resolution: Optional[Tuple[int, int]]
        jvm: Optional[str]
        jvm_args: Optional[str]
        no_better_logging: bool
        anonymize: bool
        no_legacy_fix: bool
        lwjgl: Optional[str]
        exclude_lib: Optional[List[LibrarySpecifierFilter]]
        include_bin: Optional[List[str]]
        temp_login: bool
        login: str
        login_service: str
        username: Optional[str]
        uuid: Optional[str]
        server: Optional[str]
        server_port: Optional[int]
        version: str

    class LoginCmd(RootCmd):
        login_service: str
        email_or_username: str

    class LogoutCmd(RootCmd):
        login_service: str
        email_or_username: str
    
    class ShowCmd(RootCmd):
        pass

    class ShowAboutCmd(ShowCmd):
        pass

    class ShowAuthCmd(ShowCmd):
        pass

    class ShowLangCmd(ShowCmd):
        pass


def register_arguments() -> ArgumentParser:
    parser = ArgumentParser(allow_abbrev=False, prog="portablemc", description=_("args"))
    parser.add_argument("--main-dir", help=_("args.main_dir"), type=Path)
    parser.add_argument("--work-dir", help=_("args.work_dir"), type=Path)
    parser.add_argument("--timeout", help=_("args.timeout"), type=float)
    parser.add_argument("--output", help=_("args.output"), dest="out", type=get_output_from_str, default="human")
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
    parser.add_argument("-k", "--kind", help=_("args.search.kind"), default="manifest", choices=["manifest", "local"])
    parser.add_argument("input", nargs="?")


def register_common_login_service(parser: ArgumentParser):
    parser.add_argument("--login-service", help=_("args.common.login_service"), default="microsoft", choices=["microsoft", "mojang"])


def register_start_arguments(parser: ArgumentParser):

    def resolution(raw: str) -> Tuple[int, int]:
        parts = raw.split("x")
        if len(parts) == 2:
            return (int(parts[0]), int(parts[1]))
        else:
            raise ValueError("Expected format: <width>x<height>")
    
    parser.formatter_class = new_help_formatter_class(32)
    parser.add_argument("--dry", help=_("args.start.dry"), action="store_true")
    parser.add_argument("--disable-mp", help=_("args.start.disable_multiplayer"), action="store_true")
    parser.add_argument("--disable-chat", help=_("args.start.disable_chat"), action="store_true")
    parser.add_argument("--demo", help=_("args.start.demo"), action="store_true")
    parser.add_argument("--resolution", help=_("args.start.resolution"), type=resolution)
    parser.add_argument("--jvm", help=_("args.start.jvm"))
    parser.add_argument("--jvm-args", help=_("args.start.jvm_args"))
    parser.add_argument("--no-better-logging", help=_("args.start.no_better_logging"), action="store_true")
    parser.add_argument("--anonymize", help=_("args.start.anonymize"), action="store_true")
    parser.add_argument("--no-legacy-fix", help=_("args.start.no_legacy_fix"), action="store_true")
    parser.add_argument("--lwjgl", help=_("args.start.lwjgl"), choices=["3.2.3", "3.3.0", "3.3.1"])
    parser.add_argument("--exclude-lib", help=_("args.start.exclude_lib"), action="append", type=LibrarySpecifierFilter.from_str)
    parser.add_argument("--include-bin", help=_("args.start.include_bin"), action="append")
    register_common_login_service(parser)
    parser.add_argument("-t", "--temp-login", help=_("args.start.temp_login"), action="store_true")
    parser.add_argument("-l", "--login", help=_("args.start.login"))
    parser.add_argument("-u", "--username", help=_("args.start.username"), metavar="NAME")
    parser.add_argument("-i", "--uuid", help=_("args.start.uuid"))
    parser.add_argument("-s", "--server", help=_("args.start.server"))
    parser.add_argument("-p", "--server-port", type=int, help=_("args.start.server_port"), metavar="PORT")
    parser.add_argument("version", nargs="?", default="release")


def register_login_arguments(parser: ArgumentParser):
    register_common_login_service(parser)
    parser.add_argument("email_or_username")


def register_logout_arguments(parser: ArgumentParser):
    register_common_login_service(parser)
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


def get_output_from_str(s: str) -> Output:
    """This function is used to parse the output argument given.
    """
    if s == "human":
        return HumanOutput()
    elif s == "json":
        return JsonOutput();
    else:
        raise ValueError(f"invalid type: {s}")
