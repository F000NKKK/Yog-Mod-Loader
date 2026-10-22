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
const CLIENT_MAPPINGS: &str = "client_mappings";
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

/// `name` from the cache, else made by `make` and kept.
fn cached(name: &str, make: impl FnOnce() -> Result<String, String>) -> Result<String, String> {
    let dir: PathBuf = yog_home::Home::locate()?.cache_dir().join(CACHE);
    let path = dir.join(name);
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

/// Mojang's mappings of `mc`'s client (ProGuard's text).
pub(crate) fn mojang(mc: &str) -> Result<String, String> {
    cached(&format!("{mc}.mojang.txt"), || {
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
        let url = json(version)?
            .get(DOWNLOADS)
            .and_then(|d| {
                d.get(CLIENT_MAPPINGS)?
                    .get(URL)?
                    .as_str()
                    .map(str::to_owned)
            })
            .ok_or_else(|| {
                format!("{mc} has no mappings (unobfuscated: its names are Mojang's)")
            })?;
        text(&url)
    })
}

/// Fabric's intermediary of `mc` (tiny v2).
pub(crate) fn intermediary(mc: &str) -> Result<String, String> {
    cached(&format!("{mc}.intermediary.tiny"), || {
        zip_entry(
            &format!("{INTERMEDIARY_BASE}/{mc}/intermediary-{mc}-v2.jar"),
            INTERMEDIARY_ENTRY,
        )
    })
}

/// Forge's SRG of `mc` (tsrg2), from MCPConfig's latest build for it.
pub(crate) fn srg(mc: &str) -> Result<String, String> {
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
