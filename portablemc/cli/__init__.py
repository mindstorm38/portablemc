"""Entry module for PortableMC Command Line Interface. 

This module implements all (sub)commands of the CLI, the arguments parser, language and 
other utilities are defined in child modules.

**Note that this module, even when no *underscore* "_" is used, should not be considered
as part of the public API.**
"""

from subprocess import Popen
from pathlib import Path
import socket
import sys
import io

from .parse import register_arguments, RootNs, SearchNs, StartNs, LoginNs, LogoutNs, AuthBaseNs, ShowCompletionNs

from .util import format_locale_date, format_time, format_number, anonymize_email
from .output import Output, HumanOutput, MachineOutput, OutputTable
from .lang import get as _, lang

from portablemc.util import LibrarySpecifier
from portablemc.http import HttpError
from portablemc.auth import AuthDatabase, AuthSession, MicrosoftAuthSession, \
    YggdrasilAuthSession, AuthError

from portablemc.standard import Context, Version, VersionManifest, SimpleWatcher, \
    DownloadError, DownloadStartEvent, DownloadProgressEvent, DownloadCompleteEvent, \
    VersionNotFoundError, TooMuchParentsError, FeaturesEvent, JarNotFoundError, \
    JvmNotFoundError, LibraryNotFoundError, \
    VersionLoadingEvent, VersionFetchingEvent, VersionLoadedEvent, \
    JvmLoadingEvent, JvmLoadedEvent, JarFoundEvent, \
    AssetsResolveEvent, LibrariesResolvingEvent, LibrariesResolvedEvent, \
    LoggerFoundEvent, \
    StreamRunner, XmlStreamEvent

from portablemc.fabric import FabricVersion, FabricResolveEvent
from portablemc.forge import ForgeVersion, ForgeResolveEvent, ForgePostProcessingEvent, \
    ForgePostProcessedEvent, ForgeInstallError, _NeoForgeVersion

from typing import cast, Optional, List, Union, Dict, Callable, Any, Tuple


EXIT_OK = 0
EXIT_FAILURE = 1

AUTH_DATABASE_FILE_NAME = "portablemc_auth.json"
MANIFEST_CACHE_FILE_NAME = "portablemc_version_manifest.json"
MICROSOFT_AZURE_APP_ID = "708e91b5-99f8-4a1d-80ec-e746cbb24771"

DEFAULT_JVM_ARGS = [
    "-Xmx2G",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+UseG1GC",
    "-XX:G1NewSizePercent=20",
    "-XX:G1ReservePercent=20",
    "-XX:MaxGCPauseMillis=50",
    "-XX:G1HeapRegionSize=32M"
]

CommandHandler = Callable[[Any], Any]
CommandTree = Dict[str, Union[CommandHandler, "CommandTree"]]


def main(args: Optional[List[str]] = None):
    """Main entry point of the CLI. This function parses the input arguments and try to
    find a command handler to dispatch to. These command handlers are specified by the
    `get_command_handlers` function.
    """

    # Force stdout/stderr to use UTF-8 encoding, this reconfigure method 
    # is available for Python 3.7 and onward.
    if isinstance(sys.stdout, io.TextIOWrapper):
        sys.stdout.reconfigure(encoding='utf-8')
    if isinstance(sys.stderr, io.TextIOWrapper):
        sys.stderr.reconfigure(encoding='utf-8')

    parser = register_arguments()
    ns: RootNs = cast(RootNs, parser.parse_args(args or sys.argv[1:]))

    # Setup common objects in the namespace.
    ns.parser = parser
    ns.out = get_output(ns.out_kind)
    ns.context = Context(ns.main_dir, ns.work_dir)
    ns.version_manifest = VersionManifest(ns.context.work_dir / MANIFEST_CACHE_FILE_NAME)
    ns.auth_database = AuthDatabase(ns.context.work_dir / AUTH_DATABASE_FILE_NAME)
    ns.socket_error_tips = []
    socket.setdefaulttimeout(ns.timeout)

    # Find the command handler and run it.
    command_handlers = get_command_handlers()
    command_attr = "subcommand"
    while True:
        command = getattr(ns, command_attr)
        handler = command_handlers.get(command)
        if handler is None:
            parser.print_help()
            sys.exit(EXIT_FAILURE)
        elif callable(handler):
            cmd(handler, ns)
        elif isinstance(handler, dict):
            command_attr = f"{command}_{command_attr}"
            command_handlers = handler
            continue
        sys.exit(EXIT_OK)


