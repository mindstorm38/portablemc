"""Main 
"""

from subprocess import Popen
from pathlib import Path
import socket
import sys

from .parse import register_arguments, RootNs, SearchNs, StartNs, LoginNs, LogoutNs
from .util import format_locale_date, format_time, format_number, anonymize_email
from .output import Output, HumanOutput, MachineOutput, OutputTable
from .lang import get as _, lang

from ..download import DownloadStartEvent, DownloadProgressEvent, DownloadCompleteEvent, DownloadError
from ..auth import AuthDatabase, AuthSession, MicrosoftAuthSession, YggdrasilAuthSession, \
    AuthError
from ..task import Watcher, Sequence, TaskEvent
from ..util import LibrarySpecifier

from ..vanilla import add_vanilla_tasks, Context, VersionManifest, \
    MetadataRoot, VersionRepositories, VersionNotFoundError, TooMuchParentsError, \
    VersionLoadingEvent, VersionFetchingEvent, VersionLoadedEvent, \
    JarFoundEvent, JarNotFoundError, \
    AssetsResolveEvent, \
    LibrariesOptions, Libraries, LibrariesResolvingEvent, LibrariesResolvedEvent, \
    LoggerFoundEvent, \
    Jvm, JvmResolvingEvent, JvmResolveEvent, JvmNotFoundError, \
    ArgsOptions, ArgsFixesEvent, StreamRunTask, BinaryInstallEvent, XmlStreamEvent

from ..lwjgl import add_lwjgl_tasks, LwjglVersion, LwjglVersionEvent
from ..fabric import add_fabric_tasks, FabricRoot, FabricResolveEvent
from ..forge import add_forge_tasks, ForgeRoot, ForgeResolveEvent, ForgePostProcessingEvent, \
    ForgePostProcessedEvent, ForgeInstallError

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
    ns.out = get_output(ns.out_kind)
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
        ns.out.task("HALT", "keyboard_interrupt")
        ns.out.finish()
    
    except OSError as error:

        from urllib.error import URLError
        from ssl import SSLCertVerificationError

        key = "error.os"
        if isinstance(error, URLError) and isinstance(error.reason, SSLCertVerificationError):
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
    table = ns.out.table()
    cmd_search_handler(ns, ns.kind, table)
    table.print()
    sys.exit(EXIT_OK)

def cmd_search_handler(ns: SearchNs, kind: str, table: OutputTable):
    """Internal function that handles searching a particular kind of search.
    The value of "kind" is constrained by choices in the argument parser.
    """

    search = ns.input

    if kind == "manifest":
        
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
            search = ns.version_manifest.filter_latest(search)[0]

        for alias, version in request_promo_versions().items():
            if search is None or search in alias:
                table.add(alias, version)

    elif kind in ("fabric", "quilt"):

        from ..fabric import FABRIC_API, QUILT_API

        table.add(_("search.loader_version"))
        table.separator()

        api = FABRIC_API if kind == "fabric" else QUILT_API
        for version in api.request_fabric_loader_versions():
            if search is None or search in version:
                table.add(version)

    else:
        raise ValueError()


