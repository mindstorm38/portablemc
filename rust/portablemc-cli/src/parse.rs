//! Implementation of the command line parser, using clap struct derivation.

use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args, Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use portablemc::{fabric, forge};


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
    /// If left unspecified, this argument defaults to the standard Minecraft directory
    /// for your system: in '%USERPROFILE%/AppData/Roaming' on Windows, 
    /// '$HOME/Library/Application Support/minecraft' on macOS and '$HOME/.minecraft' on
    /// other systems. If the launcher fails to find the default directory then it will
    /// abort any command exit with a failure telling you to specify it.
    /// 
    /// This argument might not always be used by a command, you can specify it through
    /// environment variables if more practical.
    #[arg(long, env = "PMC_MAIN_DIR", value_name = "PATH")]
    pub main_dir: Option<PathBuf>,
    /// Set the path to the Microsoft Authentication database for caching session tokens.
    /// 
    /// When unspecified, this argument is derived from the '--main-dir' path: 
    /// '<main-dir>/portablemc_msa.json'. This file uses a JSON human-readable format.
    /// 
    /// This argument might not always be used by a command, you can specify it through
    /// environment variables if more practical.
    #[arg(long, env = "PMC_MSA_DB_FILE", value_name = "PATH")]
    pub msa_db_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum CliCmd {
    Start(StartArgs),
    Search(SearchArgs),
    Auth(AuthArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliOutput {
    /// Human readable output, it depends on the actual command being used and is not
    /// guaranteed to be stable across releases, for that you should prefer using
    /// 'tabular' output for example. With this format, the verbosity is used to
    /// show more informative data.
    Human,
    /// Machine output mode to allow parsing by other programs, using tab ('\t', 0x09) 
    /// separated values where the first value defines which kind of data to follow on 
    /// the line, a line return ('\n', 0x0A) is used to split every line. If any line 
    /// return or tab is encoded into a value within the line, it is escaped with the 
    /// two characters '\n' (for line return) or '\t' (for tab), these are the only two
    /// escapes used. This mode is always verbose, and verbosity will not have any effect 
    /// on it. If the launcher exit with a failure code, you should expect finding a log 
    /// message prefixed with `error_`, describing the error(s) causing the exit.
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
    /// - mojang:[release|snapshot|<version>] => use 'release' (default if absent) or 
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
    /// - fabric:[<game-version>[:[<loader-version>]]] => install and launch a given 
    /// Mojang version with the Fabric mod loader. Both versions can be omitted (empty) 
    /// to use the latest stable versions available, but you can also manually specify 
    /// 'stable' or 'unstable', which are equivalent as Mojang release and snapshot
    /// but at the discretion of the Fabric API. If the version is not yet installed, 
    /// it will requires internet to access the Fabric API. See https://fabricmc.net/.
    /// 
    /// - quilt:[<game-version>[:[<loader-version>]]] => same as 'fabric' installer, 
    /// but using the Quilt API for missing versions. See https://quiltmc.org/.
    /// 
    /// - legacyfabric:[<game-version>[:[<loader-version>]]] => same as 'fabric',
    /// but using the LegacyFabric API for missing versions. This installer can be
    /// used for using the Fabric mod loader on Mojang versions prior to 1.14. 
    /// See https://legacyfabric.net/.
    /// 
    /// - babric:[:[<loader-version>]] => same as 'fabric', but using the Babric API 
    /// for missing versions. This mod loader is specifically made to support Fabric
    /// only on Mojang b1.7.3, so it's useless to specify the game version like 
    /// other Fabric-like loaders, both 'stable' and 'unstable' would be equivalent 
    /// to 'b1.7.3'. See https://babric.github.io/.
    /// 
    /// - forge::<loader-version> | forge:[<game-version>][:stable|unstable] => the 
    /// syntax is a bit cumbersome because you can either specify the full loader version
    /// such as '1.21.4-54.0.12' but you must leave the first parameter empty, or you 
    /// can specify a Mojang game version with optional second parameter that specifies
    /// if you target the latest 'stable' (default) or 'unstable' loader version.
    /// See https://minecraftforge.net/.
    /// 
    /// - neoforge::<loader-version> | neoforge:[<game-version>][:stable|unstable] => same
    /// as 'forge', but using the NeoForge repository. See https://neoforged.net/.
    #[arg(default_value = "release")]
    pub version: StartVersion,
    /// Only ensures that the game is installed but don't launch the game. This can be 
    /// used to debug installation paths while using verbose output.
    #[arg(long)]
    pub dry: bool,

    // /// Set the versions directory where all version and their metadata are stored.
    // /// 
    // /// This is applied after --main-dir has been applied.
    // #[arg(long, env = "PMC_VERSIONS_DIR", value_name = "PATH")]
    // pub versions_dir: Option<PathBuf>,
    // /// Set the libraries directory where all Java libraries are stored.
    // /// 
    // /// This is applied after --main-dir has been applied.
    // #[arg(long, env = "PMC_LIBRARIES_DIR", value_name = "PATH")]
    // pub libraries_dir: Option<PathBuf>,
    // /// Set the assets directory where all game assets are stored.
    // /// 
    // /// This is applied after --main-dir has been applied.
    // #[arg(long, env = "PMC_ASSETS_DIR", value_name = "PATH")]
    // pub assets_dir: Option<PathBuf>,
    // /// Set the JVM directory where Mojang's Java Virtual Machines are stored if needed.
    // /// 
    // /// This is applied after --main-dir has been applied.
    // #[arg(long, env = "PMC_JVM_DIR", value_name = "PATH")]
    // pub jvm_dir: Option<PathBuf>,

    /// Set the binaries directory where all binary objects are extracted before running
    /// the game, a sub-directory is created inside this directory that is uniquely named
    /// after a hash of the version's libraries.
    /// 
    /// When unspecified, this argument is derived from the '--main-dir' path: 
    /// '<main-dir>/bin/'.
    /// 
    /// This argument might not always be used by a command, you can specify it through
    /// environment variables if more practical.
    #[arg(long, env = "PMC_BIN_DIR", value_name = "PATH")]
    pub bin_dir: Option<PathBuf>,
    /// Set the directory where the game is run from, the game will use this directory
    /// to put options, saves, screenshots and access texture or resource packs and any
    /// other user related stuff.
    /// 
    /// When unspecified, this argument is equal to the '--main-dir' path.
    /// 
    /// This argument might not always be used by a command, you can specify it through
    /// environment variables if more practical.
    #[arg(long, env = "PMC_MC_DIR", value_name = "PATH")]
    pub mc_dir: Option<PathBuf>,
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
    /// The path to the JVM executable, 'java' (or 'javaw.exe' on Windows).
    /// 
    /// This is used to launch the game, it has a special use-case with Forge and NeoForge
    /// loader versions where that JVM executable is also used to run the installer 
    /// processors.
    /// 
    /// Note that when this argument is specified, you cannot specify the '--jvm-policy'
    /// argument.
    #[arg(long, value_name = "PATH")]
    pub jvm: Option<String>,
    /// The policy for finding or installing the JVM executable.
    #[arg(long, value_name = "POLICY", conflicts_with = "jvm", default_value = "system-mojang")]
    pub jvm_policy: StartJvmPolicy,
    /// Enable authentication for the username or UUID.
    /// 
    /// When enabled, the launcher will look for specified '--uuid', or '--username' as
    /// a fallback, it will then pick the matching account and start the game with it,
    /// the account is refreshed if needed. IT MEANS that you must first login into
    /// your account using the 'portablemc auth' command before starting the game with
    /// the account.
    /// 
    /// If the account is not found, the launcher won't start the game and will show an
    /// error.
    /// 
    /// Note that '--username' (-u) argument is completely ignored if the '--uuid' (-i)
    /// is specified, only one of them can be used at the same time with this flag.
    #[arg(short = 'a', long)]
    pub auth: bool,
    /// Change the default username of the player.
    /// 
    /// When the '--auth' (-a) is enabled, this argument is used, after the '--uuid' (-i)
    /// one, to find the authenticated account to start the game with.
    #[arg(short = 'u', long, value_name = "NAME")]
    pub username: Option<String>,
    /// Change the default UUID of the player.
    /// 
    /// When the '--auth' (-a) is enabled, this argument is used, before the '--username' 
    /// (-u) one, to find the authenticated account to start the game with.
    #[arg(short = 'i', long)]
    pub uuid: Option<Uuid>,

    // /// Immediately tries to connect the given server once the game is started (>= 1.6).
    // /// 
    // /// Note that the client will still be able to disconnect from the server and go back
    // /// to the game's menu and do everything it want.
    // #[arg(short, long)]
    // pub server: Option<String>,
    // /// Change the default port to connect to the server (--server).
    // #[arg(short = 'p', long, value_name = "PORT", requires = "server", default_value_t = 25565)]
    // pub server_port: u16,
}

/// Represent all possible version the launcher can start.
#[derive(Debug, Clone)]
pub enum StartVersion {
    Mojang {
        version: String,
    },
    MojangRelease,
    MojangSnapshot,
    Fabric {
        loader: fabric::Loader,
        game_version: fabric::GameVersion,
        loader_version: fabric::LoaderVersion,
    },
    Forge {
        loader: forge::Loader,
        version: String,
    },
    ForgeLatest {
        loader: forge::Loader,
        game_version: Option<String>,  // None for targeting "release"
        stable: bool,
    }
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
            return Err(format!("too much parameters for this installer kind"));
        }

        let version = match kind {
            "mojang" => {
                match parts[0] {
                    "" | 
                    "release" => Self::MojangRelease {  },
                    "snapshot" => Self::MojangSnapshot {  },
                    version => Self::Mojang { version: version.to_string() },
                }
            }
            "fabric" | "quilt" | "legacyfabric" | "babric" => {
                Self::Fabric { 
                    loader: match kind {
                        "fabric" => fabric::Loader::Fabric,
                        "quilt" => fabric::Loader::Quilt,
                        "legacyfabric" => fabric::Loader::LegacyFabric,
                        "babric" => fabric::Loader::Babric,
                        _ => unreachable!(),
                    },
                    game_version: match parts[0] {
                        "" |
                        "stable" => fabric::GameVersion::Stable,
                        "unstable" => fabric::GameVersion::Unstable,
                        id => fabric::GameVersion::Name(id.to_string()),
                    },
                    loader_version: match parts.get(1).copied() {
                        None | Some("" | "stable") => fabric::LoaderVersion::Stable,
                        Some("unstable") => fabric::LoaderVersion::Unstable,
                        Some(id) => fabric::LoaderVersion::Name(id.to_string()),
                    },
                }
            }
            "forge" | "neoforge" => {

                let loader = match kind {
                    "forge" => forge::Loader::Forge,
                    "neoforge" => forge::Loader::NeoForge,
                    _ => unreachable!(),
                };

                match parts.get(1).copied() {
                    None | 
                    Some("" | "stable" | "unstable") => {
                        Self::ForgeLatest { 
                            loader, 
                            game_version: match parts[0] {
                                "" | "release" => None,
                                id => Some(id.to_string()),
                            }, 
                            stable: match parts.get(1).copied() {
                                None | Some("" | "stable") => true,
                                Some("unstable") => false,
                                _ => unreachable!(),
                            },
                        }
                    }
                    Some(other) => {

                        if !parts[0].is_empty() {
                            return Err(format!("first parameter should be empty when specifying full loader version"));
                        }

                        Self::Forge { 
                            loader, 
                            version: other.to_string(),
                        }
                        
                    }
                }

            }
            _ => unreachable!()
        };

        Ok(version)

    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum StartJvmPolicy {
    /// The installer will try to find a suitable JVM executable in the path, searching
    /// a `java` (or `javaw.exe` on Windows) executable. On operating systems where it's
    /// supported, this will also check for known directories (on Arch for example).
    /// If the version needs a specific JVM major version, each candidate executable is 
    /// checked and a warning is triggered to notify that the version is not suited.
    /// The install fails if none of those versions is valid.
    System,
    /// The installer will try to find a suitable JVM to install from Mojang-provided
    /// distributions, if no JVM is available for the platform and for the required 
    /// distribution then the install fails.
    Mojang,
    /// The installer search system and then mojang as a fallback.
    SystemMojang,
    /// The installer search Mojang and then system as a fallback.
    MojangSystem,
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
    /// Select the target of the search query.
    #[arg(short, long, default_value = "mojang")]
    pub kind: SearchKind,
    /// The search filter string.
    /// 
    /// You can give multiple filters that will apply to various texts depending on the 
    /// search king. In general this will apply to the leftmost column, so the version
    /// name in most of the cases.
    pub filter: Vec<String>,
    /// Only keep versions of given channel.
    /// 
    /// This argument can be given multiple times to specify multiple channels to match
    /// in an OR logic.
    /// 
    /// [supported search kinds: mojang, forge, neoforge]
    #[arg(long)]
    pub channel: Vec<SearchChannel>,
    /// Only show the latest version of the given channel.
    /// 
    /// This argument can be specified only once and is incompatible with any other
    /// filters, it also don't work with any channel, some channels have no information 
    /// about their "latest" version as it doesn't make sense, like the latest Mojang's 
    /// beta was released 13 years ago, this cannot be described as the "latest" version
    /// of the game.
    /// 
    /// [supported search kinds: mojang]
    #[arg(long, conflicts_with_all = ["filter", "channel"])]
    pub latest: Option<SearchLatestChannel>,
    /// Only keep loader versions that targets the given game version. 
    /// 
    /// [supported search kinds: forge, neoforge]
    #[arg(long)]
    pub game_version: Vec<String>,
}

impl SearchArgs {

    /// Return true if the given haystack contains one of the string filters. Return true
    /// if no string filter.
    pub fn match_filter(&self, haystack: &str) -> bool {
        self.filter.is_empty() || self.filter.iter().any(|s| haystack.contains(s))
    }

    /// Return true if the given search channel is selected. Return true if no filter.
    pub fn match_channel(&self, channel: SearchChannel) -> bool {
        self.channel.is_empty() || self.channel.contains(&channel)
    }

    /// Return true if the given game version is present, exactly, in one of the filter.
    /// Return true if no filter.
    pub fn match_game_version(&self, game_version: &str) -> bool {
        self.game_version.is_empty() || self.game_version.iter().any(|v| v == game_version)
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum SearchKind {
    /// Search for official versions released by Mojang, including release and snapshots.
    Mojang,
    /// Search for locally installed versions, located in the versions directory.
    Local,
    /// Search for Fabric loader versions.
    Fabric,
    /// Search for Fabric supported game versions.
    FabricGame,
    /// Search for Quilt loader versions.
    Quilt,
    /// Search for Quilt supported game versions.
    QuiltGame,
    /// Search for LegacyFabric loader versions.
    Legacyfabric,
    /// Search for LegacyFabric supported game versions.
    LegacyfabricGame,
    /// Search for Babric loader versions.
    Babric,
    /// Search for Babric supported game versions.
    BabricGame,
    /// Search for Forge loader versions.
    Forge,
    /// Search for NeoForge loader versions.
    NeoForge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SearchChannel {
    /// Filter versions by release channel (only for mojang).
    Release,
    /// Filter versions by snapshot channel (only for mojang).
    Snapshot,
    /// Filter versions by beta channel (only for mojang).
    Beta,
    /// Filter versions by alpha channel (only for mojang).
    Alpha,
    /// Filter versions by stable channel (only for mod loaders).
    Stable,
    /// Filter versions by unstable channel (only for mod loaders).
    Unstable,
}

impl SearchChannel {

    pub fn new_stable_or_unstable(stable: bool) -> Self {
        if stable { 
            SearchChannel::Stable 
        } else {
            SearchChannel::Unstable
        }
    }
    
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SearchLatestChannel {
    /// Select the latest release version.
    Release,
    /// Select the latest snapshot version.
    Snapshot,
}

// ================= //
//   AUTH COMMAND    //
// ================= //

/// Manage the authentication sessions.
/// 
/// By default, this command will start a new authentication flow with the Microsoft
/// authentication service, when completed this will add the newly authenticated session
/// to the authentication database (specified with '--msa-db-file' argument).
/// 
/// If this command fails to load and/or store the database, its exit code is 1 (failure).
#[derive(Debug, Args)]
pub struct AuthArgs {
    /// Prevent the launcher from opening your system's web browser with the 
    /// authentication page.
    /// 
    /// When the '--output' mode is 'human', the launcher will try to open your system's
    /// web browser with the Microsoft authentication page, this flag disables this 
    /// behavior.
    #[arg(long)]
    pub no_browser: bool,
    /// Forget the given authenticated session by its UUID, or username as a fallback.
    /// 
    /// You'll no longer be able to authenticate with this session when starting the
    /// game, you'll have to authenticate again. If not account is matching the given
    /// UUID or username, then the database is not rewritten, and a warning message is
    /// issued, but the exit code is always 0 (success).
    #[arg(short, long, exclusive = true)]
    pub forget: Option<String>,
    /// Refresh the given account, updating the username if it has been modified.
    /// 
    /// If the profile cannot be refreshed, a request for refreshing
    /// 
    /// Note that this procedure is automatically done on game's start, so you don't need 
    /// to run this before starting the game with an account. You may want to use this 
    /// in order to update the database and list the updated accounts.
    #[arg(short, long, exclusive = true)]
    pub refresh: Option<String>,
    /// List all currently authenticated sessions, by username and UUID, that can be used
    /// with the start command to authenticate.
    #[arg(short, long, exclusive = true)]
    pub list: bool,
}