def get_output(kind: str) -> Output:
    """Internal function that construct the output depending on its kind.
    The kind is constrained by choices set to the arguments parser.
    """

    if kind == "human-color":
        return HumanOutput(True)
    elif kind == "human":
        return HumanOutput(False)
    elif kind == "machine":
        return MachineOutput()
    else:
        raise ValueError()


def get_command_handlers() -> CommandTree:
    """Internal function returns the tree of command handlers for each subcommand
    of the CLI argument parser.
    """

    return {
        "search": cmd_search,
        "start": cmd_start,
        "login": cmd_login,
        "logout": cmd_logout,
        "show": {
            "about": cmd_show_about,
            "auth": cmd_show_auth,
            "lang": cmd_show_lang,
            "completion": cmd_show_completion,
        },
    }


def cmd(handler: CommandHandler, ns: RootNs):
    """Generic command handler that launch the given handler with the given namespace,
    it handles error in order to pretty print them.
    """
    
    try:
        handler(ns)
        sys.exit(EXIT_OK)
    
    except ValueError as error:
        if len(error.args):
            for i, arg in enumerate(error.args):
                ns.out.task("FAILED" if i == 0 else None, "echo", echo=arg)
                ns.out.finish()
        else:
            ns.out.task("FAILED", "echo", echo="programming error")
            ns.out.finish()
        
        if ns.verbose >= 1:
            import traceback
            traceback.print_exc()
        else:
            ns.out.task("INFO", "suggest_verbose")
            ns.out.finish()
    
    except KeyboardInterrupt:
        ns.out.finish()
        ns.out.task("HALT", "keyboard_interrupt")
        ns.out.finish()
    
    except (OSError, HttpError) as error:

        from urllib.error import URLError
        from ssl import SSLCertVerificationError

        key = "error.os"

        if isinstance(error, HttpError):
            if error.res.status == 0:
                # Status 0 means that network error happened, just forward this error.
                error = error.reason
            else:
                # Unhandled HTTP error code.
                # Here we don't redefine "error" so this will skip following conditions.
                key = "error.http"

        # We are only interested in the reason, URLError is just a wrapper.
        if isinstance(error, URLError):
            error = error.reason

        # More precise errors.
        if isinstance(error, SSLCertVerificationError):
            key = "error.cert"
        elif isinstance(error, (socket.gaierror, socket.timeout)):
            key = "error.socket"
        
        ns.out.task("FAILED", key)
        ns.out.finish()

        if ns.verbose >= 1:
            import traceback
            traceback.print_exc()
        else:
            ns.out.task(None, "echo", echo=repr(error))
            ns.out.finish()
            ns.out.task("INFO", "suggest_verbose")
            ns.out.finish()

        if key == "error.socket":
            for error_tip in ns.socket_error_tips:
                ns.out.task("INFO", f"error.socket.tip.{error_tip}")
                ns.out.finish()
    
    sys.exit(EXIT_FAILURE)


def cmd_search(ns: SearchNs):
    table = ns.out.table()
    cmd_search_handler(ns, ns.kind, table)
    table.print()
    sys.exit(EXIT_OK)