def cmd_start(ns: StartNs):

    version_parts = ns.version.split(":")

    # If no split, the kind of version is "vanilla": parts have at least 2 elements.
    if len(version_parts) == 1:
        version_parts = ["vanilla", version_parts[0]]
    
    # Create sequence with supported mod loaders.
    seq = Sequence()
    add_vanilla_tasks(seq, run=False)

    # Use a custom run task.
    if not ns.dry:
        seq.append_task(OutputRunTask(ns))
    
    # Add mandatory states.
    seq.state.insert(ns.context)
    seq.state.insert(VersionRepositories(ns.version_manifest))
    
    # No handler means that the format is invalid.
    if not cmd_start_handler(ns, version_parts[0], version_parts[1:], seq):
        format_key = f"args.start.version.{version_parts[0]}"
        if format_key not in lang:
            ns.out.task("FAILED", "start.version.invalid_id_unknown_kind", kind=version_parts[0])
        else:
            ns.out.task("FAILED", "start.version.invalid_id", expected=_(format_key))
        ns.out.finish()
        sys.exit(EXIT_FAILURE)
    
    # Various options for ArgsTask in order to setup the arguments to start the game.
    args_opts = seq.state[ArgsOptions]
    args_opts.disable_multiplayer = ns.disable_mp
    args_opts.disable_chat = ns.disable_chat
    args_opts.demo = ns.demo
    args_opts.server_address = ns.server
    args_opts.server_port = ns.server_port
    args_opts.resolution = ns.resolution

    if ns.no_legacy_fix:
        args_opts.fixes.clear()

    if ns.login is not None:
        args_opts.auth_session = prompt_authenticate(ns, ns.login, not ns.temp_login, ns.auth_service, ns.auth_anonymize)
        if args_opts.auth_session is None:
            sys.exit(EXIT_FAILURE)
    else:
        args_opts.set_offline(ns.username, ns.uuid)
    
    # Included binaries
    if ns.include_bin is not None:
        native_libs = seq.state[Libraries].native_libs
        for bin_path in ns.include_bin:
            bin_path = Path(bin_path)
            if not bin_path.is_file():
                ns.out.task("FAILED", "start.additional_binary_not_found", path=bin_path)
                ns.out.finish()
                sys.exit(EXIT_FAILURE)
            native_libs.append(bin_path)

    # Excluded libraries
    if ns.exclude_lib is not None:
        exclude_filters = ns.exclude_lib
        def libraries_predicate(spec: LibrarySpecifier) -> bool:
            for spec_filter in exclude_filters:
                if spec_filter.matches(spec):
                    return False
            return True
        seq.state[LibrariesOptions].predicates.append(libraries_predicate)

    # If LWJGL fix is required.
    if ns.lwjgl is not None:
        add_lwjgl_tasks(seq)
        seq.state.insert(LwjglVersion(ns.lwjgl))

    # If a manual JVM is specified, we set the JVM state so that JvmTask won't run.
    if ns.jvm is not None:
        seq.state.insert(Jvm(Path(ns.jvm), None))

    # Add watchers of the installation.
    seq.add_watcher(StartWatcher(ns))
    seq.add_watcher(DownloadWatcher(ns))

    if ns.verbose >= 2:
        ns.out.task("INFO", "start.tasks", tasks=", ".join((type(task).__name__ for task in seq.tasks)))
        ns.out.finish()
    
    try:
        seq.execute()
        sys.exit(EXIT_OK)
    
    except VersionNotFoundError as error:
        ns.out.task("FAILED", "start.version.not_found", version=error.version)
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
        ns.out.task("FAILED", f"start.jvm.not_found_error.{error.code}")
        ns.out.finish()
    
    except ForgeInstallError as error:
        ns.out.task("FAILED", f"start.forge.install_error.{error.code}")
        ns.out.finish()
    
    sys.exit(EXIT_FAILURE)

