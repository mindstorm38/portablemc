"""Main 
"""

from urllib.error import URLError
from pathlib import Path
import socket
import ssl
import sys

from .parse import register_arguments, RootNs, SearchNs, StartNs, LoginNs, LogoutNs
from .util import format_locale_date, format_number, anonymize_email
from .output import Output, OutputTable
from .lang import get as _

from ..download import DownloadStartEvent, DownloadProgressEvent, DownloadCompleteEvent, DownloadError
from ..auth import AuthDatabase, AuthSession, MicrosoftAuthSession, YggdrasilAuthSession, \
    OfflineAuthSession, AuthError
from ..task import Watcher

from ..vanilla import make_vanilla_sequence, Context, VersionManifest, \
    VersionResolveEvent, VersionNotFoundError, TooMuchParentsError, \
    JarFoundEvent, JarNotFoundError, \
    AssetsResolveEvent, \
    LibraryResolveEvent, \
    LoggerFoundEvent, \
    VersionJvm, JvmResolveEvent, JvmNotFoundError, \
    VersionArgsOptions, VersionArgs

from typing import cast, Optional, List, Union, Dict, Callable, Any, Tuple


EXIT_OK = 0
EXIT_FAILURE = 1

AUTH_DATABASE_FILE_NAME = "portablemc_auth.json"
MANIFEST_CACHE_FILE_NAME = "portablemc_version_manifest.json"
MICROSOFT_AZURE_APP_ID = "708e91b5-99f8-4a1d-80ec-e746cbb24771"

CommandHandler = Callable[[Any], Any]
CommandTree = Dict[str, Union[CommandHandler, "CommandTree"]]


def main(args: Optional[List[str]] = None):
    """Main entry point of the CLI. This function parses the input arguments and try to
    find a command handler to dispatch to. These command handlers are specified by the
    `get_command_handlers` function.
    """

    parser = register_arguments()
    ns: RootNs = cast(RootNs, parser.parse_args(args or sys.argv[1:]))

    # Setup common objects in the namespace.
    ns.context = Context(ns.main_dir, ns.work_dir)
    ns.version_manifest = VersionManifest(ns.context.work_dir / MANIFEST_CACHE_FILE_NAME)
    ns.auth_database = AuthDatabase(ns.context.work_dir / AUTH_DATABASE_FILE_NAME)
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