def cmd_search_handler(ns: SearchNs, kind: str, table: OutputTable):
    """Internal function that handles searching a particular kind of search.
    The value of "kind" is constrained by choices in the argument parser.
    """

    search = ns.input

    if kind == "mojang":
        
        table.add(
            _("search.type"),
            _("search.name"),
            _("search.release_date"),
            _("search.flags"))
        table.separator()

        ns.socket_error_tips.append("version_manifest")
 
        if search is not None:
            search, alias = ns.version_manifest.filter_latest(search)
        else:
            alias = False

        for version_data in ns.version_manifest.all_versions():
            version_id = version_data["id"]
            if search is None or (alias and search == version_id) or (not alias and search in version_id):
                version = ns.context.get_version(version_id)
                table.add(
                    version_data["type"], 
                    version_id, 
                    format_locale_date(version_data["releaseTime"]),
                    _("search.flags.local") if version.metadata_exists() else "")
    
    elif kind == "local":

        table.add(
            _("search.name"),
            _("search.last_modified"))
        table.separator()

        search = ns.input
        for version in ns.context.list_versions():
            if search is None or search in version.id:
                table.add(version.id, format_locale_date(version.metadata_file().stat().st_mtime))
    
    elif kind == "forge":

        from ..forge import request_promo_versions
        
        table.add(_("search.name"), _("search.loader_version"))
        table.separator()

        if search is not None:
            ns.socket_error_tips.append("version_manifest")
            search = ns.version_manifest.filter_latest(search)[0]

        for alias, version in request_promo_versions().items():
            if search is None or search in alias:
                table.add(alias, version)

    elif kind in ("fabric", "quilt", "legacyfabric", "babric"):

        from ..fabric import FABRIC_API, QUILT_API, LEGACYFABRIC_API, BABRIC_API

        table.add(_("search.loader_version"), _("search.flags"))
        table.separator()

        if kind == "fabric":
            api = FABRIC_API
        elif kind == "quilt":
            api = QUILT_API
        elif kind == "legacyfabric":
            api = LEGACYFABRIC_API
        elif kind == "babric":
            api = BABRIC_API
        
        for loader in api._request_loaders():
            if search is None or search in loader.version:
                table.add(loader.version, _("search.flags.stable") if loader.stable else "")

    else:
        raise ValueError()


