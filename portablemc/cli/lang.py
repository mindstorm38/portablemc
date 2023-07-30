"""CLI languages management.
"""

from portablemc.standard import JvmNotFoundError, JvmLoadedEvent, Version
from portablemc.forge import ForgeInstallError
from portablemc.util import jvm_bin_filename
from portablemc.download import DownloadResultError

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
    # "addon.import_error": "The addon '{addon}' has failed to build because some packages is missing:",
    # "addon.unknown_error": "The addon '{addon}' has failed to build for unknown reason:",
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
    "args.timeout": "Set a global timeout (in decimal seconds) for network requests.",
    "args.output": "Set the output format of the launcher, defaults to human-color.",
    "args.verbose": "Enable verbose output. The more -v argument you put, the more verbose the launcher will be, depending on subcommands' support (usually -v, -vv, -vvv).",
    # Args common langs
    "args.common.auth_service": "Authentication service type to use for logging in the game.",
    # Args search
    "args.search": "Search for Minecraft versions.",
    "args.search.kind": "Select the kind of search to operate.",
    # Args start
    "args.start": "Start a Minecraft version.",
    "args.start.version": "Version identifier (default to release): {formats}.",
    "args.start.version.standard": "release|snapshot|<vanilla-version>",
    "args.start.version.fabric": "fabric:[<vanilla-version>[:<loader-version>]]",
    "args.start.version.quilt": "quilt:[<vanilla-version>[:<loader-version>]]",
    "args.start.version.forge": "forge:[<forge-version>] (forge-version >= 1.5.2)",
    "args.start.dry": "Simulate game starting.",
    "args.start.disable_multiplayer": "Disable the multiplayer buttons (>= 1.16).",
    "args.start.disable_chat": "Disable the online chat (>= 1.16).",
    "args.start.demo": "Start game in demo mode.",
    "args.start.resolution": "Set a custom start resolution (<width>x<height>, >= 1.6).",
    "args.start.resolution.invalid": "invalid format '{given}', expected <width>x<height>",
    "args.start.jvm": f"Set a custom JVM '{jvm_bin_filename}' executable path. If this argument is omitted a public build "
        "of a JVM is downloaded from Mojang services (if Mojang does not support your system, error is returned).",
    "args.start.jvm_args": "Change the default JVM arguments.",
    "args.start.no_fix": "Flag that globally disable fixes (proxy for old versions), "
        "enabled by default.",
    "args.start.fabric_prefix": "Change the prefix of the version ID when starting with Fabric (<prefix>-<vanilla-version>-<loader-version>).",
    "args.start.quilt_prefix": "Change the prefix of the version ID when starting with Quilt (<prefix>-<vanilla-version>-<loader-version>).",
    "args.start.forge_prefix": "Change the prefix of the version ID when starting with Forge (<prefix>-<forge-version>).",
    "args.start.lwjgl": "Change the default LWJGL version used by Minecraft. "
        "This argument makes additional changes in order to support additional architectures such as Arm. "
        "It's not guaranteed to work with every version of Minecraft and downgrading LWJGL version is not recommended.",
    "args.start.exclude_lib": "Specify Java libraries to exclude from the classpath (and download) "
        "before launching the game. Follow this pattern to specify libraries: <artifact>[:[<version>][:<classifier>]]. "
        "If your system doesn't support Mojang-provided natives, you can use both --exclude-lib and "
        "--include-bin to replace them with your own (e.g. --exclude-lib lwjgl-glfw::natives --include-bin /lib/libglfw.so).",
    "args.start.include_bin": "Include binaries (.so, .dll, .dylib) in the bin directory of the game, "
        "given files are symlinked in the directory if possible, copied if not. "
        "On linux, version numbers are discarded (e.g. /usr/lib/foo.so.1.22.2 -> foo.so). "
        "Read the --exclude-lib help for use cases.",
    "args.start.auth_anonymize": "Anonymize your email or username for authentication messages.",
    "args.start.temp_login": "Flag used with -l (--login) to tell launcher not to cache your session if "
        "not already cached, disabled by default.",
    "args.start.login": "Use a email (or deprecated username) to authenticate using selected service (with --auth-service, also overrides --username and --uuid).",
    "args.start.username": "Set a custom user name to play.",
    "args.start.uuid": "Set a custom user UUID to play.",
    "args.start.server": "Start the game and directly connect to a multiplayer server (>= 1.6).",
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
    # "args.addon": "Addons management subcommands.",
    # "args.addon.list": "List addons.",
    # "args.addon.show": "Show an addon details.",
    # Common
    "echo": "{echo}",
    "cancelled": "Cancelled.",
    "keyboard_interrupt": "Keyboard interrupted.",
    # Common errors
    "error.os": "An unexpected OS error happened:",
    "error.socket": "This operation requires an operational network, but a socket error happened:",
    "error.cert": "Certificate verification failed, you can try installing 'certifi' package:",
    # Command search
    "search.type": "Type",
    "search.name": "Identifier",
    "search.release_date": "Release date",
    "search.last_modified": "Last modified",
    "search.flags": "Flags",
    "search.flags.local": "local",
    "search.loader_version": "Loader version",
    # Command logout
    "logout.yggdrasil.pending": "Logging out {email} from Mojang...",
    "logout.microsoft.pending": "Logging out {email} from Microsoft...",
    "logout.success": "Logged out {email}",
    "logout.unknown_session": "No session for {email}",
    # Command addon list
    # "addon.list.id": "ID ({count})",
    # "addon.list.version": "Version",
    # "addon.list.authors": "Authors",
    # Command addon show
    # "addon.show.not_found": "Addon '{addon}' not found.",
    # "addon.show.version": "Version: {version}",
    # "addon.show.authors": "Authors: {authors}",
    # "addon.show.description": "Description: {description}",
    # Command start
    "start.version.invalid_id": "Invalid version id, expected: {expected}",
    "start.version.invalid_id_unknown_kind": "Invalid version id, unknown kind: {kind}.",
    "start.version.loading": "Loading version {version}... ",
    "start.version.fetching": "Fetching version {version}... ",
    "start.version.loaded": "Loaded version {version}",
    "start.version.loaded.fetched": "Loaded version {version} (fetched)",
    "start.version.not_found": "Version {version} not found",
    "start.version.too_much_parents": "Too much parents while resolving versions.",
    "start.features": "Features: {features}",
    "start.jar.found": "Checked version jar",
    "start.jar.not_found": "Version jar not found",
    "start.assets.resolving": "Checking assets version {index_version}... ",
    "start.assets.resolved": "Checked {count} assets version {index_version}",
    "start.libraries.resolving": "Checking libraries...",
    "start.libraries.resolved": "Checked {class_libs_count} class and {native_libs_count} native libraries",
    "start.libraries.excluded": "Excluded library {spec}",
    "start.libraries.unused_filter": "Unused library filter {filter}",
    "start.libraries.not_found_error": "A required library is not installed but no way to download it: {spec}",
    "start.logger.found": "Using logger {version}",
    "start.jvm.loading": "Loading java...",
    f"start.jvm.loaded.{JvmLoadedEvent.MOJANG}": "Loaded Mojang java {version}",
    f"start.jvm.loaded.{JvmLoadedEvent.BUILTIN}": "Loaded builtin java {version}",
    f"start.jvm.loaded.{JvmLoadedEvent.CUSTOM}": "Loaded custom java {version}",
    f"start.jvm.not_found_error.{JvmNotFoundError.UNSUPPORTED_ARCH}": "No JVM download was found for your platform architecture, "
        "use --jvm argument to manually set the path to your JVM executable.",
    f"start.jvm.not_found_error.{JvmNotFoundError.UNSUPPORTED_VERSION}": "No JVM download was found, "
        "use --jvm argument to manually set the path to your JVM executable.",
    f"start.jvm.not_found_error.{JvmNotFoundError.UNSUPPORTED_LIBC}": "No JVM download was found for your libc (only glibc is supported), "
        "use --jvm argument to manually set the path to your JVM executable.",
    f"start.jvm.not_found_error.{JvmNotFoundError.BUILTIN_INVALID_VERSION}": f"The builtin JVM ({jvm_bin_filename}) is not compatible "
        "with selected game version.",
    "start.fixes": "Applied the following fixes:",
    f"start.fix.{Version.FIX_LEGACY_PROXY}": "Using legacy proxy for online resources: {value}",
    f"start.fix.{Version.FIX_LEGACY_MERGE_SORT}": "Using legacy merge sort: {value}",
    f"start.fix.{Version.FIX_LEGACY_RESOLUTION}": "Included resolution into game arguments: {value}",
    f"start.fix.{Version.FIX_LEGACY_QUICK_PLAY}": "Included legacy quick play into game arguments: {value}",
    f"start.fix.{Version.FIX_AUTH_LIB_2_1_28}": "Fixed authlib for 1.16.4 or 1.16.5: {value}",
    f"start.fix.{Version.FIX_LWJGL}": "Fixed LWJGL: {value}",
    "start.additional_binary_not_found": "The additional binary '{path}' doesn't exists.",
    "start.bin_install": "Installed binary {src_file} as {dst_name}",
    # Command start (LWJGL fix)
    "start.lwjgl.version": "Forced LWJGL version to {version}",
    # Command start (fabric)
    "start.fabric.resolving": "Resolving {api} loader for {vanilla_version}...",
    "start.fabric.resolved": "Resolved {api} loader {loader_version} for {vanilla_version}",
    # Command start (forge)
    "start.forge.resolving": "Resolving forge alias {version}...",
    "start.forge.resolved": "Resolved forge {version}",
    "start.forge.post_processing": "Forge post processing: {task}...",
    "start.forge.post_processed": "Forge post processing done",
    f"start.forge.install_error.{ForgeInstallError.INSTALL_PROFILE_NOT_FOUND}": "Install profile not found in the forge installer.",
    f"start.forge.install_error.{ForgeInstallError.VERSION_METADATA_NOT_FOUND}": "Version metadata not found in the forge installer.",
    # Pretty download
    "download.threads_count": "Download threads count: {count}",
    "download.start": "Download starting...",
    "download.progress": "Download: {count}/{total_count} {size:>8} @ {speed}",
    "download.error": "{name}: {message}",
    f"download.error.{DownloadResultError.CONNECTION}": "Connection error",
    f"download.error.{DownloadResultError.NOT_FOUND}": "Not found",
    f"download.error.{DownloadResultError.INVALID_SIZE}": "Invalid size",
    f"download.error.{DownloadResultError.INVALID_SHA1}": "Invalid SHA1",
    # Auth common
    "auth.refreshing": "Invalid session, refreshing...",
    "auth.refreshed": "Session refreshed for {email}",
    "auth.validated": "Session validated for {email}",
    "auth.caching": "Caching your session...",
    "auth.logged_in": "Logged in",
    "auth.error": "Error authenticating: {message}",
    # Auth Yggdrasil
    "auth.yggdrasil": "Authenticating {email} with Mojang...",
    "auth.yggdrasil.enter_password": "Password: ",
    # Auth Microsoft
    "auth.microsoft": "Authenticating {email} with Microsoft...",
    "auth.microsoft.no_browser": "Failed to open Microsoft login page, no web browser found on your system.",
    "auth.microsoft.opening_browser_and_listening": "Opened authentication page in browser...",
    "auth.microsoft.close_tab_and_return": "Close this tab and return to the launcher.",
    "auth.microsoft.failed_to_authenticate": "Failed to authenticate.",
    "auth.microsoft.processing": "Processing authentication against Minecraft services...",
    "auth.microsoft.incoherent_data": "Incoherent authentication data, please retry.",
}
