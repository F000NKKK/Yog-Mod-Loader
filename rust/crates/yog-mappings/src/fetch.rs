//! The mappings `yog mappings` joins, fetched once into Yog's cache
//! (`<home>/cache/mappings/`): Mojang's for a version, Fabric's
//! intermediary, Forge's SRG (MCPConfig).

use std::io::Read;
use std::path::PathBuf;

use yog_data::Value;

/// Mojang's version list.
const MOJANG_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
/// Fabric's intermediary for a version: `{base}/{mc}/intermediary-{mc}-v2.jar`.
const INTERMEDIARY_BASE: &str = "https://maven.fabricmc.net/net/fabricmc/intermediary";
const INTERMEDIARY_ENTRY: &str = "mappings/mappings.tiny";
/// Forge's MCPConfig: its versions, and `{base}/{v}/mcp_config-{v}.zip`.
const MCP_BASE: &str = "https://maven.minecraftforge.net/de/oceanlabs/mcp/mcp_config";
const MCP_METADATA: &str = "maven-metadata.xml";
const MCP_ENTRY: &str = "config/joined.tsrg";
/// The cache's folder.
const CACHE: &str = "mappings";
/// The manifest's and a version's keys.
const VERSIONS: &str = "versions";
const ID: &str = "id";
const URL: &str = "url";
const DOWNLOADS: &str = "downloads";
pub const CLIENT_MAPPINGS: &str = "client_mappings";
pub const SERVER_MAPPINGS: &str = "server_mappings";
/// The game's jars, by their downloads' keys.
pub const CLIENT_JAR: &str = "client";
pub const SERVER_JAR: &str = "server";
/// A Maven metadata version's tags.
const VERSION_OPEN: &str = "<version>";
const VERSION_CLOSE: &str = "</version>";

fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut response = ureq::get(url).call().map_err(|e| format!("{url}: {e}"))?;
    response
        .body_mut()
        .with_config()
        .limit(u64::MAX)
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))
}

fn text(url: &str) -> Result<String, String> {
    String::from_utf8(get(url)?).map_err(|e| format!("{url}: {e}"))
}

/// Where `name` is kept in the cache.
fn cache_path(name: &str) -> Result<PathBuf, String> {
    Ok(yog_home::Home::locate()?.cache_dir().join(CACHE).join(name))
}

/// `name` from the cache, else made by `make` and kept.
fn cached(name: &str, make: impl FnOnce() -> Result<String, String>) -> Result<String, String> {
    let path = cache_path(name)?;
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    if let Ok(t) = std::fs::read_to_string(&path) {
        return Ok(t);
    }
    let t = make()?;
    std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(&path, &t))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(t)
}

/// Entry `entry` of the zip at `url`.
fn zip_entry(url: &str, entry: &str) -> Result<String, String> {
    let bytes = get(url)?;
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("{url}: {e}"))?;
    let mut file = zip
        .by_name(entry)
        .map_err(|e| format!("{url}: {entry}: {e}"))?;
    let mut out = String::new();
    file.read_to_string(&mut out)
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(out)
}

fn json(url: &str) -> Result<Value, String> {
    yog_data::json::from_json_text(&text(url)?).ok_or_else(|| format!("{url}: not JSON"))
}

/// The URL of `mc`'s download `key` (`client`, `server_mappings`, …);
/// none when the version has no such download.
fn download_url(mc: &str, key: &str) -> Result<Option<String>, String> {
    let manifest = json(MOJANG_MANIFEST)?;
    let version = manifest
        .get(VERSIONS)
        .and_then(Value::as_list)
        .and_then(|vs| {
            vs.iter()
                .find(|v| v.get(ID).and_then(Value::as_str) == Some(mc))
        })
        .and_then(|v| v.get(URL)?.as_str())
        .ok_or_else(|| format!("Mojang has no version {mc}"))?;
    Ok(json(version)?
        .get(DOWNLOADS)
        .and_then(|d| d.get(key)?.get(URL)?.as_str().map(str::to_owned)))
}

/// Mojang's mappings of `mc`'s client (ProGuard's text).
pub fn mojang(mc: &str) -> Result<String, String> {
    side_mappings(mc, CLIENT_MAPPINGS)?
        .ok_or_else(|| format!("{mc} has no mappings (unobfuscated: its names are Mojang's)"))
}

/// Mojang's mappings of one side of `mc` (`client_mappings`,
/// `server_mappings`); none for a version that isn't obfuscated.
pub fn side_mappings(mc: &str, key: &str) -> Result<Option<String>, String> {
    let name = format!("{mc}.{key}.txt");
    if let Some(t) = cache_path(&name)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
    {
        return Ok(Some(t));
    }
    match download_url(mc, key)? {
        Some(url) => cached(&name, || text(&url)).map(Some),
        None => Ok(None),
    }
}

/// `mc`'s `client` or `server` jar, fetched once.
pub fn game_jar(mc: &str, side: &str) -> Result<Vec<u8>, String> {
    let path = cache_path(&format!("{mc}.{side}.jar"))?;
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    if let Ok(b) = std::fs::read(&path) {
        return Ok(b);
    }
    let url = download_url(mc, side)?.ok_or_else(|| format!("{mc} has no {side} jar"))?;
    let bytes = get(&url)?;
    std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(&path, &bytes))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes)
}

/// Fabric's intermediary of `mc` (tiny v2).
pub fn intermediary(mc: &str) -> Result<String, String> {
    cached(&format!("{mc}.intermediary.tiny"), || {
        zip_entry(
            &format!("{INTERMEDIARY_BASE}/{mc}/intermediary-{mc}-v2.jar"),
            INTERMEDIARY_ENTRY,
        )
    })
}

/// Forge's SRG of `mc` (tsrg2), from MCPConfig's latest build for it.
pub fn srg(mc: &str) -> Result<String, String> {
    cached(&format!("{mc}.srg.tsrg"), || {
        let metadata = text(&format!("{MCP_BASE}/{MCP_METADATA}"))?;
        let prefix = format!("{mc}-");
        let version = metadata
            .split(VERSION_OPEN)
            .filter_map(|s| s.split(VERSION_CLOSE).next())
            .filter(|v| v.starts_with(&prefix))
            .last()
            .ok_or_else(|| format!("MCPConfig has no build for {mc}"))?;
        zip_entry(
            &format!("{MCP_BASE}/{version}/mcp_config-{version}.zip"),
            MCP_ENTRY,
        )
    })
}