def cmd_start(ns: StartNs):

    version_parts = ns.version.split(":")
    
    # If no split, the kind of version is "standard": parts have at least 2 elements.
    if len(version_parts) == 1:
        version_parts = ["standard", version_parts[0]]

    # No handler means that the format is invalid.
    version = cmd_start_handler(ns, version_parts[0], version_parts[1:])
    if version is None:
        format_key = f"args.start.version.{version_parts[0]}"
        if format_key not in lang:
            ns.out.task("FAILED", "start.version.invalid_id_unknown_kind", kind=version_parts[0])
        else:
            ns.out.task("FAILED", "start.version.invalid_id", expected=_(format_key))
        ns.out.finish()
        sys.exit(EXIT_FAILURE)

    version.manifest = ns.version_manifest
    version.disable_multiplayer = ns.disable_mp
    version.disable_chat = ns.disable_chat
    version.demo = ns.demo
    version.resolution = ns.resolution
    version.jvm_path = ns.jvm

    if ns.server is not None:
        version.set_quick_play_multiplayer(ns.server, ns.server_port or 25565)

    if ns.no_fix:
        version.fixes.clear()
    
    if ns.lwjgl is not None:
        version.fixes[Version.FIX_LWJGL] = ns.lwjgl

    if ns.login is not None:
        version.auth_session = prompt_authenticate(ns, ns.login, not ns.temp_login, ns.auth_anonymize)
        if version.auth_session is None:
            sys.exit(EXIT_FAILURE)
    else:
        version.set_auth_offline(ns.username, ns.uuid)

    # Excluded libraries
    if ns.exclude_lib is not None:

        exclude_filters = ns.exclude_lib
        def filter_libraries(libs: Dict[LibrarySpecifier, Any]) -> None:
            # Here the complexity is terrible, but I guess it's acceptable?
            to_del = []
            unused_filters = set(exclude_filters)
            for spec in libs.keys():
                for spec_filter in exclude_filters:
                    if spec_filter.matches(spec):
                        unused_filters.remove(spec_filter)
                        to_del.append(spec)
                        break
            # Finally delete selected specifiers
            for spec in to_del:
                del libs[spec]
                if ns.verbose >= 1:
                    ns.out.task("INFO", "start.libraries.excluded", spec=str(spec))
                    ns.out.finish()
            # Inform the user of unused filters
            for unused_filter in unused_filters:
                ns.out.task("WARN", "start.libraries.unused_filter", filter=str(unused_filter))
                ns.out.finish()
        
        version.libraries_filters.append(filter_libraries)

    try:

        env = version.install(watcher=StartWatcher(ns))

        if ns.verbose >= 1:
            for fix, fix_value in env.fixes.items():
                ns.out.task("INFO", f"start.fix.{fix}", value=fix_value)
                ns.out.finish()

        # Included binaries
        if ns.include_bin is not None:
            for bin_path in ns.include_bin:
                if not bin_path.is_file():
                    ns.out.task("FAILED", "start.additional_binary_not_found", path=bin_path)
                    ns.out.finish()
                    sys.exit(EXIT_FAILURE)
                env.native_libs.append(bin_path)
            
        # Extend JVM arguments with given arguments, or defaults
        if ns.jvm_args is None:
            env.jvm_args.extend(DEFAULT_JVM_ARGS)
        elif len(ns.jvm_args):
            env.jvm_args.extend(ns.jvm_args.split())

        # This CliRunner will abort running if in dry mode.
        env.run(CliRunner(ns))
        sys.exit(EXIT_OK)
    
    except VersionNotFoundError as error:
        ns.out.task("FAILED", "start.version.not_found", version=error.version)
        ns.out.finish()
    
    except TooMuchParentsError as error:
        ns.out.task("FAILED", "start.version.too_much_parents")
        ns.out.finish()
        ns.out.task(None, "echo", echo=", ".join(error.versions))
        ns.out.finish()

    except JarNotFoundError as error:
        ns.out.task("FAILED", "start.jar.not_found")
        ns.out.finish()

    except JvmNotFoundError as error:
        ns.out.task("FAILED", f"start.jvm.not_found_error.{error.code}")
        ns.out.finish()
    
    except LibraryNotFoundError as error:
        ns.out.task("FAILED", f"start.libraries.not_found_error", spec=str(error.lib))
        ns.out.finish()
    
    except ForgeInstallError as error:
        ns.out.task("FAILED", f"start.forge.install_error.{error.code}")
        ns.out.finish()

    except DownloadError as error:
        ns.out.task("FAILED", None)
        ns.out.finish()
        for entry, code, _origin in error.errors:
            ns.out.task(None, "download.error", name=entry.url, message=_(f"download.error.{code}"))
            ns.out.finish()
    
    sys.exit(EXIT_FAILURE)

def cmd_start_handler(ns: StartNs, kind: str, parts: List[str]) -> Optional[Version]:
    """This function handles particular kind of versions. If this function successfully
    decodes, the corresponding version should be returned. The global version's format 
    being parsed is <kind>[:<part>..].

    The parts list contains at least one element.

    This function returns false if parsing fail, in such case the expected format is
    printed out to the user on output (language key: "args.start.version.<kind>").
    """

    version = parts[0] or "release"
    ns.socket_error_tips.append("version_manifest")
    
    if ns.verbose >= 1:
        ns.out.task("INFO", "start.global_version", kind=kind, version=version, remaining=" ".join(parts[1:]))
        ns.out.finish()

    if kind == "standard":
        if len(parts) != 1:
            return None
        return Version(version, context=ns.context)
    
    elif kind in ("fabric", "quilt", "legacyfabric", "babric"):

        if len(parts) > 2:
            return None
        
        # Legacy fabric has a special case because it will never be supported for 
        # versions past 1.13.2, it is not made for latest release version.
        if version == "release":
            if kind == "legacyfabric":
                version = "1.13.2"
            elif kind == "babric":
                version = "b1.7.3"
        
        if kind == "fabric":
            constructor = FabricVersion.with_fabric
            prefix = ns.fabric_prefix
        elif kind == "quilt":
            constructor = FabricVersion.with_quilt
            prefix = ns.quilt_prefix
        elif kind == "legacyfabric":
            constructor = FabricVersion._with_legacyfabric
            prefix = ns.legacyfabric_prefix
        elif kind == "babric":
            constructor = FabricVersion._with_babric
            prefix = "babric"
        
        if len(parts) != 2:
            ns.socket_error_tips.append(f"{kind}_loader_version")

        return constructor(version, parts[1] if len(parts) == 2 else None, context=ns.context, prefix=prefix)
    
    elif kind in ("forge", "neoforge"):
        if len(parts) != 1:
            return None
        constructor = ForgeVersion if kind == "forge" else _NeoForgeVersion
        prefix = ns.forge_prefix if kind == "forge" else ns.neoforge_prefix
        return constructor(version, context=ns.context, prefix=prefix)
    
    else:
        return None


