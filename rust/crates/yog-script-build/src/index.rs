//! The class index of a Minecraft version
//! (`yog_script_analyze::index`), built from the game's own client and server jars
//! (Mojang's names: an obfuscated version's renamed by its mappings) and
//! kept in Yog's cache. A class in both jars is shared, in one only that
//! side's.

mod jar;
mod remap;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub use yog_script_analyze::index::{Class, ClassIndex, Side};

use crate::classfile::ClassFile;
use remap::Remap;
use yog_mappings::fetch::{self, CLIENT_JAR, CLIENT_MAPPINGS, SERVER_JAR, SERVER_MAPPINGS};
use yog_mappings::proguard;

/// Where indexes are kept in the cache, and their files' extension.
const CACHE: &str = "class-index";

/// The class index of `mc`, from the cache or built (fetching the jars).
pub fn load(mc: &str) -> Result<ClassIndex, String> {
    let path = cache_path(mc)?;
    if let Some(index) = std::fs::read(&path)
        .ok()
        .and_then(|b| yog_data::read_bytes::<ClassIndex>(&b).ok())
    {
        return Ok(index);
    }
    let index = build(mc)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, yog_data::to_bytes(&index))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(index)
}

fn cache_path(mc: &str) -> Result<PathBuf, String> {
    let file = format!("{mc}.{}", yog_ids::Format::Data.ext());
    Ok(yog_home::Home::locate()?.cache_dir().join(CACHE).join(file))
}

/// One side's classes: its jar read, renamed to Mojang's names.
fn side(mc: &str, jar: &str, mappings: &str) -> Result<Vec<ClassFile>, String> {
    let bytes = jar::game_classes(&fetch::game_jar(mc, jar)?)?;
    let remap = fetch::side_mappings(mc, mappings)?.map(|t| Remap::new(&proguard::parse(&t)));
    Ok(bytes
        .into_iter()
        .filter_map(|b| crate::classfile::read(&b))
        .map(|c| match &remap {
            Some(r) => r.apply(c),
            None => c,
        })
        // The game's own classes (the jars carry no others but obfuscated
        // ones, which renaming puts in these).
        .filter(|c| yog_script_analyze::index::namespace::is_game_class(&c.name))
        .collect())
}

/// The index of `mc`, built from its jars.
pub fn build(mc: &str) -> Result<ClassIndex, String> {
    let client = side(mc, CLIENT_JAR, CLIENT_MAPPINGS)?;
    let server = side(mc, SERVER_JAR, SERVER_MAPPINGS)?;
    let on_server: HashSet<&str> = server.iter().map(|c| c.name.as_str()).collect();
    let on_client: HashSet<&str> = client.iter().map(|c| c.name.as_str()).collect();
    let mut classes: HashMap<String, Class> = HashMap::new();
    for c in &client {
        let side = match on_server.contains(c.name.as_str()) {
            true => Side::Shared,
            false => Side::Client,
        };
        classes.insert(c.name.clone(), class(c, side));
    }
    for c in server
        .iter()
        .filter(|c| !on_client.contains(c.name.as_str()))
    {
        classes.insert(c.name.clone(), class(c, Side::Server));
    }
    let mut classes: Vec<Class> = classes.into_values().collect();
    classes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ClassIndex {
        mc: mc.to_owned(),
        classes,
    })
}

fn class(c: &ClassFile, side: Side) -> Class {
    Class {
        name: c.name.clone(),
        side,
        access: c.access,
        extends: c.extends.clone(),
        implements: c.implements.clone(),
        fields: c.fields.clone(),
        methods: c.methods.clone(),
    }
}
