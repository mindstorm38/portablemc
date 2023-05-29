from argparse import ArgumentParser, HelpFormatter

from ..util import LibrarySpecifier

from typing import Optional, Type


def register_arguments() -> ArgumentParser:
    _ = get_message
    parser = ArgumentParser(allow_abbrev=False, prog="portablemc", description=_("args"))
    parser.add_argument("--main-dir", help=_("args.main_dir"))
    parser.add_argument("--work-dir", help=_("args.work_dir"))
    parser.add_argument("--timeout", help=_("args.timeout"), type=float)
    register_subcommands(parser.add_subparsers(title="subcommands", dest="subcommand"))
    return parser


def register_subcommands(subparsers):
    _ = get_message
    register_search_arguments(subparsers.add_parser("search", help=_("args.search")))
    register_start_arguments(subparsers.add_parser("start", help=_("args.start")))
    register_login_arguments(subparsers.add_parser("login", help=_("args.login")))
    register_logout_arguments(subparsers.add_parser("logout", help=_("args.logout")))
    register_show_arguments(subparsers.add_parser("show", help=_("args.show")))
    register_addon_arguments(subparsers.add_parser("addon", help=_("args.addon")))


def register_search_arguments(parser: ArgumentParser):
    parser.add_argument("-l", "--local", help=get_message("args.search.local"), action="store_true")
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
    
    _ = get_message
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
    parser.add_argument("-m", "--microsoft", help=get_message("args.login.microsoft"), action="store_true")
    parser.add_argument("email_or_username")


def register_logout_arguments(parser: ArgumentParser):
    parser.add_argument("-m", "--microsoft", help=get_message("args.logout.microsoft"), action="store_true")
    parser.add_argument("email_or_username")


def register_show_arguments(parser: ArgumentParser):
    _ = get_message
    subparsers = parser.add_subparsers(title="subcommands", dest="show_subcommand")
    subparsers.required = True
    subparsers.add_parser("about", help=_("args.show.about"))
    subparsers.add_parser("auth", help=_("args.show.auth"))
    subparsers.add_parser("lang", help=_("args.show.lang"))


def register_addon_arguments(parser: ArgumentParser):
    _ = get_message
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