def get_command_handlers() -> CommandTree:
    """This internal function returns the tree of command handlers for each subcommand
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
        },
        # "addon": {
        #     "list": cmd_addon_list,
        #     "show": cmd_addon_show
        # }
    }


def cmd(handler: CommandHandler, ns: RootNs):
    """Generic command handler that launch the given handler with the given namespace,
    it handles error in order to pretty print them.
    """
    
    try:
        handler(ns)
        sys.exit(EXIT_OK)
    
    except ValueError as error:
        ns.out.task("FAILED", None)
        ns.out.finish()
        for arg in error.args:
            ns.out.task(None, "echo", echo=arg)
            ns.out.finish()
    
    except KeyboardInterrupt:
        ns.out.finish()
        ns.out.task("HALT", "error.keyboard_interrupt")
        ns.out.finish()
    
    except OSError as error:

        key = "error.os"

        if isinstance(error, URLError) and isinstance(error.reason, ssl.SSLCertVerificationError):
            key = "error.cert"
        elif isinstance(error, (URLError, socket.gaierror, socket.timeout)):
            key = "error.socket"
        
        ns.out.task("FAILED", None)
        ns.out.finish()
        ns.out.task(None, key)
        ns.out.finish()

        import traceback
        traceback.print_exc()

    except DownloadError as error:
        ns.out.task("FAILED", None)
        ns.out.finish()
        for entry, code in error.errors:
            ns.out.task(None, "download.error", name=entry.name, message=_(f"download.error.{code}"))
            ns.out.finish()
    
    sys.exit(EXIT_FAILURE)


def cmd_search(ns: SearchNs):


    def cmd_search_manifest(search: Optional[str], table: OutputTable):
        
        table.add(
            _("search.type"),
            _("search.name"),
            _("search.release_date"),
            _("search.flags"))
        table.separator()

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

    def cmd_search_local(search: Optional[str], table: OutputTable):

        table.add(
            _("search.name"),
            _("search.last_modified"))
        table.separator()

        for version in ns.context.list_versions():
            table.add(version.id, format_locale_date(version.metadata_file().stat().st_mtime))

    search_handler = {
        "manifest": cmd_search_manifest,
        "local": cmd_search_local
    }[ns.kind]

    table = ns.out.table()
    search_handler(ns.input, table)

    table.print()
    sys.exit(EXIT_OK)


def cmd_start(ns: StartNs):

    version_raw = ns.version.split(":", maxsplit=1)
    
    if len(version_raw) == 2:
        version_kind, version_id = version_raw
    else:
        version_kind = None
        version_id = version_raw[0]
    
    version_id, _alias = ns.version_manifest.filter_latest(version_id)
    
    seq = make_vanilla_sequence(version_id, 
            context=ns.context, 
            version_manifest=ns.version_manifest)
    
    # Various options for ArgsTask in order to setup the arguments to start the game.
    args_opts = VersionArgsOptions()
    args_opts.disable_multiplayer = ns.disable_mp
    args_opts.disable_chat = ns.disable_chat
    args_opts.demo = ns.demo
    args_opts.server_address = ns.server
    args_opts.server_port = ns.server_port
    args_opts.fix_legacy = not ns.no_legacy_fix
    args_opts.resolution = ns.resolution

    if ns.login is not None:
        args_opts.auth_session = prompt_authenticate(ns, ns.login, not ns.temp_login, ns.auth_service, ns.auth_anonymize)
        if args_opts.auth_session is None:
            sys.exit(EXIT_FAILURE)
    else:
        args_opts.auth_session = OfflineAuthSession(ns.username, ns.uuid)

    seq.state.insert(args_opts)

    # If a manual JVM is specified, we set the JVM state so that JvmTask won't run.
    if ns.jvm is not None:
        seq.state.insert(VersionJvm(Path(ns.jvm), None))

    # Add watchers of the installation.
    seq.add_watcher(StartWatcher(ns.out))
    seq.add_watcher(DownloadWatcher(ns.out))

    try:
        
        seq.execute()

        # Take compute arguments.
        args1 = seq.state[VersionArgs]

        sys.exit(EXIT_OK)

    except VersionNotFoundError as error:
        ns.out.task("FAILED", "start.version.not_found", version=error.version.id)
        ns.out.finish()
    
    except TooMuchParentsError as error:
        ns.out.task("FAILED", "start.version.too_much_parents")
        ns.out.finish()
        ns.out.task(None, "echo", echo=", ".join(map(lambda v: v.id, error.versions)))
        ns.out.finish()

    except JarNotFoundError as error:
        ns.out.task("FAILED", "start.jar.not_found")
        ns.out.finish()

    except JvmNotFoundError as error:
        ns.out.task("FAILED", f"start.jvm.not_found.{error.code}")
        ns.out.finish()
    
    sys.exit(EXIT_FAILURE)

    
class StartWatcher(Watcher):

    def __init__(self, out: Output) -> None:
        self.out = out
    
    def on_event(self, event: Any) -> None:
        
        if isinstance(event, VersionResolveEvent):
            if event.done:
                self.out.task("OK", "start.version.resolved", version=event.version_id)
                self.out.finish()
            else:
                self.out.task("..", "start.version.resolving", version=event.version_id)
        
        elif isinstance(event, JarFoundEvent):
            self.out.task("OK", "start.jar.found", version=event.version_id)
            self.out.finish()
        
        elif isinstance(event, AssetsResolveEvent):
            if event.count is None:
                self.out.task("..", "start.assets.resolving", index_version=event.index_version)
            else:
                self.out.task("OK", "start.assets.resolved", index_version=event.index_version, count=event.count)
                self.out.finish()
        
        elif isinstance(event, LibraryResolveEvent):
            if event.count is None:
                self.out.task("..", "start.libraries.resolving")
            else:
                self.out.task("OK", "start.libraries.resolved", count=event.count)
                self.out.finish()

        elif isinstance(event, LoggerFoundEvent):
            self.out.task("OK", "start.logger.found", version=event.version)
        
        elif isinstance(event, JvmResolveEvent):
            if event.count is None:
                self.out.task("..", "start.jvm.resolving", version=event.version or _("start.jvm.unknown_version"))
            else:
                self.out.task("OK", "start.jvm.resolved", version=event.version or _("start.jvm.unknown_version"), count=event.count)
                self.out.finish()



class DownloadWatcher(Watcher):
    """A watcher for pretty printing download task.
    """

    def __init__(self, out: Output) -> None:

        self.out = out

        self.entries_count: int
        self.total_size: int
        self.size: int
        self.speeds: List[float]
    
    def on_event(self, event: Any) -> None:

        if isinstance(event, DownloadStartEvent):
            self.entries_count = event.entries_count
            self.total_size = event.size
            self.size = 0
            self.speeds = [0.0] * event.threads_count
            self.out.task("..", "download.start")

        elif isinstance(event, DownloadProgressEvent):
            self.speeds[event.thread_id] = event.speed
            speed = sum(self.speeds)
            self.size += event.size
            self.out.task("..", "download.progress", 
                speed=f"{format_number(speed)}o/s",
                count=event.count,
                total_count=self.entries_count,
                size=f"{format_number(self.size)}o")
            
        elif isinstance(event, DownloadCompleteEvent):
            self.out.task("OK", None)
            self.out.finish()


def cmd_login(ns: LoginNs):
    session = prompt_authenticate(ns, ns.email_or_username, True, ns.auth_service)
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


def prompt_authenticate(ns: RootNs, email: str, caching: bool, service: str, anonymise: bool = False) -> Optional[AuthSession]:
    """Prompt the user to login using the given email (or legacy username) for specific 
    service (Microsoft or Yggdrasil) and return the :class:`AuthSession` if successful, 
    None otherwise. This function handles task printing and all exceptions are caught 
    internally.
    """

    session_class = {
        "microsoft": MicrosoftAuthSession,
        "yggdrasil": YggdrasilAuthSession,
    }[service]

    ns.auth_database.load()

    task_text = f"auth.{service}"
    email_text = anonymize_email(email) if anonymise else email

    ns.out.task("..", task_text, email=email_text)

    session = ns.auth_database.get(email, session_class)
    if session is not None:
        try:
            
            if not session.validate():
                ns.out.task(None, "auth.refreshing")
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
    ns.out.finish()

    try:

        if service == "microsoft":
            session = prompt_microsoft_authenticate(ns, email)
        else:
            session = prompt_yggdrasil_authenticate(ns, email)
        
    except AuthError as error:
        ns.out.task("FAILED", None)
        ns.out.task(None, "auth.error", message=str(error))
        ns.out.finish()
        return None

    if session is None:
        return None
    if caching:
        ns.out.task("..", "auth.caching")
        ns.auth_database.put(email, session)
        ns.auth_database.save()
    
    ns.out.task("OK", "auth.logged_in")
    ns.out.finish()

    return session


def prompt_yggdrasil_authenticate(ns: RootNs, email_or_username: str) -> Optional[YggdrasilAuthSession]:
    ns.out.task(None, "auth.yggdrasil.enter_password")
    password = ns.out.prompt(password=True)
    if password is None:
        ns.out.task("FAILED", "cancelled")
        ns.out.finish()
        return None
    else:
        return YggdrasilAuthSession.authenticate(ns.auth_database.get_client_id(), email_or_username, password)


def prompt_microsoft_authenticate(ns: RootNs, email: str) -> Optional[MicrosoftAuthSession]:

    from .. import LAUNCHER_NAME, LAUNCHER_VERSION
    from http.server import HTTPServer, BaseHTTPRequestHandler
    import urllib.parse
    import webbrowser
    import uuid

    server_port = 12782
    app_id = MICROSOFT_AZURE_APP_ID
    redirect_auth = "http://localhost:{}".format(server_port)
    code_redirect_uri = "{}/code".format(redirect_auth)
    exit_redirect_uri = "{}/exit".format(redirect_auth)

    nonce = uuid.uuid4().hex

    auth_url = MicrosoftAuthSession.get_authentication_url(app_id, code_redirect_uri, email, nonce)
    if not webbrowser.open(auth_url):
        ns.out.task("FAILED", "auth.microsoft.no_browser")
        ns.out.finish()
        return None

    class AuthServer(HTTPServer):

        def __init__(self):
            super().__init__(("", server_port), RequestHandler)
            self.timeout = 0.5
            self.ms_auth_done = False
            self.ms_auth_id_token: Optional[str] = None
            self.ms_auth_code: Optional[str] = None

    class RequestHandler(BaseHTTPRequestHandler):

        server_version = f"{LAUNCHER_NAME}/{LAUNCHER_VERSION}"

        def __init__(self, request, client_address: Tuple[str, int], auth_server: AuthServer) -> None:
            super().__init__(request, client_address, auth_server)

        def log_message(self, _format: str, *args: Any):
            return

        def send_auth_response(self, msg: str):
            self.end_headers()
            self.wfile.write("{}\n\n{}".format(msg, _('auth.microsoft.close_tab_and_return') if cast(AuthServer, self.server).ms_auth_done else "").encode())
            self.wfile.flush()

        def do_POST(self):
            if self.path.startswith("/code") and self.headers.get_content_type() == "application/x-www-form-urlencoded":
                content_length = int(self.headers["Content-Length"])
                qs = urllib.parse.parse_qs(self.rfile.read(content_length).decode())
                auth_server = cast(AuthServer, self.server)
                if "code" in qs and "id_token" in qs:
                    self.send_response(307)
                    # We log out the user directly after authorization, this just clear the browser cache to allow
                    # another user to authenticate with another email after. This doesn't invalid the access token.
                    self.send_header("Location", MicrosoftAuthSession.get_logout_url(app_id, exit_redirect_uri))
                    auth_server.ms_auth_id_token = qs["id_token"][0]
                    auth_server.ms_auth_code = qs["code"][0]
                    self.send_auth_response("Redirecting...")
                elif "error" in qs:
                    self.send_response(400)
                    auth_server.ms_auth_done = True
                    self.send_auth_response("Error: {} ({}).".format(qs["error_description"][0], qs["error"][0]))
                else:
                    self.send_response(404)
                    self.send_auth_response("Missing parameters.")
            else:
                self.send_response(404)
                self.send_auth_response("Unexpected page.")

        def do_GET(self):
            auth_server = cast(AuthServer, self.server)
            if self.path.startswith("/exit"):
                self.send_response(200)
                auth_server.ms_auth_done = True
                self.send_auth_response("Logged in.")
            else:
                self.send_response(404)
                self.send_auth_response("Unexpected page.")

    ns.out.task("", "auth.microsoft.opening_browser_and_listening")

    with AuthServer() as server:
        try:
            while not server.ms_auth_done:
                server.handle_request()
        except KeyboardInterrupt:
            pass

    if server.ms_auth_code is None or server.ms_auth_id_token is None:
        ns.out.task("FAILED", "auth.microsoft.failed_to_authenticate")
        ns.out.finish()
        return None
    else:
        ns.out.task("", "auth.microsoft.processing")
        if MicrosoftAuthSession.check_token_id(server.ms_auth_id_token, email, nonce):
            return MicrosoftAuthSession.authenticate(ns.auth_database.get_client_id(), app_id, server.ms_auth_code, code_redirect_uri)
        else:
            ns.out.task("FAILED", "auth.microsoft.incoherent_data")
            ns.out.finish()
            return None


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
