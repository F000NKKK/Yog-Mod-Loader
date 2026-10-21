//! Run targets: where `yog run` puts a mod and starts the game. The same for
//! every mod — nothing in a project names them:
//! - a **loader on a Minecraft version** (`fabric` on `26.2`): every version
//!   the loader SDK has, run by the SDK's own `build.sh` in its dev instance;
//! - a **machine's own** target from the local patch (`[run."<id>"]`).
//!
//! A target is a group (the loader) and a variant (the version), so a picker
//! can offer the two apart.

use crate::components::{self, Components};
use crate::local::{Local, RunSection, pairs};
use crate::{Home, resolve_in};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// What a Yog Mod Loader source tree is laid out as (its `build.sh` and Gradle builds).
mod sdk {
    /// A loader's Minecraft versions: `<loader>/versions/<mc>.properties`.
    pub const VERSIONS_DIR: &str = "versions";
    pub const PROPERTIES_EXT: &str = "properties";
    /// A loader's own settings, where its shown name is (`loader_name`).
    pub const LOADER_PROPERTIES: &str = "gradle.properties";
    pub const LOADER_NAME: &str = "loader_name";
    /// A version's Java (`java_version`) and whether it runs (`status=ported`).
    pub const JAVA_VERSION: &str = "java_version";
    pub const STATUS: &str = "status";
    pub const PORTED: &str = "ported";
    /// The dev instance a loader runs: `<loader>/run`, its mods in `yog-mods`.
    pub const RUN_DIR: &str = "run";
    /// The script that builds and runs it: `build.sh run <loader> --client --mc <mc> --no-build`.
    pub const SCRIPT: &str = "build.sh";
    pub const RUN: &str = "run";
    pub const CLIENT: &str = "--client";
    pub const MC: &str = "--mc";
    /// The runtime is the SDK's to build (`dev.sh`); a launch doesn't rebuild it.
    pub const NO_BUILD: &str = "--no-build";
}

/// How a target's game starts.
#[derive(Debug, Clone)]
enum How {
    Loader { sdk: PathBuf, loader: String, mc: String, java: Option<u32> },
    Custom(RunSection),
}

/// One run target.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// What `yog run` takes: `fabric-26.2`, or a local target's id.
    pub id: String,
    /// The loader (`fabric`), or a local target's id.
    pub group: String,
    pub group_label: String,
    /// The Minecraft version; none for a local target.
    pub variant: Option<String>,
    pub label: String,
    #[serde(skip)]
    how: How,
}

/// What launching a target does: export the `.yog`, then run a command.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub export_dir: Option<PathBuf>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

/// The run targets of this machine: the home's components and local patch.
pub struct Resolver {
    local_file: PathBuf,
    local: Local,
    components: Components,
}

impl Resolver {
    pub fn new(home: &Home) -> Result<Self, String> {
        Ok(Self { local_file: home.local_file(), local: home.local()?, components: home.components()? })
    }

    /// The loader SDK: the local patch's checkout, else the newest installed.
    pub fn loader_sdk(&self) -> Option<PathBuf> {
        self.local
            .loader_sdk
            .clone()
            .or_else(|| self.components.newest(components::id::LOADER_SDK).map(|c| c.path.clone()))
    }

    /// Java `major`'s JDK: the local patch's, else the newest installed (none: the SDK finds one).
    pub fn jdk(&self, major: u32) -> Option<PathBuf> {
        self.local
            .jdk(major)
            .map(Path::to_path_buf)
            .or_else(|| self.components.newest(&components::id::jdk(major)).map(|c| c.path.clone()))
    }

    /// The folders the targets are read from — the home, each loader's
    /// versions — for a tool that keeps the list fresh while they change.
    pub fn sources(&self, home: &Home) -> Vec<PathBuf> {
        let mut out = vec![home.root().to_path_buf()];
        if let Some(sdk) = self.loader_sdk() {
            let loaders = std::fs::read_dir(&sdk).into_iter().flatten().flatten().map(|e| e.path().join(sdk::VERSIONS_DIR));
            out.extend(loaders.filter(|v| v.is_dir()));
        }
        out.sort();
        out
    }

    /// Every target: the SDK's loaders and versions, then the local ones.
    pub fn targets(&self) -> Result<Vec<Target>, String> {
        let mut out = match self.loader_sdk() {
            Some(sdk) => loader_targets(&sdk)?,
            None => Vec::new(),
        };
        out.extend(self.local.runs.iter().map(|(id, r)| Target {
            id: id.clone(),
            group: id.clone(),
            group_label: r.name.clone().unwrap_or_else(|| id.clone()),
            variant: None,
            label: r.name.clone().unwrap_or_else(|| id.clone()),
            how: How::Custom(r.clone()),
        }));
        Ok(out)
    }

    /// Target `id`; the error lists what there is.
    pub fn target(&self, id: &str) -> Result<Target, String> {
        let all = self.targets()?;
        if let Some(t) = all.iter().find(|t| t.id == id) {
            return Ok(t.clone());
        }
        if all.is_empty() {
            return Err(format!(
                "no run target `{id}`, and none at all: install a loader SDK with Yog Installer, or name a checkout in {} ([sdk] loader = \"…\")",
                self.local_file.display()
            ));
        }
        let ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
        Err(format!("no run target `{id}`. There are: {}", ids.join(", ")))
    }

