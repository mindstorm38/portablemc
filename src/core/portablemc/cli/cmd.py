from argparse import ArgumentParser, HelpFormatter
import sys

from ..util import LibrarySpecifier
from .output import Output, HumanOutput
from .lang import get as _

from typing import Optional, Type, List


EXIT_OK = 0
EXIT_FAILURE = 1


class Command:

    def register():
        pass


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


def register_arguments() -> ArgumentParser:
    parser = ArgumentParser(allow_abbrev=False, prog="portablemc", description=_("args"))
    parser.add_argument("--main-dir", help=_("args.main_dir"))
    parser.add_argument("--work-dir", help=_("args.work_dir"))
    parser.add_argument("--timeout", help=_("args.timeout"), type=float)
    parser.add_argument("--output", help=_("args.output"), default="human")
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
    parser.add_argument("-l", "--local", help=_("args.search.local"), action="store_true")
    parser.add_argument("input", nargs="?")


def register_start_arguments(parser: ArgumentParser):

    def resolution(raw: str):
        parts = raw.split("x")
        if len(parts) == 2:
            return (int(parts[0]), int(parts[1]))
        else:
            raise ValueError()

    def library_specifier(raw: str) -> LibrarySpecifierFilter:
        parts = raw.split(":")
        if len(parts) > 3:
            raise ValueError("Too much parts")
        def emptynone(s: str) -> Optional[str]:
            return None if s == "" else s
        return {
            1: lambda: LibrarySpecifierFilter(parts[0], None, None),
            2: lambda: LibrarySpecifierFilter(parts[0], emptynone(parts[1]), None),
            3: lambda: LibrarySpecifierFilter(parts[0], emptynone(parts[1]), emptynone(parts[2]))
        }[len(parts)]()
    
    parser.formatter_class = new_help_formatter_class(32)
    parser.add_argument("--dry", help=_("args.start.dry"), action="store_true")
    parser.add_argument("--disable-mp", help=_("args.start.disable_multiplayer"), action="store_true")
    parser.add_argument("--disable-chat", help=_("args.start.disable_chat"), action="store_true")
    parser.add_argument("--demo", help=_("args.start.demo"), action="store_true")
    parser.add_argument("--resol", help=_("args.start.resol"), type=resolution)
    parser.add_argument("--jvm", help=_("args.start.jvm"))
    parser.add_argument("--jvm-args", help=_("args.start.jvm_args"))
    parser.add_argument("--no-better-logging", help=_("args.start.no_better_logging"), action="store_true")
    parser.add_argument("--anonymise", help=_("args.start.anonymise"), action="store_true")
    parser.add_argument("--no-old-fix", help=_("args.start.no_old_fix"), action="store_true")
    parser.add_argument("--lwjgl", help=_("args.start.lwjgl"), choices=["3.2.3", "3.3.0", "3.3.1"])
    parser.add_argument("--exclude-lib", help=_("args.start.exclude_lib"), action="append", type=library_specifier)
    parser.add_argument("--include-bin", help=_("args.start.include_bin"), action="append")
    parser.add_argument("-t", "--temp-login", help=_("args.start.temp_login"), action="store_true")
    parser.add_argument("-l", "--login", help=_("args.start.login"))
    parser.add_argument("-m", "--microsoft", help=_("args.start.microsoft"), action="store_true")
    parser.add_argument("-u", "--username", help=_("args.start.username"), metavar="NAME")
    parser.add_argument("-i", "--uuid", help=_("args.start.uuid"))
    parser.add_argument("-s", "--server", help=_("args.start.server"))
    parser.add_argument("-p", "--server-port", type=int, help=_("args.start.server_port"), metavar="PORT")
    parser.add_argument("version", nargs="?", default="release")


def register_login_arguments(parser: ArgumentParser):
    parser.add_argument("-m", "--microsoft", help=_("args.login.microsoft"), action="store_true")
    parser.add_argument("email_or_username")


def register_logout_arguments(parser: ArgumentParser):
    parser.add_argument("-m", "--microsoft", help=_("args.logout.microsoft"), action="store_true")
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


class LibrarySpecifierFilter:
    """A filter for library specifier, used with the start command to exclude some 
    libraries.
    """
    
    __slots__ = "artifact", "version", "classifier"

    def __init__(self, artifact: str, version: Optional[str], classifier: Optional[str]):
        self.artifact = artifact
        self.version = version
        self.classifier = classifier
    
    def matches(self, spec: LibrarySpecifier) -> bool:
        return self.artifact == spec.artifact \
            and (self.version is None or self.version == spec.version) \
            and (self.classifier is None or (spec.classifier or "").startswith(self.classifier))

    def __str__(self) -> str:
        return f"{self.artifact}:{self.version or ''}" + ("" if self.classifier is None else f":{self.classifier}")
