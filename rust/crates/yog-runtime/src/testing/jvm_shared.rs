//! The tests' one JVM (a process may start one), shared by every JVM test.

use std::path::{Path, PathBuf};
use std::process::Command;
use substrate_lock::Locked;

use jni::JavaVM;

use crate::testing::jvm::{jdk, start};

/// The tests' one JVM (a process may start one): its classes in one
/// directory each test compiles its sources into (found when first asked
/// for, so a test compiling after another started it is fine), and Mixin's
/// and ASM's jars when they're in Gradle's cache (`YOG_MIXIN_CP` overrides).
pub struct Shared {
    pub jdk: PathBuf,
    pub classes: PathBuf,
    /// Mixin's and ASM's jars, when found: what the nanite tests need.
    pub mixin: Option<String>,
    pub vm: JavaVM,
}

static SHARED: std::sync::OnceLock<Option<Shared>> = std::sync::OnceLock::new();

/// The shared JVM, started the first time; `None` without a JDK.
pub fn shared() -> Option<&'static Shared> {
    SHARED
        .get_or_init(|| {
            let jdk = jdk()?;
            let classes =
                std::env::temp_dir().join(format!("yog-jvm-tests-{}", std::process::id()));
            std::fs::create_dir_all(&classes).ok()?;
            let mixin = mixin_classpath();
            let cp = match &mixin {
                Some(jars) => format!("{}:{jars}", classes.display()),
                None => classes.display().to_string(),
            };
            let vm = start(&jdk, Path::new(&cp));
            Some(Shared {
                jdk,
                classes,
                mixin,
                vm,
            })
        })
        .as_ref()
}

impl Shared {
    /// Compiles `sources` into the shared classes (against what's there and the jars).
    pub fn compile(&self, sources: &[PathBuf]) -> bool {
        // One javac at a time into the one directory (the tests run in parallel).
        static COMPILING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _one = COMPILING.locked();
        let cp = match &self.mixin {
            Some(jars) => format!("{}:{jars}", self.classes.display()),
            None => self.classes.display().to_string(),
        };
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let source_path = std::env::join_paths([
            root.join("java-api/src/main/java"),
            root.join("java-common/src/main/java"),
            root.join("../../Substrate-Data/ports/java/src/main/java"),
        ])
        .expect("Java source roots form a search path");
        Command::new(self.jdk.join("bin/javac"))
            // Mixin's jar carries an annotation processor; nothing here is for it.
            .arg("-proc:none")
            .arg("-cp")
            .arg(cp)
            .arg("-sourcepath")
            .arg(source_path)
            .arg("-d")
            .arg(&self.classes)
            .args(sources)
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Mixin's, ASM's and MixinExtras' jars: `YOG_MIXIN_CP`, else the newest in Gradle's cache.
fn mixin_classpath() -> Option<String> {
    if let Ok(cp) = std::env::var("YOG_MIXIN_CP") {
        return Some(cp);
    }
    let cache = PathBuf::from(std::env::var_os("HOME")?).join(".gradle/caches/modules-2/files-2.1");
    let newest = |group: &str, artifact: &str| -> Option<PathBuf> {
        let mut jars: Vec<PathBuf> = std::fs::read_dir(cache.join(group).join(artifact))
            .ok()?
            .flatten()
            .flat_map(|v| std::fs::read_dir(v.path()).into_iter().flatten().flatten())
            .flat_map(|h| std::fs::read_dir(h.path()).into_iter().flatten().flatten())
            .map(|f| f.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                name.starts_with(artifact)
                    && name.ends_with(".jar")
                    && !name.contains("sources")
                    && !name.contains("processor")
            })
            .collect();
        jars.sort();
        jars.pop()
    };
    let jars = [
        newest("org.spongepowered", "mixin")?,
        newest("org.ow2.asm", "asm")?,
        newest("org.ow2.asm", "asm-tree")?,
        newest("io.github.llamalad7", "mixinextras-common")?,
    ];
    Some(
        jars.iter()
            .map(|j| j.display().to_string())
            .collect::<Vec<_>>()
            .join(":"),
    )
}
