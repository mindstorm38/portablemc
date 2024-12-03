//! Implementation of the command line parser, using clap struct derivation.

use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args, Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use portablemc::mojang::Root;


// ================= //
//    MAIN COMMAND   //
// ================= //

/// Command line utility for launching Minecraft quickly and reliably with included 
/// support for Mojang versions and popular mod loaders.
#[derive(Debug, Parser)]
#[command(name = "portablemc", version, author, disable_help_subcommand = true, max_term_width = 140)]
pub struct CliArgs {
    #[command(subcommand)]
    pub cmd: CliCmd,
    /// Enable verbose output, the more -v argument you put, the more verbose the
    /// launcher will be.
    #[arg(short, env = "PMC_VERBOSE", action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Change the default output format of the launcher.
    #[arg(long, env = "PMC_OUTPUT", default_value = "human")]
    pub output: CliOutput,
    /// Set the directory where versions, libraries, assets, JVM and where the game's run.
    /// 
    /// This argument is equivalent to calling: 
    /// --versions-dir <main>/versions
    /// --libraries-dir <main>/libraries
    /// --assets-dir <main>/assets
    /// --jvm-dir <main>/jvm
    /// --bin-dir <main>/bin
    /// --mc-dir <main>
    /// 
    /// If left unspecified, this argument defaults to the standard Minecraft directory
    /// for your system: in '%USERPROFILE%/AppData/Roaming' on Windows, 
    /// '$HOME/Library/Application Support/minecraft' on macOS and `$HOME/.minecraft` on
    /// other systems. If the launcher fails to find the default directory then it will
    /// abort any command exit with a failure telling you to specify it (using argument 
    /// or environment variable).
    #[arg(long, env = "PMC_MAIN_DIR", value_name = "PATH")]
    pub main_dir: Option<PathBuf>,
    /// Set the versions directory where all version and their metadata are stored.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long, env = "PMC_VERSIONS_DIR", value_name = "PATH")]
    pub versions_dir: Option<PathBuf>,
    /// Set the libraries directory where all Java libraries are stored.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long, env = "PMC_LIBRARIES_DIR", value_name = "PATH")]
    pub libraries_dir: Option<PathBuf>,
    /// Set the assets directory where all game assets are stored.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long, env = "PMC_ASSETS_DIR", value_name = "PATH")]
    pub assets_dir: Option<PathBuf>,
    /// Set the JVM directory where Mojang's Java Virtual Machines are stored if needed.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long, env = "PMC_JVM_DIR", value_name = "PATH")]
    pub jvm_dir: Option<PathBuf>,
    /// Set the binaries directory where all binary objects are extracted before running
    /// the game.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long, env = "PMC_BIN_DIR", value_name = "PATH")]
    pub bin_dir: Option<PathBuf>,
    /// Set the directory where the game is run from, the game will use this directory
    /// to put options, saves, screenshots and access texture or resource packs and any
    /// other user related stuff.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long, env = "PMC_MC_DIR", value_name = "PATH")]
    pub mc_dir: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum CliCmd {
    Start(StartArgs),
    Search(SearchArgs),
    Info(InfoArgs),
    Login(LoginArgs),
    Logout(LogoutArgs),
    Show(ShowArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliOutput {
    /// Human readable output, it depends on the actual command being used and is not
    /// guaranteed to be stable across releases, for that you should prefer using
    /// 'tabular' output for example. With this format, the verbosity is used to
    /// show more informative data.
    Human,
    /// Machine output mode to allow parsing by other programs, using tab separated 
    /// values where the first value defines which kind of data to follow, so that the
    /// program reading this output will be able to properly interpret the following
    /// values, a line return is use to split every line. This mode is always verbose,
    /// and verbose will not have any effect on it. If the launcher exit with a failure
    /// code, you should expect finding a log message prefixed with `error_`, describing
    /// the error(s) causing the exit.
    Machine,
}

// ================= //
//   START COMMAND   //
// ================= //

/// Start the game.
/// 
/// This command is the main entrypoint for installing and then launching the game,
/// it works with many different versions, this includes official Mojang versions 
/// but also popular mod loaders, such as Fabric, Quilt, Forge, NeoForge and 
/// LegacyFabric. It ensures that the version is properly installed prior to launching
/// it.
#[derive(Debug, Args)]
pub struct StartArgs {
    /// The version to launch (see more with '--help').
    /// 
    /// You can provide this argument with colon-separated ':' syntax, in such case the
    /// first part defines the kind of installer, supported values are: mojang, fabric,
    /// quilt, forge, neoforge, legacyfabric and babric. 
    /// When not using the colon-separated syntax, this will defaults to the 'mojang' 
    /// installer. Below are detailed each installer.
    /// 
    /// - mojang:[release|snapshot|<version>] => use 'release' (default is absent) or 
    /// 'snapshot' to install and launch the latest version of that type, or you can 
    /// use any valid version id provided by Mojang (you can search for them using the 
    /// 'portablemc search' command). 
    /// This also supports any local version that is already installed, with support 
    /// for inheriting other versions: a generic rule of the Mojang installer is that
    /// each version in the hierarchy that is known by the Mojang's version manifest 
    /// will be checked for validity (file hash) and fetched if needed.
    /// You can manually exclude versions from this rule using '--exclude-fetch' with
    /// each version you don't want to fetch (see this argument's help). The Mojang's
    /// version manifest is only accessed for resolving 'release', 'snapshot' or a
    /// non-excluded version, if it's not yet cached this will require internet.
    /// 
    /// - fabric:[<mojang-version>[:[<loader-version>]]] => install and launch a given 
    /// mojang version with the Fabric mod loader. Both versions can be omitted (empty) 
    /// to use the latest versions available, for Mojang version you can use 'release' 
    /// or 'snapshot' like the 'mojang'. If the version is not yet installed, it will
    /// requires internet to access the Fabric API. See https://fabricmc.net/.
    /// 
    /// - quilt:[<mojang-version>[:[<loader-version>]]] => same as 'fabric' installer, 
    /// but using the Quilt API for missing versions.
    /// 
    /// - legacyfabric:[<mojang-version>[:[<loader-version>]]] => same as 'fabric',
    /// but using the LegacyFabric API for missing versions. This installer can be
    /// used for using the Fabric mod loader on Mojang versions prior to 1.14, therefore
    /// it will treat 'release' and 'snapshot' as the latest Mojang release or snapshot
    /// supported by LegacyFabric. See https://legacyfabric.net/.
    /// 
    /// - babric:[:[<loader-version>]] => same as 'fabric', but using the Babric API 
    /// for missing versions. This mod loader is specifically made to support Fabric
    /// only on Mojang's b1.7.3, so it's useless to specify the Mojang version like 
    /// other Fabric-like loaders, the 'release' and 'snapshot' would be equivalent 
    /// to 'b1.7.3'. See https://babric.github.io/.
    /// 
    /// - raw:<version>
    #[arg(default_value = "release")]
    pub version: StartVersion,
    /// Only ensures that the game is installed but don't launch the game.
    #[arg(long)]
    pub dry: bool,
    /// Disable the multiplayer buttons (>= 1.16).
    #[arg(long)]
    pub disable_multiplayer: bool,
    /// Disable the online chat (>= 1.16).
    #[arg(long)]
    pub disable_chat: bool,
    /// Enable demo mode for the game.
    #[arg(long)]
    pub demo: bool,
    /// Change the resolution of the game window (<width>x<height>, >= 1.6).
    #[arg(long)]
    pub resolution: Option<StartResolution>,
    /// Change the LWJGL version used by the game (LWJGL >= 3.2.3).
    /// 
    /// This argument will cause all LWJGL libraries of the game to be changed to the
    /// given version, this applies to natives as well. In addition to simply changing
    /// the versions, this will also add natives that are missing, such as ARM.
    /// 
    /// It's not guaranteed to work with every version of Minecraft and downgrading 
    /// LWJGL version is not recommended.
    #[arg(long, value_name = "VERSION")]
    pub lwjgl: Option<String>,
    /// Exclude the given version from validity check and fetch.
    /// 
    /// This is used by the Mojang installer and all installers relying on it to exclude
    /// version from being validated and fetched from the Mojang's version manifest, as
    /// described in 'VERSION' help. You can use '*' a single time to fully disable 
    /// fetching.
    /// 
    /// This argument can be specified multiple times.
    #[arg(long, value_name = "VERSION")]
    pub exclude_fetch: Vec<String>,
    /// Use a filter to exclude Java libraries from the installation.
    /// 
    /// The filter is checked against each GAV (Group-Artifact-Version) of each library
    /// resolved in the version metadata and remove each library matching the filter.
    /// It's using the following syntax, requiring the artifact name and optionally
    /// the version and the classifier: <artifact>[:[<version>][:<classifier>]].
    /// 
    /// A typical use case for this argument would be to exclude some natives-providing
    /// library (such as LWJGL libraries with 'natives' classifier) and then provide 
    /// those natives manually using '--include-bin' argument. Known usage of this 
    /// argument has been for supporting MUSL-only systems, because LWJGL binaries are
    /// only provided for glibc (see #110 and #112 on GitHub).
    /// 
    /// This argument can be specified multiple times.
    #[arg(long, value_name = "FILTER")]
    pub exclude_lib: Vec<String>,  // TODO: Use a specific type.
    /// Include files in the binaries directory, usually shared objects.
    /// 
    /// Those files are symlinked (or copied if not possible) to the binaries directory 
    /// where the game will check for natives to load. The main use case is for including
    /// shared objects (.so, .dll, .dylib), in case of versioned .so files like we can
    /// see on UNIX systems, the version is discarded when linked or copied to the bin
    /// directory (/usr/lib/foo.so.1.22.2 -> foo.so).
    /// 
    /// Read the help message of '--exclude-lib' for a typical use case.
    /// 
    /// This argument can be specified multiple times.
    #[arg(long, value_name = "PATH")]
    pub include_bin: Vec<PathBuf>,
    /// Authentication common arguments.
    #[command(flatten)]
    pub auth_common: LoginArgs,
    /// Anonymize your email or username when writing it on the output.
    #[arg(long)]
    pub auth_anonymize: bool,
    /// Prevents the launcher from using the session cache for login.
    #[arg(long, requires = "login")]
    pub temp_login: bool,
    /// Authenticate into an online session.
    /// 
    /// This conflicts with both '--username' or `--uuid` arguments.
    #[arg(short, long, env = "PMC_START_LOGIN", conflicts_with_all = ["username", "uuid"])]
    pub login: Option<String>,
    /// Change the default username of the player, for offline-mode.
    #[arg(short, long, value_name = "NAME")]
    pub username: Option<String>,
    /// Change the default UUID of the player, for offline-mode.
    #[arg(short = 'i', long)]
    pub uuid: Option<Uuid>,
    /// Immediately tries to connect the given server once the game is started (>= 1.6).
    /// 
    /// Note that the client will still be able to disconnect from the server and go back
    /// to the game's menu and do everything it want.
    #[arg(short, long)]
    pub server: Option<String>,
    /// Change the default port to connect to the server (--server).
    #[arg(short = 'p', long, value_name = "PORT", requires = "server", default_value_t = 25565)]
    pub server_port: u16,
}

/// Represent all possible version the launcher can start.
#[derive(Debug, Clone)]
pub enum StartVersion {
    Raw {
        root: String,
    },
    Mojang {
        root: Root,
    },
    Loader {
        root: Root,
        loader: Option<String>,
        kind: StartLoaderKind,
    },
}

#[derive(Debug, Clone)]
pub enum StartLoaderKind {
    Fabric,
    Quilt,
    LegacyFabric,
    Babric,
    Forge,
    NeoForge,
}

impl FromStr for StartVersion {

    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        
        // Extract the kind (defaults to mojang) and the parameters.
        let (kind, rest) = s.split_once(':')
            .unwrap_or(("mojang", s));

        // Then split the rest into all parts.
        let parts = rest.split(':').collect::<Vec<_>>();
        debug_assert!(!parts.is_empty());

        // Compute max parts count and immediately discard 
        let max_parts = match kind {
            "raw" => 1,
            "mojang" => 1,
            "fabric" | "quilt" | "legacyfabric" | "babric" => 2,
            "forge" | "neoforge" => 2,
            _ => return Err(format!("unknown installer kind: {kind}")),
        };

        if parts.len() > max_parts {
            return Err(format!("too much colons for this installer kind"));
        }

        // Raw version have no alias.
        if kind == "raw" {
            return Ok(Self::Raw { root: parts[0].to_string() });
        }

        // Most versions use the first part as the Mojang's version, and there is always
        // at least one part.
        let root = match parts[0] {
            "" | 
            "release" => Root::Release,
            "snapshot" => Root::Snapshot,
            id => Root::Id(id.to_string()),
        };

        let version = match kind {
            "mojang" => Self::Mojang { root },
            "fabric" | "quilt" | "legacyfabric" | "babric" | "forge" | "neoforge" => {
                Self::Loader { 
                    root, 
                    loader: parts.get(1).copied().map(str::to_string).filter(|s| !s.is_empty()),
                    kind: match kind {
                        "fabric" => StartLoaderKind::Fabric,
                        "quilt" => StartLoaderKind::Quilt,
                        "legacyfabric" => StartLoaderKind::LegacyFabric,
                        "babric" => StartLoaderKind::Babric,
                        "forge" => StartLoaderKind::Forge,
                        "neoforge" => StartLoaderKind::NeoForge,
                        _ => unreachable!(),
                    },
                }
            }
            _ => unreachable!()
        };

        Ok(version)

    }

}

/// Represent an optional initial resolution for the game window.
#[derive(Debug, Clone, Copy)]
pub struct StartResolution {
    pub width: u16,
    pub height: u16,
}

impl FromStr for StartResolution {

    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        
        let Some((width, height)) = s.split_once('x') else {
            return Err(format!("invalid resolution syntax, expecting <width>x<height>"))
        };

        Ok(Self {
            width: width.parse().map_err(|e| format!("invalid resolution width: {e}"))?,
            height: height.parse().map_err(|e| format!("invalid resolution height: {e}"))?,
        })

    }

}

// ================= //
//  SEARCH COMMAND   //
// ================= //

/// Search for versions.
/// 
/// By default this command will search for official Mojang version but you can change 
/// this behavior and search for local or mod loaders versions with the -k (--kind) 
/// argument. Note that the displayed table layout depends on the kind. How the
/// query string is interpreted depends on the kind.
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// The search query string.
    /// 
    /// Its syntax allows giving multiple space-separated words (quoted arguments are not
    /// split), then if a word contains a colon ':' then it is split in a parameter and
    /// its value, the parameter and its value are then interpreted depending on the 
    /// search kind. If a word if not of parameter:value syntax then it's interpreted
    /// depending on the search kind, for example to filter version name. Multiple
    /// different parameters acts in a AND logic, but giving multiple times the same
    /// parameters acts in a OR logic. 
    /// 
    /// For example when searching for Mojang versions, the query '1.3 1.4 type:release 
    /// type:snapshot', all versions containing '1.3' OR '1.4' in their id AND of type
    /// 'release' or 'snapshot' will be displayed.
    pub query: Vec<String>,
    /// Select the target of the search query.
    #[arg(short, default_value = "mojang")]
    pub kind: SearchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SearchKind {
    /// Search for official versions released by Mojang, including release and snapshots.
    /// A query word is used to filter versions' identifiers. Supported parameters are 
    /// 'type:<release|snapshot|beta|alpha>' for filtering by version type, 'release:'
    /// to show only the latest release and 'snapshot:' to show only the latest snapshot
    /// (these last two overrides any other query).
    Mojang,
    /// Search for locally installed versions, located in the versions directory.
    Local,
    /// Search for Forge loader versions.
    Forge,
    /// Search for Fabric loader versions.
    Fabric,
    /// Search for Quilt loader versions.
    Quilt,
    /// Search for LegacyFabric loader versions.
    LegacyFabric,
}

// ================= //
//   INFO COMMAND    //
// ================= //

/// Get informations about a version.
#[derive(Debug, Args)]
pub struct InfoArgs {

}

// ================= //
//   LOGIN COMMAND   //
// ================= //

/// Login into your account and save the session.
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Authentication service.
    #[arg(long, name = "SERVICE", default_value = "microsoft")]
    pub auth_service: AuthService,
    /// Use an alternative authentication flow that avoids opening you web browser.
    #[arg(long)]
    pub auth_no_browser: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AuthService {
    /// Microsoft authentication for Minecraft.
    Microsoft,
}

// ================= //
//  LOGOUT COMMAND   //
// ================= //

/// Logout and invalidate a saved session.
#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// Authentication common arguments.
    #[command(flatten)]
    pub auth_common: LoginArgs,
}

// ================= //
//   SHOW COMMAND    //
// ================= //

/// Show and debug various informations.
#[derive(Debug, Args)]
pub struct ShowArgs {
    #[command(subcommand)]
    pub cmd: ShowCmd,
}

#[derive(Debug, Subcommand)]
pub enum ShowCmd {
    About,
    Auth,
    Completion,
}