def cmd_login(ns: LoginNs):
    session = prompt_authenticate(ns, ns.email_or_username, True)
    if session is not None:
        ns.out.task("INFO", "login.tip.remember_start_login", email=ns.email_or_username)
        ns.out.finish()
    sys.exit(EXIT_FAILURE if session is None else EXIT_OK)


def cmd_logout(ns: LogoutNs):

    session_class = {
        "microsoft": MicrosoftAuthSession,
        "yggdrasil": YggdrasilAuthSession,
    }[ns.auth_service]

    ns.out.task("", f"logout.{ns.auth_service}.pending", email=ns.email_or_username)
    ns.auth_database.load()
    session = ns.auth_database.remove(ns.email_or_username, session_class)
    if session is not None:
        session.invalidate()
        ns.auth_database.save()
        ns.out.task("OK", "logout.success", email=ns.email_or_username)
        ns.out.finish()
        sys.exit(EXIT_OK)
    else:
        ns.out.task("FAILED", "logout.unknown_session", email=ns.email_or_username)
        ns.out.finish()
        sys.exit(EXIT_FAILURE)


def cmd_show_about(ns: RootNs):
    
    from .. import LAUNCHER_VERSION, LAUNCHER_AUTHORS, LAUNCHER_URL, LAUNCHER_COPYRIGHT

    print(f"Version: {LAUNCHER_VERSION}")
    print(f"Authors: {', '.join(LAUNCHER_AUTHORS)}")
    print(f"Website: {LAUNCHER_URL}")
    print(f"License: {LAUNCHER_COPYRIGHT}")
    print( "         This program comes with ABSOLUTELY NO WARRANTY. This is free software,")
    print( "         and you are welcome to redistribute it under certain conditions.")
    print( "         See <https://www.gnu.org/licenses/gpl-3.0.html>.")


def cmd_show_auth(ns: RootNs):

    ns.auth_database.load()
    table = ns.out.table()

     # Intentionally not i18n for now because used for debug purpose.
    table.add("Type", "Email", "Username", "UUID")
    table.separator()

    for auth_type, auth_type_sessions in ns.auth_database.sessions.items():
        for email, sess in auth_type_sessions.items():
            table.add(auth_type, email, sess.username, sess.uuid)
    
    table.print()


def cmd_show_lang(ns: RootNs):

    from .lang import lang

    table = ns.out.table()

     # Intentionally not i18n for now because used for debug purpose.
    table.add("Key", "Message")
    table.separator()

    for key, msg in lang.items():
        table.add(key, msg)

    table.print()


def cmd_show_completion(ns: ShowCompletionNs):
    
    from .complete import gen_zsh_completion, gen_bash_completion

    if ns.shell == "zsh":
        content = gen_zsh_completion(ns.parser)
    elif ns.shell == "bash":
        content = gen_bash_completion(ns.parser)
    else:
        raise RuntimeError
    
    print(content, end="")