    /// How `target` launches (a local target's relative paths are the home's).
    pub fn plan(&self, target: &Target) -> Plan {
        let mut plan = match &target.how {
            How::Loader { sdk, loader, mc, java } => {
                let mut env = Vec::new();
                if let Some(jdk) = java.and_then(|j| self.jdk(j).map(|p| (j, p))) {
                    env.push((yog_ids::env::java_home(jdk.0), jdk.1.display().to_string()));
                }
                Plan {
                    export_dir: Some(sdk.join(loader).join(sdk::RUN_DIR).join(yog_ids::game_dir::MODS)),
                    command: Some(sdk.join(sdk::SCRIPT).display().to_string()),
                    args: [sdk::RUN, loader, sdk::CLIENT, sdk::MC, mc, sdk::NO_BUILD].map(str::to_string).to_vec(),
                    cwd: Some(sdk.clone()),
                    env,
                }
            }
            How::Custom(r) => Plan {
                export_dir: r.export_dir.as_deref().map(|d| resolve_in(&self.local.base, d)),
                command: r.command.clone(),
                args: r.args.clone(),
                cwd: r.cwd.as_deref().map(|d| resolve_in(&self.local.base, d)),
                env: pairs(&r.env),
            },
        };
        // The machine's launch environment, under the target's own.
        let mut env = self.local.env.clone();
        env.retain(|(k, _)| !plan.env.iter().any(|(pk, _)| pk == k));
        env.append(&mut plan.env);
        plan.env = env;
        plan
    }
}

/// Every loader and ported version in the SDK at `sdk`.
fn loader_targets(sdk: &Path) -> Result<Vec<Target>, String> {
    let entries = std::fs::read_dir(sdk).map_err(|e| format!("loader SDK {}: {e}", sdk.display()))?;
    let mut loaders: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.join(sdk::VERSIONS_DIR).is_dir()).collect();
    loaders.sort();
    let mut out = Vec::new();
    for dir in loaders {
        let Some(loader) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        let group_label = properties(&dir.join(sdk::LOADER_PROPERTIES), sdk::LOADER_NAME).unwrap_or_else(|| loader.clone());
        let mut versions: Vec<(String, Option<u32>)> = std::fs::read_dir(dir.join(sdk::VERSIONS_DIR))
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == sdk::PROPERTIES_EXT))
            .filter(|p| properties(p, sdk::STATUS).as_deref() == Some(sdk::PORTED))
            .filter_map(|p| {
                let mc = p.file_stem()?.to_str()?.to_string();
                Some((mc, properties(&p, sdk::JAVA_VERSION).and_then(|j| j.parse().ok())))
            })
            .collect();
        versions.sort_by(|a, b| crate::version::compare(&a.0, &b.0));
        for (mc, java) in versions {
            out.push(Target {
                id: format!("{loader}-{mc}"),
                label: format!("{group_label} {mc}"),
                group: loader.clone(),
                group_label: group_label.clone(),
                variant: Some(mc.clone()),
                how: How::Loader { sdk: sdk.to_path_buf(), loader: loader.clone(), mc, java },
            });
        }
    }
    Ok(out)
}

/// `key`'s value in the `.properties` file at `path`.
fn properties(path: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yog-home-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_checkout_gives_its_loaders_versions_and_the_patch_its_launch() {
        let root = temp("targets");
        let sdk = root.join("loader");
        write(&sdk.join("fabric/gradle.properties"), "loader_name=Fabric\n");
        write(&sdk.join("fabric/versions/26.2.properties"), "java_version=25\nstatus=ported\n");
        write(&sdk.join("fabric/versions/1.21.1.properties"), "java_version=21\nstatus=ported\n");
        write(&sdk.join("fabric/versions/27.1.properties"), "java_version=25\nstatus=scaffold\n");
        let home = Home::at(root.join("home"));
        write(
            &home.local_file(),
            &format!(
                "[sdk]\nloader = {:?}\n\n[java]\n21 = \"/jdk21\"\n\n[launch]\nenv = [\"GPU=nvidia\"]\n\n[run.\"prism\"]\nname = \"Prism\"\nexport_dir = \"inst/yog-mods\"\n",
                sdk.display().to_string()
            ),
        );
        let r = Resolver::new(&home).unwrap();
        let targets = r.targets().unwrap();
        let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["fabric-1.21.1", "fabric-26.2", "prism"]);
        assert_eq!((targets[0].group_label.as_str(), targets[0].variant.as_deref()), ("Fabric", Some("1.21.1")));

        let plan = r.plan(&targets[0]);
        assert_eq!(plan.export_dir, Some(sdk.join("fabric/run/yog-mods")));
        assert_eq!(plan.args, ["run", "fabric", "--client", "--mc", "1.21.1", "--no-build"]);
        assert_eq!(plan.env, [("GPU".to_string(), "nvidia".to_string()), ("YOG_JAVA21_HOME".to_string(), "/jdk21".to_string())]);

        assert_eq!(r.sources(&home), [home.root().to_path_buf(), sdk.join("fabric/versions")]);

        let prism = r.plan(&targets[2]);
        assert_eq!(prism.export_dir, Some(home.root().join("inst/yog-mods")));
        assert!(r.target("forge-1.20.1").unwrap_err().contains("There are: fabric-1.21.1"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
