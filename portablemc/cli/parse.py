from argparse import ArgumentParser, Action, SUPPRESS, \
    HelpFormatter, RawDescriptionHelpFormatter, ArgumentTypeError
from pathlib import Path
import sys
import os

from portablemc.standard import Context, VersionManifest
from portablemc.auth import AuthDatabase

from .util import LibrarySpecifierFilter
from .output import Output
from .lang import get as _

from typing import Optional, Type, Tuple, List, Dict

# The following classes are only used for type checking and represent a typed namespace
# as produced by the arguments registered to the argument parser.

class RootNs:
    main_dir: Optional[Path]
    work_dir: Optional[Path]
    timeout: float
    out_kind: str
    verbose: int
    # Initialized by main function after argument parsing.
    parser: ArgumentParser
    out: Output
    context: Context
    version_manifest: VersionManifest
    auth_database: AuthDatabase
    socket_error_tips: List[str]

class SearchNs(RootNs):
    kind: str
    input: str

class AuthBaseNs(RootNs):
    auth_service: str
    auth_no_browser: bool

class StartNs(AuthBaseNs):
    dry: bool
    disable_mp: bool
    disable_chat: bool
    demo: bool
    resolution: Optional[Tuple[int, int]]
    jvm: Optional[Path]
    jvm_args: Optional[str]
    no_fix: bool
    babric_prefix: str
    fabric_prefix: str
    legacyfabric_prefix: str
    quilt_prefix: str
    forge_prefix: str
    neoforge_prefix: str
    lwjgl: Optional[str]
    exclude_lib: Optional[List[LibrarySpecifierFilter]]
    include_bin: Optional[List[Path]]
    temp_login: bool
    auth_service: str
    auth_anonymize: bool
    login: Optional[str]
    username: Optional[str]
    uuid: Optional[str]
    server: Optional[str]
    server_port: Optional[int]
    version: str

class LoginNs(AuthBaseNs):
    email_or_username: str

class LogoutNs(AuthBaseNs):
    email_or_username: str

class ShowCompletionNs(RootNs):
    shell: str


def register_common_help(parser: ArgumentParser) -> None:
    parser.formatter_class = new_help_formatter_class(40)
    parser.add_argument("-h", "--help", action="help", default=SUPPRESS, help=_("args.common.help"))


def register_common_auth_service(parser: ArgumentParser) -> None:
    
    auth_choices = get_auth_services()
    auth_arg = parser.add_argument("--auth-service", help=_("args.common.auth_service"), default="microsoft", choices=auth_choices)
    for choice in auth_choices:
        add_completion(auth_arg, choice, _(f"args.common.auth_service.comp.{choice}"))

    parser.add_argument("--auth-no-browser", help=_("args.common.auth_no_browser"), action="store_true")


def register_arguments() -> ArgumentParser:

    parser = ArgumentParser(allow_abbrev=False, prog="portablemc", description=_("args._"), add_help=False)
    register_common_help(parser)
    
    parser.add_argument("--main-dir", help=_("args.main_dir"), type=type_path_dir)
    parser.add_argument("--work-dir", help=_("args.work_dir"), type=type_path_dir)
    parser.add_argument("--timeout", help=_("args.timeout"), type=float)

    output_choices = get_outputs()
    output_default = "human-color" if sys.stdout.isatty() else "human"
    output_arg = parser.add_argument("--output", help=_("args.output"), dest="out_kind", choices=output_choices, default=output_default)
    for choice in output_choices:
        add_completion(output_arg, choice, _(f"args.output.comp.{choice}"))

    parser.add_argument("-v", dest="verbose", help=_("args.verbose"), action="count", default=0)
    register_subcommands(parser.add_subparsers(title="subcommands", dest="subcommand"))

    return parser


def register_subcommands(subparsers) -> None:
    register_search_arguments(subparsers.add_parser("search", help=_("args.search"), add_help=False))
    register_start_arguments(subparsers.add_parser("start", help=_("args.start"), add_help=False))
    register_login_arguments(subparsers.add_parser("login", help=_("args.login"), add_help=False))
    register_logout_arguments(subparsers.add_parser("logout", help=_("args.logout"), add_help=False))
    register_show_arguments(subparsers.add_parser("show", help=_("args.show"), add_help=False))


def register_search_arguments(parser: ArgumentParser) -> None:
    
    parser.description = _("args.search._")
    register_common_help(parser)

    kind_choices = get_search_kinds()
    kind_arg = parser.add_argument("-k", "--kind", help=_("args.search.kind"), default="mojang", choices=kind_choices)
    for choice in kind_choices:
        add_completion(kind_arg, choice, _(f"args.search.kind.comp.{choice}"))

    input_arg = parser.add_argument("input", nargs="?", help=_("args.search.input"))
    add_completion(input_arg, "release", _("args.search.input.comp.release"))
    add_completion(input_arg, "snapshot", _("args.search.input.comp.snapshot"))

