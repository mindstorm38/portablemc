"""CLI languages management.
"""

from ..util import get_jvm_bin_filename

from typing import Optional


def get_raw(key: str, kwargs: Optional[dict], default: Optional[str] = None) -> str:
    """Get a message translated using the given keyword formatting arguments.

    :param key: The key of the message to translate.
    :param kwargs: The keyword formatting dictionary.
    :return: Translated message, or the default value if not found. By default, the 
    default value if the key itself.
    """
    try:
        return lang[key].format_map(kwargs or {})
    except KeyError:
        return key


def get(key: str, **kwargs) -> str:
    """Get a message translated using the given keyword formatting arguments.

    :param key: The key of the message to translate.
    :return: Translated message, or the key itself if not found.
    """
    return get_raw(key, kwargs)


lang = {
        # Addons
    "addon.import_error": "The addon '{addon}' has failed to build because some packages is missing:",
    "addon.unknown_error": "The addon '{addon}' has failed to build for unknown reason:",
    # Args root
    "args": "PortableMC is an easy to use portable Minecraft launcher in only one Python "
        "script! This single-script launcher is still compatible with the official "
        "(Mojang) Minecraft Launcher stored in .minecraft and use it.",
    "args.main_dir": "Set the main directory where libraries, assets and versions. "
        "This argument can be used or not by subcommand.",
    "args.work_dir": "Set the working directory where the game run and place for examples "
        "saves, screenshots (and resources for legacy versions), it also store "
        "runtime binaries and authentication. "
        "This argument can be used or not by subcommand.",
    "args.timeout": "Set a global timeout (in decimal seconds) that can be used by various requests done by the launcher or "
        "addons. A value of 0 is usually interpreted as an 'offline mode', this means that the launcher "
        "will try to use a cached copy of the requests' response.",
    # Args common langs
    "args.common.login_service": "Authentication service type to use for logging in the game.",
    # Args search
    "args.search": "Search for Minecraft versions.",
    "args.search.kind": "Select the kind of search to operate.",
    # Args start
    "args.start": "Start a Minecraft version, default to the latest release.",
    "args.start.dry": "Simulate game starting.",
    "args.start.disable_multiplayer": "Disable the multiplayer buttons (>= 1.16).",
    "args.start.disable_chat": "Disable the online chat (>= 1.16).",
    "args.start.demo": "Start game in demo mode.",
    "args.start.resolution": "Set a custom start resolution (<width>x<height>, >= 1.6).",
    "args.start.jvm": f"Set a custom JVM '{get_jvm_bin_filename()}' executable path. If this argument is omitted a public build "
        "of a JVM is downloaded from Mojang services.",
    "args.start.jvm_args": "Change the default JVM arguments.",
    "args.start.no_better_logging": "Disable the better logging configuration built by the launcher in "
        "order to improve the log readability in the console.",
    "args.start.anonymize": "Anonymize your email or username for authentication messages.",
    "args.start.no_legacy_fix": "Flag that disable fixes for old versions (legacy merge sort, betacraft proxy), "
        "enabled by default.",
    "args.start.lwjgl": "Change the default LWJGL version used by Minecraft."
        "This argument makes additional changes in order to support additional architectures such as ARM32/ARM64. "
        "It's not guaranteed to work with every version of Minecraft and downgrading LWJGL version is not recommended.",
    "args.start.exclude_lib": "Specify Java libraries to exclude from the classpath (and download) "
        "before launching the game. Follow this pattern to specify libraries: <artifact>[:[<version>][:<classifier>]]. "
        "If your system doesn't support Mojang-provided natives, you can use both --exclude-lib and "
        "--include-bin to replace them with your own (e.g. --exclude-lib lwjgl-glfw::natives --include-bin /lib/libglfw.so).",
    "args.start.include_bin": "Include binaries (.so, .dll, .dylib) in the bin directory of the game, "
        "given files are symlinked in the directory if possible, copied if not. "
        "On linux, version numbers are discarded (e.g. /usr/lib/foo.so.1.22.2 -> foo.so). "
        "Read the --exclude-lib help for use cases.",
    "args.start.temp_login": "Flag used with -l (--login) to tell launcher not to cache your session if "
        "not already cached, disabled by default.",
    "args.start.login": "Use a email (or deprecated username) to authenticate using selected service (with --login-service, also overrides --username and --uuid).",
    "args.start.username": "Set a custom user name to play.",
    "args.start.uuid": "Set a custom user UUID to play.",
    "args.start.server": "Start the game and auto-connect to this server address (>= 1.6).",
    "args.start.server_port": "Set the server address port (given with -s, --server, >= 1.6).",
    # Args login
    "args.login": "Login into your account and save the session.",
    "args.login.microsoft": "Login using Microsoft account.",
    # Args logout
    "args.logout": "Logout and invalidate a session.",
    "args.logout.microsoft": "Logout from a Microsoft account.",
    # Args show
    "args.show": "Show and debug various data.",
    "args.show.about": "Display authors, version and license of PortableMC.",
    "args.show.auth": "Debug the authentication database and supported services.",
    "args.show.lang": "Debug the language mappings used for messages translation.",
    # Args addon
    "args.addon": "Addons management subcommands.",
    "args.addon.list": "List addons.",
    "args.addon.show": "Show an addon details.",
    # Common
    "continue_using_main_dir": "Continue using this main directory ({})? (y/N) ",
    "cancelled": "Cancelled.",
    # Version manifest error
    # f"version_manifest.error.{VersionManifestError.NOT_FOUND}": "Failed to load version manifest, timed out or not locally cached.",
    # # Json Request
    # f"json_request.error.{JsonRequestError.INVALID_RESPONSE_NOT_JSON}": "Invalid JSON response from {method} {url}, status: {status}, data: {data}",
    # Misc errors
    "error.generic": "An unexpected error happened, please report it to the authors:",
    "error.socket": "This operation requires an operational network, but a socket error happened:",
    "error.cert": "Certificate verification failed, you can try installing 'certifi' package:",
    "error.keyboard_interrupt": "Interrupted.",
    # Command search
    "search.type": "Type",
    "search.name": "Identifier",
    "search.release_date": "Release date",
    "search.last_modified": "Last modified",
    "search.flags": "Flags",
    "search.flags.local": "local",
    "search.not_found": "No version match the input.",
    # Command logout
    "logout.yggdrasil.pending": "Logging out {email} from Mojang...",
    "logout.microsoft.pending": "Logging out {email} from Microsoft...",
    "logout.success": "Logged out {email}.",
    "logout.unknown_session": "No session for {email}.",
    # Command addon list
    "addon.list.id": "ID ({count})",
    "addon.list.version": "Version",
    "addon.list.authors": "Authors",
    # Command addon show
    "addon.show.not_found": "Addon '{addon}' not found.",
    "addon.show.version": "Version: {version}",
    "addon.show.authors": "Authors: {authors}",
    "addon.show.description": "Description: {description}",
    # Command start
    "start.version.resolving": "Resolving version {version}... ",
    "start.version.resolved": "Resolved version {version}.",
    "start.version.fixed.lwjgl": "Fixed LWJGL version to {version}",
    "start.version.jar.loading": "Loading version JAR... ",
    "start.version.jar.loaded": "Loaded version JAR.",
    # f"start.version.error.{VersionError.NOT_FOUND}": "Version {version} not found.",
    # f"start.version.error.{VersionError.TO_MUCH_PARENTS}": "The version {version} has to much parents.",
    # f"start.version.error.{VersionError.JAR_NOT_FOUND}": "Version {version} JAR not found.",
    # f"start.version.error.{VersionError.INVALID_ID}": "Version id {version} is invalid for the file system.",
    "start.assets.checking": "Checking assets... ",
    "start.assets.checked": "Checked {count} assets.",
    "start.logger.loading": "Loading logger... ",
    "start.logger.loaded": "Loaded logger.",
    "start.logger.loaded_pretty": "Loaded pretty logger.",
    "start.libraries.loading": "Loading libraries... ",
    "start.libraries.loaded": "Loaded {count} libraries.",
    "start.libraries.exclude.unused": "Library exclusion '{pattern}' didn't match a libary.",
    "start.libraries.exclude.usage": "Library exclusion '{pattern}' matched {count} libraries.",
    "start.jvm.loading": "Loading Java... ",
    "start.jvm.system_fallback": "Loaded system Java at {path}.",
    "start.jvm.loaded": "Loaded Mojang Java {version}.",
    # f"start.jvm.error.{JvmLoadingError.UNSUPPORTED_ARCH}": "No JVM download was found for your platform architecture, "
    #     "use --jvm argument to manually set the path to your JVM executable.",
    # f"start.jvm.error.{JvmLoadingError.UNSUPPORTED_VERSION}": "No JVM download was found, "
    #     "use --jvm argument to manually set the path to your JVM executable.",
    # f"start.jvm.error.{JvmLoadingError.UNSUPPORTED_LIBC}": "No JVM download was found for your libc (only glibc is supported), "
    #     "use --jvm argument to manually set the path to your JVM executable.",
    "start.additional_binary_not_found": "The additional binary '{bin}' doesn't exists.",
    "start.starting": "Starting the game...",
    "start.starting_info": "Username: {username} ({uuid})",
    # Pretty download
    "download.downloading": "Downloading",
    "download.downloaded": "Downloaded {success_count}/{total_count} files, {size} in {duration:.1f}s ({errors}).",
    "download.no_error": "no error",
    "download.errors": "{count} errors",
    # f"download.error.{DownloadReport.CONN_ERROR}": "Connection error",
    # f"download.error.{DownloadReport.NOT_FOUND}": "Not found",
    # f"download.error.{DownloadReport.INVALID_SIZE}": "Invalid size",
    # f"download.error.{DownloadReport.INVALID_SHA1}": "Invalid SHA1",
    # f"download.error.{DownloadReport.TOO_MANY_REDIRECTIONS}": "Too many redirections",
    # Auth common
    "auth.refreshing": "Invalid session, refreshing...",
    "auth.refreshed": "Session refreshed for {email}.",
    "auth.validated": "Session validated for {email}.",
    "auth.caching": "Caching your session...",
    "auth.logged_in": "Logged in",
    "auth.microsoft_requires_email": "Even if you are using -m (--microsoft), you must use -l argument with your "
                                     "Microsoft email.",
    # Auth Yggdrasil
    "auth.yggdrasil": "Authenticating {email} with Mojang...",
    "auth.yggdrasil.note_for_microsoft": "Logging in with Mojang is now deprecated, if you intented to log into a Microsoft account, add -m flag in your command.",
    "auth.yggdrasil.enter_password": "Password: ",
    # f"auth.error.{AuthError.YGGDRASIL}": "{details}",
    # Auth Microsoft
    "auth.microsoft": "Authenticating {email} with Microsoft...",
    "auth.microsoft.no_browser": "Failed to open Microsoft login page, no web browser is supported.",
    "auth.microsoft.opening_browser_and_listening": "Opened authentication page in browser...",
    "auth.microsoft.failed_to_authenticate": "Failed to authenticate.",
    "auth.microsoft.processing": "Processing authentication against Minecraft services...",
    "auth.microsoft.incoherent_data": "Incoherent authentication data, please retry.",
    # f"auth.error.{AuthError.MICROSOFT_INCONSISTENT_USER_HASH}": "Inconsistent user hash.",
    # f"auth.error.{AuthError.MICROSOFT_DOES_NOT_OWN_MINECRAFT}": "This account does not own Minecraft.",
    # f"auth.error.{AuthError.MICROSOFT_OUTDATED_TOKEN}": "The token is no longer valid.",
    # f"auth.error.{AuthError.MICROSOFT}": "Misc error: {details}."
}
