//! Implementation of the command line parser, using clap struct derivation.

use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args, Parser, Subcommand, ValueEnum};
use uuid::Uuid;


// ================= //
//    MAIN COMMAND   //
// ================= //

/// Command line utility for launching Minecraft quickly and reliably with included 
/// support for Mojang versions and popular mod loaders.
#[derive(Debug, Parser)]
#[command(name = "portablemc", version, author, disable_help_subcommand = true, max_term_width = 140)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
    /// Enable verbose output, the more -v argument you put, the more verbose the
    /// launcher will be.
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Change the default output format of the launcher.
    #[arg(long)]
    pub output: Option<Output>,
    /// Set the directory where versions, libraries, assets, JVM and where the game is run.
    /// 
    /// This argument is equivalent to calling: 
    /// --versions-dir <main>/versions
    /// --libraries-dir <main>/libraries
    /// --assets-dir <main>/assets
    /// --jvm-dir <main>/jvm
    /// --bin-dir <main>/bin
    /// --work-dir <main>
    /// 
    /// This argument defaults the standard Minecraft directory on your system.
    #[arg(long)]
    pub main_dir: Option<PathBuf>,
    /// Set the versions directory where all version and their metadata are stored.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long)]
    pub versions_dir: Option<PathBuf>,
    /// Set the libraries directory where all Java libraries are stored.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long)]
    pub libraries_dir: Option<PathBuf>,
    /// Set the assets directory where all game assets are stored.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long)]
    pub assets_dir: Option<PathBuf>,
    /// Set the JVM directory where Mojang's Java Virtual Machines are stored if needed.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long)]
    pub jvm_dir: Option<PathBuf>,
    /// Set the binaries directory where all binary objects are extracted before running
    /// the game.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long)]
    pub bin_dir: Option<PathBuf>,
    /// Set the directory where the game is run from, the game will use this directory
    /// to put options, saves, screenshots and access texture or resource packs and any
    /// other user related stuff.
    /// 
    /// This is applied after --main-dir has been applied.
    #[arg(long)]
    pub work_dir: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    Search(SearchArgs),
    Start(StartArgs),
    Login(LoginArgs),
    Logout(LogoutArgs),
    Show(ShowArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Output {
    /// Human-readable output with color to improve readability, it's the default if 
    /// stdout is detected as a terminal.
    HumanColor,
    /// Human-readable output, it's the default if stdout is not detected as a terminal.
    Human,
    /// Machine output mode to allow parsing by other programs.
    Machine,
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
    pub query: Option<String>,
    /// Select the target of the search query.
    #[arg(short, default_value = "mojang")]
    pub kind: SearchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SearchKind {
    /// Search for official versions released by Mojang, including release and snapshots.
    /// With this kind of search you can give the special names 'release' or 'snapshot'
    /// to only return the latest release or snapshot name version.
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
    /// quilt, forge, neoforge and legacyfabric. When not using the colon-separated 
    /// syntax, this will defaults to the mojang installer. Below are detailed each 
    /// installer.
    /// 
    /// - mojang:[release|snapshot|<version>]: use 'release' (default is absent) or 
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
    /// non-excluded version, if it's not yet cached this will require internet access.
    /// 
    /// - fabric:[<mojang-version>[:<loader-version>]]: install and launch a fabric
    /// version
    /// 
    /// - quilt:[<mojang-version>[:<loader-version>]]: same as fabric, but for Quilt.
    #[arg(default_value = "release")]
    pub version: String,
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
    pub resolution: Option<Resolution>,
    /// Exclude the given version from validity check and fetch.
    /// 
    /// This is used by the Mojang installer and all installers relying on it to exclude
    /// version from being validated and fetched from the Mojang's version manifest, as
    /// described in 'VERSION' help.
    /// 
    /// This argument can be specified multiple times.
    #[arg(long, name = "VERSION")]
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
    #[arg(long, name = "FILTER")]
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
    #[arg(long, name = "PATH")]
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
    #[arg(short, long, conflicts_with_all = ["username", "uuid"])]
    pub login: Option<String>,
    /// Change the default username of the player, for offline-mode.
    #[arg(short, long)]
    pub username: Option<String>,
    /// Change the default UUID of the player, for offline-mode.
    #[arg(short = 'i', long)]
    pub uuid: Option<Uuid>,
    /// Immediately tries to connect the given server once the game is started (>= 1.6).
    #[arg(short, long)]
    pub server: Option<String>,
    /// Change the default port to connect to the server (--server).
    #[arg(short = 'p', long, name = "PORT", requires = "server", default_value_t = 25565)]
    pub server_port: u16,
}

#[derive(Debug, Clone)]
pub struct Resolution {
    pub width: u16,
    pub height: u16,
}

impl FromStr for Resolution {

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