def register_start_arguments(parser: ArgumentParser) -> None:
    
    register_common_help(parser)
    parser.add_argument("--dry", help=_("args.start.dry"), action="store_true")
    parser.add_argument("--disable-mp", help=_("args.start.disable_multiplayer"), action="store_true")
    parser.add_argument("--disable-chat", help=_("args.start.disable_chat"), action="store_true")
    parser.add_argument("--demo", help=_("args.start.demo"), action="store_true")
    parser.add_argument("--resolution", help=_("args.start.resolution"), type=type_resolution)
    parser.add_argument("--jvm", help=_("args.start.jvm"), type=type_path)
    parser.add_argument("--jvm-args", help=_("args.start.jvm_args"), metavar="ARGS")
    parser.add_argument("--no-fix", help=_("args.start.no_fix"), action="store_true")
    parser.add_argument("--fabric-prefix", help=_("args.start.fabric_prefix"), default="fabric", metavar="PREFIX")
    parser.add_argument("--quilt-prefix", help=_("args.start.quilt_prefix"), default="quilt", metavar="PREFIX")
    parser.add_argument("--legacyfabric-prefix", help=_("args.start.legacyfabric_prefix"), default="legacyfabric", metavar="PREFIX")
    parser.add_argument("--babric-prefix", help=_("args.start.babric_prefix"), default="babric", metavar="PREFIX")
    parser.add_argument("--forge-prefix", help=_("args.start.forge_prefix"), default="forge", metavar="PREFIX")
    parser.add_argument("--neoforge-prefix", help=_("args.start.neoforge_prefix"), default="neoforge", metavar="PREFIX")
    parser.add_argument("--lwjgl", help=_("args.start.lwjgl"))
    parser.add_argument("--exclude-lib", help=_("args.start.exclude_lib"), action="append", metavar="SPEC", type=LibrarySpecifierFilter.from_str)
    parser.add_argument("--include-bin", help=_("args.start.include_bin"), action="append", metavar="PATH", type=type_path)
    parser.add_argument("--auth-anonymize", help=_("args.start.auth_anonymize"), action="store_true")
    register_common_auth_service(parser)
    parser.add_argument("-t", "--temp-login", help=_("args.start.temp_login"), action="store_true")
    parser.add_argument("-l", "--login", help=_("args.start.login"), type=type_email_or_username)
    parser.add_argument("-u", "--username", help=_("args.start.username"), metavar="NAME")
    parser.add_argument("-i", "--uuid", help=_("args.start.uuid"))
    parser.add_argument("-s", "--server", help=_("args.start.server"), type=type_host)
    parser.add_argument("-p", "--server-port", help=_("args.start.server_port"), metavar="PORT")

    version_arg = parser.add_argument("version", nargs="?", default="release", help=_("args.start.version", formats=", ".join(map(lambda s: _(f"args.start.version.{s}"), ("standard", "fabric", "quilt", "legacyfabric", "babric", "forge", "neoforge")))))
    for standard in ("release", "snapshot"):
        add_completion(version_arg, standard, _(f"args.start.version.comp.{standard}"))
    for loader in ("fabric", "quilt", "legacyfabric", "babric", "forge", "neoforge"):
        add_completion(version_arg, f"{loader}:", _(f"args.start.version.comp.{loader}"))


def register_login_arguments(parser: ArgumentParser) -> None:
    register_common_help(parser)
    register_common_auth_service(parser)
    parser.add_argument("email_or_username", type=type_email_or_username)


def register_logout_arguments(parser: ArgumentParser) -> None:
    register_common_help(parser)
    register_common_auth_service(parser)
    parser.add_argument("email_or_username", type=type_email_or_username)


def register_show_arguments(parser: ArgumentParser) -> None:
    register_common_help(parser)
    subparsers = parser.add_subparsers(title="subcommands", dest="show_subcommand")
    subparsers.required = True
    subparsers.add_parser("about", help=_("args.show.about"), add_help=False)
    subparsers.add_parser("auth", help=_("args.show.auth"), add_help=False)
    subparsers.add_parser("lang", help=_("args.show.lang"), add_help=False)
    register_show_completion_arguments(subparsers.add_parser("completion", help=_("args.show.completion"), add_help=False))


def register_show_completion_arguments(parser: ArgumentParser) -> None:

    parser.description = _("args.show.completion._")
    register_common_help(parser)

    # The shell argument is only required if the shell cannot be determined.
    shell_choices = get_completion_shells()
    shell_arg = parser.add_argument("shell", choices=shell_choices, help=_("args.show.completion.shell"))

    for choice in shell_choices:
        add_completion(shell_arg, choice, _(f"args.show.completion.shell.comp.{choice}"))


def new_help_formatter_class(max_help_position: int) -> Type[HelpFormatter]:

    class CustomHelpFormatter(RawDescriptionHelpFormatter):
        def __init__(self, prog):
            super().__init__(prog, max_help_position=max_help_position)

    return CustomHelpFormatter


def get_outputs() -> List[str]:
    return ["human-color", "human", "machine"]

def get_search_kinds() -> List[str]:
    return ["mojang", "local", "forge", "fabric", "quilt", "legacyfabric", "babric"]

def get_auth_services() -> List[str]:
    return ["microsoft", "yggdrasil"]

def get_completion_shells() -> List[str]:
    return ["bash", "zsh"]


def type_path(s: str) -> Path:
    return Path(s)

def type_path_dir(s: str) -> Path:
    return Path(s)

def type_resolution(s: str) -> Tuple[int, int]:
    parts = s.split("x")
    if len(parts) == 2:
        return (int(parts[0]), int(parts[1]))
    else:
        raise ArgumentTypeError(_("args.start.resolution.invalid", given=s))

def type_email_or_username(s: str) -> str:
    return s

def type_host(s: str) -> str:
    return s


def add_completion(action: Action, name: str, description: str):
    """Add a completion for this action, this is used by 'complete' module.
    """
    if not hasattr(action, "_pmc_completions"):
        action._pmc_completions = {} # type: ignore
    action._pmc_completions[name] = description # type: ignore

def get_completions(action: Action) -> Dict[str, str]:
    return getattr(action, "_pmc_completions", {})
