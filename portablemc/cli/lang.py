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
    # Args root
    "args._": 
        "  A fast, reliable and cross-platform command-line Minecraft launcher and API\n"
        "  for developers. Including fast and easy installation of common mod loaders such\n"
        "  as Fabric, LegacyFabric, Forge, NeoForge, Quilt and Babric. This launcher is\n" 
        "  compatible with the standard Minecraft directories.\n\n",
    "args.main_dir": "Set the main directory where libraries, assets and versions.",
    "args.work_dir": "Set the working directory where the game run and place for examples "
        "saves, screenshots (and resources for legacy versions), it also store "
        "runtime binaries and authentication.",
    "args.timeout": "Set a global timeout (in decimal seconds) for network requests.",
    "args.output": "Set the output format of the launcher, defaults to human-color, human if not a TTY.",
    "args.output.comp.human-color": "Human readable output with color.",
    "args.output.comp.human": "Human readable output.",
    "args.output.comp.machine": "Machine readable output.",
    "args.verbose": "Enable verbose output. The more -v argument you put, the more verbose "
        "the launcher will be, depending on subcommands' support (usually -v, -vv, -vvv).",
    # Args common langs
    "args.common.help": "Show this help message and exit.",
    "args.common.auth_service": "Authentication service type to use for logging in the game.",
    "args.common.auth_service.comp.microsoft": "Microsoft authentication (default).",
    "args.common.auth_service.comp.yggdrasil": "Mojang authentication (deprecated).",
    "args.common.auth_no_browser": "Prevent the authentication service to open your system's web browser.",
    # Args search
    "args.search": "Search for versions.",
    "args.search._": 
        "  Search for versions, by default this command will search for official Mojang version\n"
        "  but you can change this behavior and search for local or mod loaders versions with the\n"
        "  -k (--kind) argument. Note that the displayed table layout depends on the version kind.\n"
        "  There is a special case when using version aliases 'release' or 'snapshot', in such case\n"
        "  the version alias is resolved and the real version is displayed. If no filter is given,\n"
        "  all results are displayed.\n\n"
        "    $ portablemc search\n"
        "    $ portablemc search release\n",
    "args.search.kind": "Select the kind of search to operate.",
    "args.search.kind.comp.mojang": "Search for official Mojang versions (default).",
    "args.search.kind.comp.local": "Search for locally installed versions.",
    "args.search.kind.comp.forge": "Search for Forge versions.",
    "args.search.kind.comp.fabric": "Search for Fabric versions.",
    "args.search.kind.comp.legacyfabric": "Search for LegacyFabric versions.",
    "args.search.kind.comp.quilt": "Search for Quilt versions.",
    "args.search.kind.comp.babric": "Search for Babric versions.",
    "args.search.input": "Search input.",
    "args.search.input.comp.release": "Resolve version of the latest release.",
    "args.search.input.comp.snapshot": "Resolve version of the latest snapshot.",
    # Args start
    "args.start": "Start the game.",
    "args.start.version": "Version identifier (default to release): {formats}.",
    "args.start.version.standard": "release|snapshot|<vanilla-version>",
    "args.start.version.fabric": "fabric:[<vanilla-version>[:<loader-version>]]",
    "args.start.version.legacyfabric": "legacyfabric:[<vanilla-version>[:<loader-version>]]",
    "args.start.version.quilt": "quilt:[<vanilla-version>[:<loader-version>]]",
    "args.start.version.babric": "babric::[<loader-version>]",
    "args.start.version.forge": "forge:[<forge-version>] (forge-version >= 1.5.2)",
    "args.start.version.neoforge": "neoforge:[<neoforge-version>] (neoforge-version >= 1.20.1)",
    "args.start.version.comp.release": "Start the latest release (default).",
    "args.start.version.comp.snapshot": "Start the latest snapshot.",
    "args.start.version.comp.fabric": "Start Fabric mod loader with latest release.",
    "args.start.version.comp.legacyfabric": "Start LegacyFabric mod loader with latest release.",
    "args.start.version.comp.quilt": "Start Quilt mod loader with latest release.",
    "args.start.version.comp.babric": "Start Babric mod loader with beta 1.7.3.",
    "args.start.version.comp.forge": "Start Forge mod loader with latest release.",
    "args.start.version.comp.neoforge": "Start NeoForge mod loader with latest release.",
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
    "args.start.legacyfabric_prefix": "Change the prefix of the version ID when starting with LegacyFabric (<prefix>-<vanilla-version>-<loader-version>).",
    "args.start.babric_prefix": "Change the prefix of the version ID when starting with Babric (<prefix>-<vanilla-version>-<loader-version>).",
    "args.start.quilt_prefix": "Change the prefix of the version ID when starting with Quilt (<prefix>-<vanilla-version>-<loader-version>).",
    "args.start.forge_prefix": "Change the prefix of the version ID when starting with Forge (<prefix>-<forge-version>).",
    "args.start.neoforge_prefix": "Change the prefix of the version ID when starting with NeoForge (<prefix>-<neoforge-version>).",
    "args.start.lwjgl": "Change the default LWJGL version used by Minecraft (LWJGL >= 3.2.3). "
        "This argument makes additional changes in order to support processor architectures such as ARM. "
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
    "args.start.login": "Use a email (or deprecated username) to authenticate using selected "
        "service (with --auth-service, also overrides --username and --uuid).",
    "args.start.username": "Set a custom user name to play.",
    "args.start.uuid": "Set a custom user UUID to play.",
    "args.start.server": "Start the game and directly connect to a multiplayer server (>= 1.6).",
    "args.start.server_port": "Set the server port (given with -s, --server, >= 1.6).",
    # Args login
    "args.login": "Login into your account and save the session.",
    # Args logout
    "args.logout": "Logout and invalidate a session.",
    # Args show
    "args.show": "Show, debug and generate data unrelated to the game.",
    "args.show.about": "Display authors, version and license of PortableMC.",
    "args.show.auth": "Debug the authentication database and supported services.",
    "args.show.lang": "Debug the language mappings used for messages translation.",
    "args.show.completion": "Print a shell completion script.",
    "args.show.completion._": 
        # Part of this description are from 'rustup' completion description.
        "  This command prints a shell completion script in the terminal.\n"
        "  The installation of this completion script depends on you shell and is explained below.\n\n"
        "  BASH:\n\n"
        "  Completion files are commonly stored in '/etc/bash_completion.d/' for system-wide commands,\n"
        "  but can be stored in '~/.local/share/bash-completion/completions' for user-specific commands.\n"
        "  You can run the following commands to generate the file:\n\n"
        "    $ mkdir -p ~/.local/share/bash-completion/completions\n"
        "    $ portablemc show completion bash > ~/.local/share/bash-completion/completions/portablemc\n\n"
        "  You can also dynamically evaluate the script, but it may slow your shell startup:\n\n"
        "    $ eval \"$(portablemc show completion bash)\"\n\n"
        "  ZSH:\n\n"
        "  Zsh completions are commonly stored in any directory listed in your '$fpath' variable.\n"
        "  To use these completions, you must either add the generated script to one of those\n"
        "  directories, or add your own to this list. Once you chose a '$fpath' directory:\n\n"
        "    $ portablemc show completion zsh > your-dir/_portablemc\n\n"
        "  You can also dynamically evaluate a script, but it may slow your shell startup:\n\n"
        "    $ eval \"$(portablemc show completion zsh)\"\n\n",
    "args.show.completion.shell": "The shell to generate completion script for (default to your current shell, required if not found).",
    "args.show.completion.shell.comp.bash": "Generate completion script for Bash.",
    "args.show.completion.shell.comp.zsh": "Generate completion script for Zsh.",
    # Common
    "echo": "{echo}",
    "cancelled": "Cancelled.",
    "keyboard_interrupt": "Keyboard interrupted.",
    "suggest_verbose": "Use verbose flag -v to get informations that may be useful for developers.",
    # Common errors
    "error.os": "An unexpected OS error happened:",
    "error.http": "Unhandled HTTP error happened:",
    "error.socket": "This operation requires an operational network, but a socket error happened:",
    "error.socket.tip.version_manifest": "Version manifest may not be locally cached, try to run this command once with an operational network.",
    "error.socket.tip.fabric_loader_version": "Fabric loader version must be specified if network is not operational.",
    "error.socket.tip.legacyfabric_loader_version": "Fabric loader version must be specified if network is not operational.",
    "error.socket.tip.quilt_loader_version": "Quilt loader version must be specified if network is not operational.",
    "error.socket.tip.babric_loader_version": "Babric loader version must be specified if network is not operational.",
    "error.cert": "Certificate verification failed, you can try installing 'certifi' package:",
    # Command search
    "search.type": "Type",
    "search.name": "Identifier",
    "search.release_date": "Release date",
    "search.last_modified": "Last modified",
    "search.flags": "Flags",
    "search.flags.local": "local",
    "search.flags.stable": "stable",
    "search.loader_version": "Loader version",
    # Command login
    "login.tip.remember_start_login": "Remember to start the game with '-l {email}' if you want to be authenticated in-game.",
    # Command logout
    "logout.yggdrasil.pending": "Logging out {email} from Mojang...",
    "logout.microsoft.pending": "Logging out {email} from Microsoft...",
    "logout.success": "Logged out {email}",
    "logout.unknown_session": "No session for {email}",
    # Command start
    "start.global_version": "Global version: {kind} {version} {remaining}",
    "start.version.invalid_id": "Invalid version id, expected: {expected}",
    "start.version.invalid_id_unknown_kind": "Invalid version id, unknown kind: {kind}.",
    "start.version.loading": "Loading version {version}... ",
    "start.version.fetching": "Fetching version {version}... ",
    "start.version.loaded": "Loaded version {version}",
    "start.version.loaded.fetched": "Loaded version {version} (fetched)",
    "start.version.not_found": "Version {version} not found",
    "start.version.too_much_parents": "Too much parents while resolving versions.",
    "start.features": "Features: [{features}]",
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
    "start.forge.resolving": "Resolving {api} alias {version}...",
    "start.forge.resolved": "Resolved {api} {version}",
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
    "auth.logged_in": "Session logged for {email}",
    "auth.error": "Error authenticating: {message}",
    # Auth Yggdrasil
    "auth.yggdrasil": "Authenticating {email} with Mojang...",
    "auth.yggdrasil.enter_password": "Password: ",
    "auth.yggdrasil.deprecated": "Mojang authentication is deprecated and does not work anymore.",
    # Auth Microsoft
    "auth.microsoft": "Authenticating {email} with Microsoft...",
    "auth.microsoft.no_browser_fallback": "Authenticating without local browser, please go to the following url to login:",
    "auth.microsoft.no_browser_code": "Paste the code: ",
    "auth.microsoft.opening_browser_and_listening": "Opened authentication page in browser...",
    "auth.microsoft.close_tab_and_return": "Close this tab and return to the launcher.",
    "auth.microsoft.failed_to_authenticate": "Failed to authenticate.",
    "auth.microsoft.processing": "Processing authentication against Minecraft services...",
    "auth.microsoft.incoherent_data": "Incoherent authentication data, please retry.",
}