def prompt_authenticate(ns: AuthBaseNs, email: str, caching: bool, anonymise: bool = False) -> Optional[AuthSession]:
    """Prompt the user to login using the given email (or legacy username) for specific 
    service (Microsoft or Yggdrasil) and return the :class:`AuthSession` if successful, 
    None otherwise. This function handles task printing and all exceptions are caught 
    internally.
    """

    service = ns.auth_service

    session_class = {
        "microsoft": MicrosoftAuthSession,
        "yggdrasil": YggdrasilAuthSession,
    }[service]

    ns.auth_database.load()

    if service == "yggdrasil":
        ns.out.task("WARN", "auth.yggdrasil.deprecated")
        ns.out.finish()

    task_text = f"auth.{service}"
    email_text = anonymize_email(email) if anonymise else email

    ns.out.task("..", task_text, email=email_text)

    session = ns.auth_database.get(email, session_class)
    if session is not None:
        try:
            
            if not session.validate():
                ns.out.task("..", "auth.refreshing")
                session.refresh()
                ns.auth_database.save()
                ns.out.task("OK", "auth.refreshed", email=email_text)
            else:
                ns.out.task("OK", "auth.validated", email=email_text)

            ns.out.finish()
            return session
        
        except AuthError as error:
            ns.out.task("FAILED", None)
            ns.out.task(None, "auth.error", message=str(error))
            ns.out.finish()

    ns.out.task("..", task_text, email=email_text)

    try:

        if service == "microsoft":
            session = prompt_microsoft_authenticate(ns, email)
        else:
            session = prompt_yggdrasil_authenticate(ns, email)
        
    except AuthError as error:
        ns.out.task("FAILED", "auth.error", message=str(error))
        ns.out.finish()
        return None

    if session is None:
        return None
    if caching:
        ns.out.task("..", "auth.caching")
        ns.auth_database.put(email, session)
        ns.auth_database.save()
    
    ns.out.task("OK", "auth.logged_in", email=email_text)
    ns.out.finish()

    return session


def prompt_yggdrasil_authenticate(ns: RootNs, email_or_username: str) -> Optional[YggdrasilAuthSession]:
    ns.out.finish()
    ns.out.task("..", "auth.yggdrasil.enter_password")
    password = ns.out.prompt(password=True)
    if password is None:
        ns.out.task("FAILED", "cancelled")
        ns.out.finish()
        return None
    else:
        return YggdrasilAuthSession.authenticate(ns.auth_database.get_client_id(), email_or_username, password)


