//! The environment variables Yog's programs pass each other: the IDE and
//! `dev.sh` set them, the CLI, the runtime and the installer read them.

/// Where Yog keeps its home (SDKs, JDKs, caches); unset, the user's.
pub const HOME: &str = "YOG_HOME";
/// The platforms to build for (`linux-x86_64,windows-x86_64`); unset, every one.
pub const TARGETS: &str = "YOG_TARGETS";
/// A local `yog-api` crate the mods build against instead of crates.io's.
pub const API_PATH: &str = "YOG_API_PATH";
/// The `yog-api` version the mods build against instead of the newest.
pub const API_VERSION: &str = "YOG_API_VERSION";
/// The game's client jar the designers take vanilla's resources from.
pub const MC_CLIENT_JAR: &str = "YOG_MC_CLIENT_JAR";
/// The game's control socket (`127.0.0.1:<port>`), and the token a client proves itself with.
pub const CONTROL_SOCKET: &str = "YOG_CONTROL_SOCKET";
pub const CONTROL_TOKEN: &str = "YOG_CONTROL_TOKEN";
/// Where `yog build` and `yog check` write each diagnostic as a Yog Data
/// frame (the IDE's Error List reads them; the text goes on as ever).
pub const DIAGNOSTICS: &str = "YOG_DIAGNOSTICS";
/// Where a designer writes what it draws, as a PNG.
pub const DESIGNER_PNG: &str = "YOG_DESIGNER_PNG";
/// Where the images designer writes what it draws, as a PNG.
pub const IMAGES_PNG: &str = "YOG_IMAGES_PNG";
/// The custom-component designer: the mod's library, its component, the component's props and the mod.
pub const DESIGNER_LIB: &str = "YOG_DESIGNER_LIB";
pub const DESIGNER_COMPONENT: &str = "YOG_DESIGNER_COMPONENT";
pub const DESIGNER_PROPS: &str = "YOG_DESIGNER_PROPS";
pub const DESIGNER_MOD: &str = "YOG_DESIGNER_MOD";

/// The variable `build.sh` (`find_java`) takes Java `major`'s JDK from: `YOG_JAVA21_HOME`.
pub fn java_home(major: u32) -> String {
    format!("{JAVA_PREFIX}{major}{HOME_SUFFIX}")
}

const JAVA_PREFIX: &str = "YOG_JAVA";
const HOME_SUFFIX: &str = "_HOME";