def cmd_start_handler(ns: StartNs, kind: str, parts: List[str], seq: Sequence) -> bool:
    """This function handles particular kind of versions. If this function successfully
    decodes, the corresponding tasks and states should be configured in the given 
    sequence. The global version's format being parsed is <kind>[:<part>..].

    The parts list contains at least one element, parts may be empty.

    This function returns false if parsing fail, in such case the expected format is
    printed out to the user on output (lang's key: "args.start.version.<kind>").
    """

    if kind == "vanilla":
        if len(parts) != 1:
            return False
        
        vanilla_version = ns.version_manifest.filter_latest(parts[0] or "release")[0]
        seq.state.insert(MetadataRoot(vanilla_version))

    elif kind in ("fabric", "quilt"):
        if len(parts) > 2:
            return False
        
        vanilla_version = ns.version_manifest.filter_latest(parts[0] or "release")[0]
        loader_version = parts[1] if len(parts) == 2 else None
        
        constructor = FabricRoot.with_fabric if kind == "fabric" else FabricRoot.with_quilt
        prefix = ns.fabric_prefix if kind == "fabric" else ns.quilt_prefix
        
        add_fabric_tasks(seq)
        seq.state.insert(constructor(vanilla_version, loader_version, prefix))
    
    elif kind == "forge":
        if len(parts) != 1:
            return False
        
        vanilla_version = ns.version_manifest.filter_latest(parts[0] or "release")[0]

        add_forge_tasks(seq)
        seq.state.insert(ForgeRoot(vanilla_version, ns.forge_prefix))
    
    else:
        return False

    return True


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
    from uuid import uuid4
    import urllib.parse
    import webbrowser

    server_port = 12782
    app_id = MICROSOFT_AZURE_APP_ID
    redirect_auth = f"http://localhost:{server_port}"
    code_redirect_uri = f"{redirect_auth}/code"
    exit_redirect_uri = f"{redirect_auth}/exit"

    nonce = uuid4().hex

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

    
class StartWatcher(Watcher):

    def __init__(self, ns: RootNs) -> None:
        self.ns = ns
    
    def handle(self, event: Any) -> None:

        out = self.ns.out

        if isinstance(event, TaskEvent):
            # We let the message being overwritten
            if self.ns.verbose >= 2 and not event.done:
                out.task("INFO", "start.task.execute", task=type(event.task).__name__)
                out.finish()
            
        if isinstance(event, VersionLoadingEvent):
            out.task("..", "start.version.loading", version=event.version)
        
        elif isinstance(event, VersionFetchingEvent):
            out.task("..", "start.version.fetching", version=event.version)

        elif isinstance(event, VersionLoadedEvent):
            out.task("OK", "start.version.loaded", version=event.version)
            out.finish()
        
        elif isinstance(event, LwjglVersionEvent):
            out.task("OK", "start.lwjgl.version", version=event.version)
            out.finish()

        elif isinstance(event, JarFoundEvent):
            out.task("OK", "start.jar.found")
            out.finish()
        
        elif isinstance(event, AssetsResolveEvent):
            if event.count is None:
                out.task("..", "start.assets.resolving", index_version=event.index_version)
            else:
                out.task("OK", "start.assets.resolved", index_version=event.index_version, count=event.count)
                out.finish()
        
        elif isinstance(event, LibrariesResolvingEvent):
            out.task("..", "start.libraries.resolving")
        
        elif isinstance(event, LibrariesResolvedEvent):
            out.task("OK", "start.libraries.resolved", class_libs_count=event.class_libs_count, native_libs_count=event.native_libs_count)
            out.finish()
            for spec in event.excluded_libs:
                out.task(None, "start.libraries.excluded", spec=str(spec))
                out.finish()

        elif isinstance(event, LoggerFoundEvent):
            out.task("OK", "start.logger.found", version=event.version)
            out.finish()
        
        elif isinstance(event, JvmResolvingEvent):
            out.task("..", "start.jvm.resolving")
        
        elif isinstance(event, JvmResolveEvent):
            if event.files_count is None:
                out.task("OK", "start.jvm.resolved_builtin", version=event.version or _("start.jvm.unknown_version"))
            else:
                out.task("OK", "start.jvm.resolved", version=event.version or _("start.jvm.unknown_version"), files_count=event.files_count)
            out.finish()
        
        elif isinstance(event, ArgsFixesEvent):
            if self.ns.verbose >= 1 and len(event.fixes):
                out.task("INFO", "start.args.fixes")
                out.finish()
                for fix in event.fixes:
                    out.task(None, f"start.args.fix.{fix}")
                    out.finish()
        
        elif isinstance(event, BinaryInstallEvent):
            if self.ns.verbose >= 2:
                try:
                    event.src_file.relative_to(self.ns.context.libraries_dir)
                    src_file = Path(*event.src_file.parts[-2:])
                except ValueError:
                    src_file = str(event.src_file)
                
                out.task("INFO", "start.bin_install", src_file=src_file, dst_name=event.dst_name)
                out.finish()
        
        elif isinstance(event, FabricResolveEvent):
            if event.loader_version is None:
                out.task("..", "start.fabric.resolving", api=event.api.name, vanilla_version=event.vanilla_version)
            else:
                out.task("OK", "start.fabric.resolved", api=event.api.name, loader_version=event.loader_version, vanilla_version=event.vanilla_version)
                out.finish()
        
        elif isinstance(event, ForgeResolveEvent):
            if event.alias:
                out.task("..", "start.forge.resolving", version=event.forge_version)
            else:
                out.task("OK", "start.forge.resolved", version=event.forge_version)
                out.finish()
        
        elif isinstance(event, ForgePostProcessingEvent):
            out.task("..", "start.forge.post_processing", task=event.task)

        elif isinstance(event, ForgePostProcessedEvent):
            out.task("OK", "start.forge.post_processed")
            out.finish()


class DownloadWatcher(Watcher):
    """A watcher for pretty printing download task.
    """

    def __init__(self, ns: RootNs) -> None:
        self.ns = ns
        self.entries_count: int
        self.total_size: int
        self.size: int
        self.speeds: List[float]
    
    def handle(self, event: Any) -> None:

        if isinstance(event, DownloadStartEvent):

            if self.ns.verbose:
                self.ns.out.task("INFO", "download.threads_count", count=event.threads_count)
                self.ns.out.finish()

            self.entries_count = event.entries_count
            self.total_size = event.size
            self.size = 0
            self.speeds = [0.0] * event.threads_count
            self.ns.out.task("..", "download.start")

        elif isinstance(event, DownloadProgressEvent):
            self.speeds[event.thread_id] = event.speed
            speed = sum(self.speeds)
            self.size += event.size
            total_count = str(self.entries_count)
            count = f"{event.count:{len(total_count)}}"
            self.ns.out.task("..", "download.progress", 
                count=count,
                total_count=total_count,
                size=f"{format_number(self.size)}o",
                speed=f"{format_number(speed)}o/s")
            
        elif isinstance(event, DownloadCompleteEvent):
            self.ns.out.task("OK", None)
            self.ns.out.finish()


class OutputRunTask(StreamRunTask):

    def __init__(self, ns: RootNs) -> None:
        super().__init__()
        self.ns = ns

    def process_stream_thread(self, process: Popen) -> None:
        self.ns.out.print("\n")
        return super().process_stream_thread(process)

    def process_stream_event(self, event: Any) -> None:

        out = self.ns.out

        if isinstance(event, XmlStreamEvent):
            time = format_time(event.time)
            out.print(f"[{time}] [{event.thread}] [{event.level}] {event.message}\n")
            if event.throwable is not None:
                out.print(f"{event.throwable.rstrip()}\n")
        else:
            out.print(str(event))
