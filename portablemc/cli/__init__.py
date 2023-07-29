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

from portablemc.util import LibrarySpecifier
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
    ForgePostProcessedEvent, ForgeInstallError

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

    version.disable_multiplayer = ns.disable_mp
    version.disable_chat = ns.disable_chat
    version.demo = ns.demo
    version.resolution = ns.resolution
    version.jvm_path = None if ns.jvm is None else Path(ns.jvm)

    if ns.server is not None:
        version.set_quick_play_multiplayer(ns.server, ns.server_port or 25565)

    if ns.no_fix:
        version.fixes.clear()
    
    if ns.lwjgl is not None:
        version.fixes[Version.FIX_LWJGL] = ns.lwjgl

    if ns.login is not None:
        version.auth_session = prompt_authenticate(ns, ns.login, not ns.temp_login, ns.auth_service, ns.auth_anonymize)
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

        if ns.verbose >= 1 and len(env.fixes):
            ns.out.task("INFO", "start.fixes")
            ns.out.finish()
            for fix, fix_value in env.fixes.items():
                ns.out.task(None, f"start.fix.{fix}", value=fix_value)
                ns.out.finish()

        # If not dry run, run it!
        if not ns.dry:

            # Included binaries
            if ns.include_bin is not None:
                for bin_path in ns.include_bin:
                    bin_path = Path(bin_path)
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

            env.run(CliRunner(ns))

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
    
    except LibraryNotFoundError as error:
        ns.out.task("FAILED", f"start.libraries.not_found_error", spec=str(error.lib))
        ns.out.finish()
    
    except ForgeInstallError as error:
        ns.out.task("FAILED", f"start.forge.install_error.{error.code}")
        ns.out.finish()

    except DownloadError as error:
        ns.out.task("FAILED", None)
        ns.out.finish()
        for entry, code in error.errors:
            ns.out.task(None, "download.error", name=entry.name, message=_(f"download.error.{code}"))
            ns.out.finish()
    
    sys.exit(EXIT_FAILURE)

def cmd_start_handler(ns: StartNs, kind: str, parts: List[str]) -> Optional[Version]:
    """This function handles particular kind of versions. If this function successfully
    decodes, the corresponding version should be returned. The global version's format 
    being parsed is <kind>[:<part>..].

    The parts list contains at least one element, parts may be empty.

    This function returns false if parsing fail, in such case the expected format is
    printed out to the user on output (lang's key: "args.start.version.<kind>").
    """

    if kind == "standard":
        if len(parts) != 1:
            return None
        return Version(parts[0] or "release", context=ns.context)
    
    elif kind in ("fabric", "quilt"):
        if len(parts) > 2:
            return None
        constructor = FabricVersion.with_fabric if kind == "fabric" else FabricVersion.with_quilt
        prefix = ns.fabric_prefix if kind == "fabric" else ns.quilt_prefix
        return constructor(parts[0] or "release", parts[1] if len(parts) == 2 else None, context=ns.context, prefix=prefix)
    
    elif kind == "forge":
        if len(parts) != 1:
            return None
        return ForgeVersion(parts[0] or "release", context=ns.context, prefix=ns.forge_prefix)
    
    else:
        return None


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

    
class StartWatcher(SimpleWatcher):

    def __init__(self, ns: RootNs) -> None:

        def progress_task(key: str, **kwargs) -> None:
            ns.out.task("..", key, **kwargs)

        def finish_task(key: str, **kwargs) -> None:
            ns.out.task("OK", key, **kwargs)
            ns.out.finish()
        
        def features(e: FeaturesEvent) -> None:
            if ns.verbose >= 1 and len(e.features):
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
                ns.out.task("..", "start.forge.resolving", version=e.forge_version)
            else:
                ns.out.task("OK", "start.forge.resolved", version=e.forge_version)
                ns.out.finish()

        super().__init__({
            VersionLoadingEvent: lambda e: progress_task("start.version.loading", version=e.version),
            VersionFetchingEvent: lambda e: progress_task("start.version.fetching", version=e.version),
            VersionLoadedEvent: lambda e: finish_task("start.version.loaded", version=e.version),
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
            size=f"{format_number(self.size + sum(self.sizes))}o",
            speed=f"{format_number(speed)}o/s")

        if e.done:
            self.size += e.size

    def download_complete(self, e: DownloadCompleteEvent) -> None:
        self.ns.out.task("OK", None)
        self.ns.out.finish()


class CliRunner(StreamRunner):

    def __init__(self, ns: RootNs) -> None:
        super().__init__()
        self.ns = ns

    def process_create(self, args: List[str], work_dir: Path) -> Popen:
        
        self.ns.out.print("\n")
        if self.ns.verbose >= 1:
            self.ns.out.print(" ".join(args) + "\n")

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
