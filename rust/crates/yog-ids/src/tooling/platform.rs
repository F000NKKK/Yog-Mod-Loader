//! The platforms a `.yog` bundles natives for: their tags (what the
//! archive's folders, `YOG_TARGETS` and the release assets go by), Rust
//! target triples and operating systems.

/// One platform a mod is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// `linux-x86_64`: its OS and architecture as Rust names them.
    pub tag: &'static str,
    /// The Rust target a mod's native is built with.
    pub triple: &'static str,
    /// Its OS as Rust names it (`std::env::consts::OS`).
    pub os: &'static str,
}

/// Every platform, the order builds go in.
pub const ALL: &[Platform] = &[
    Platform {
        tag: "linux-x86_64",
        triple: "x86_64-unknown-linux-gnu",
        os: "linux",
    },
    Platform {
        tag: "linux-aarch64",
        triple: "aarch64-unknown-linux-gnu",
        os: "linux",
    },
    Platform {
        tag: "windows-x86_64",
        triple: "x86_64-pc-windows-gnu",
        os: "windows",
    },
    Platform {
        tag: "macos-x86_64",
        triple: "x86_64-apple-darwin",
        os: "macos",
    },
    Platform {
        tag: "macos-aarch64",
        triple: "aarch64-apple-darwin",
        os: "macos",
    },
];

/// This computer's tag, whether or not mods are built for it.
pub fn host_tag() -> String {
    format!(
        "{}{}{}",
        std::env::consts::OS,
        crate::sep::HYPHEN,
        std::env::consts::ARCH
    )
}

/// This computer's platform, if mods are built for it.
pub fn host() -> Option<&'static Platform> {
    by_tag(&host_tag())
}

/// The platform tagged `tag`.
pub fn by_tag(tag: &str) -> Option<&'static Platform> {
    ALL.iter().find(|p| p.tag == tag)
}