def prompt_microsoft_authenticate(ns: AuthBaseNs, email: str) -> Optional[MicrosoftAuthSession]:

    from .. import LAUNCHER_NAME, LAUNCHER_VERSION
    from http.server import HTTPServer, BaseHTTPRequestHandler
    from uuid import uuid4
    import urllib.parse
    import webbrowser

    nonce = uuid4().hex
    app_id = MICROSOFT_AZURE_APP_ID
    redirect_uri = "https://www.theorozier.fr/portablemc/auth"

    def gen_auth_url(state: str) -> str:
        return "https://login.live.com/oauth20_authorize.srf?{}".format(urllib.parse.urlencode({
            "client_id": app_id,
            "redirect_uri": redirect_uri,
            "response_type": "code id_token",
            "scope": "xboxlive.signin offline_access openid email",
            "login_hint": email,
            "nonce": nonce,
            "state": state,
            "prompt": "login",
            "response_mode": "fragment"
        }))
    
    auth_query = None
    
    if not ns.auth_no_browser:

        class AuthServer(HTTPServer):

            def __init__(self):
                super().__init__(("127.0.0.1", 0), RequestHandler)
                self.timeout = 0.5
                self.ms_auth_query: Optional[str] = None

        class RequestHandler(BaseHTTPRequestHandler):

            server_version = f"{LAUNCHER_NAME}/{LAUNCHER_VERSION}"

            def __init__(self, request, client_address: Tuple[str, int], auth_server: AuthServer) -> None:
                super().__init__(request, client_address, auth_server)

            def log_message(self, _format: str, *args: Any):
                return

            def do_GET(self):
                parsed = urllib.parse.urlparse(self.path)
                if parsed.path in ("", "/"):
                    cast(AuthServer, self.server).ms_auth_query = parsed.query
                    self.send_response(200)
                else:
                    self.send_response(404)
                self.send_header("Access-Control-Allow-Origin", "*")
                self.end_headers()
                self.wfile.flush()
    
        # We start be creating the authentication server, at this point we don't start it 
        # but we have allocated a free port.
        with AuthServer() as server:

            # First try opening the authentication page with the local webbrowser
            if webbrowser.open(gen_auth_url(f"port:{server.server_port}")):
                # If successfully opened the browser, we actually start the web server.
                ns.out.task("..", "auth.microsoft.opening_browser_and_listening")
                try:
                    while server.ms_auth_query is None:
                        server.handle_request()
                except KeyboardInterrupt:
                    pass
            
            if server.ms_auth_query is None:
                ns.out.finish()
                ns.out.task("FAILED", "auth.microsoft.failed_to_authenticate")
                ns.out.finish()
                return None

            auth_query = server.ms_auth_query

    # If here we have code or id token none, it means that no web browser has been opened.
    # So we want to print the URL auth URL so the user can try manually.
    if auth_query is None:

        ns.out.task("INFO", "auth.microsoft.no_browser_fallback")
        ns.out.finish()
        ns.out.print(gen_auth_url("") + "\n")

        ns.out.task("..", "auth.microsoft.no_browser_code")
        auth_query = ns.out.prompt()
        if auth_query is None:
            ns.out.finish()
            ns.out.task("FAILED", "auth.microsoft.failed_to_authenticate")
            ns.out.finish()
            return None
        else:
            auth_query = auth_query.strip()

    qs = urllib.parse.parse_qs(auth_query)

    if "code" in qs and "id_token" in qs:

        ns.out.task("..", "auth.microsoft.processing")
        id_token = qs["id_token"][0]
        code = qs["code"][0]

        if not MicrosoftAuthSession.check_token_id(id_token, email, nonce):
            ns.out.finish()
            ns.out.task("FAILED", "auth.microsoft.incoherent_data")
            ns.out.finish()
            return None

        return MicrosoftAuthSession.authenticate(ns.auth_database.get_client_id(), app_id, code, redirect_uri)

    else:
        ns.out.finish()
        ns.out.task("FAILED", "auth.microsoft.failed_to_authenticate")
        ns.out.finish()
        return None

    
class StartWatcher(SimpleWatcher):

    def __init__(self, ns: RootNs) -> None:

        def progress_task(key: str, **kwargs) -> None:
            ns.out.task("..", key, **kwargs)

        def finish_task(key: str, **kwargs) -> None:
            ns.out.task("OK", key, **kwargs)
            ns.out.finish()
        
        def features(e: FeaturesEvent) -> None:
            if ns.verbose >= 1:
                ns.out.task("INFO", "start.features", features=", ".join(e.features))
                ns.out.finish()
        
        def assets_resolve(e: AssetsResolveEvent) -> None:
            if e.count is None:
                ns.out.task("..", "start.assets.resolving", index_version=e.index_version)
            else:
                ns.out.task("OK", "start.assets.resolved", index_version=e.index_version, count=e.count)
                ns.out.finish()

        def libraries_resolved(e: LibrariesResolvedEvent) -> None:
            ns.out.task("OK", "start.libraries.resolved", class_libs_count=e.class_libs_count, native_libs_count=e.native_libs_count)
            ns.out.finish()

        def fabric_resolve(e: FabricResolveEvent) -> None:
            if e.loader_version is None:
                ns.out.task("..", "start.fabric.resolving", api=e.api.name, vanilla_version=e.vanilla_version)
            else:
                ns.out.task("OK", "start.fabric.resolved", api=e.api.name, loader_version=e.loader_version, vanilla_version=e.vanilla_version)
                ns.out.finish()
        
        def forge_resolve(e: ForgeResolveEvent) -> None:
            if e.alias:
                ns.out.task("..", "start.forge.resolving", api=e._api, version=e.forge_version)
            else:
                ns.out.task("OK", "start.forge.resolved", api=e._api, version=e.forge_version)
                ns.out.finish()

        super().__init__({
            VersionLoadingEvent: lambda e: progress_task("start.version.loading", version=e.version),
            VersionFetchingEvent: lambda e: progress_task("start.version.fetching", version=e.version),
            VersionLoadedEvent: lambda e: finish_task("start.version.loaded.fetched" if e.fetched else "start.version.loaded", version=e.version),
            FeaturesEvent: features,
            JvmLoadingEvent: lambda e: progress_task("start.jvm.loading"),
            JvmLoadedEvent: lambda e: finish_task(f"start.jvm.loaded.{e.kind}", version=e.version or ""),
            JarFoundEvent: lambda e: finish_task("start.jar.found"),
            AssetsResolveEvent: assets_resolve,
            LibrariesResolvingEvent: lambda e: progress_task("start.libraries.resolving"),
            LibrariesResolvedEvent: libraries_resolved,
            LoggerFoundEvent: lambda e: finish_task("start.logger.found", version=e.version),
            FabricResolveEvent: fabric_resolve,
            ForgeResolveEvent: forge_resolve,
            ForgePostProcessingEvent: lambda e: progress_task("start.forge.post_processing", task=e.task),
            ForgePostProcessedEvent: lambda e: finish_task("start.forge.post_processed"),
            DownloadStartEvent: self.download_start,
            DownloadProgressEvent: self.download_progress,
            DownloadCompleteEvent: self.download_complete,
        })
            
        self.ns = ns
        self.entries_count: int
        self.total_size: int
        self.speeds: List[float]
        self.sizes: List[int]
        self.size = 0

    def download_start(self, e: DownloadStartEvent):

        if self.ns.verbose:
            self.ns.out.task("INFO", "download.threads_count", count=e.threads_count)
            self.ns.out.finish()

        self.entries_count = e.entries_count
        self.total_size = e.size
        self.speeds = [0.0] * e.threads_count
        self.sizes = [0] * e.threads_count
        self.size = 0
        self.ns.out.task("..", "download.start")

    def download_progress(self, e: DownloadProgressEvent) -> None:

        self.speeds[e.thread_id] = e.speed
        self.sizes[e.thread_id] = e.size

        speed = sum(self.speeds)
        total_count = str(self.entries_count)
        count = f"{e.count:{len(total_count)}}"
        
        self.ns.out.task("..", "download.progress", 
            count=count,
            total_count=total_count,
            size=f"{format_number(self.size + sum(self.sizes))}B",
            speed=f"{format_number(speed)}B/s")

        if e.done:
            self.size += e.size

    def download_complete(self, e: DownloadCompleteEvent) -> None:
        self.ns.out.task("OK", None)
        self.ns.out.finish()


class CliRunner(StreamRunner):

    def __init__(self, ns: StartNs) -> None:
        super().__init__()
        self.ns = ns

    def process_create(self, args: List[str], work_dir: Path) -> Optional[Popen]:
        
        if not self.ns.dry or self.ns.verbose >= 2:
            self.ns.out.print("\n")

        if self.ns.verbose >= 2:
            self.ns.out.print(" ".join(args) + "\n")

        if self.ns.dry:
            return None
        
        return super().process_create(args, work_dir)

    def process_stream_event(self, event: Any) -> None:

        out = self.ns.out

        if isinstance(event, XmlStreamEvent):
            time = format_time(event.time)
            out.print(f"[{time}] [{event.thread}] [{event.level}] {event.message}\n")
            if event.throwable is not None:
                out.print(f"{event.throwable.rstrip()}\n")
        else:
            out.print(str(event))
