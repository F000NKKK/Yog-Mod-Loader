//! `yog control`: what a launcher (`dev.sh`) asks a running game over its
//! control channel (`yog_hot_reload::control`) — the game whose address and
//! token are in `YOG_CONTROL_SOCKET` and `YOG_CONTROL_TOKEN`.

use std::path::Path;
use std::time::Duration;

use yog_hot_reload::control::{Client, Command, Event};

pub(super) const WORDS: &[&str] = &["control"];
const HOT_RELOAD: &str = "hot-reload";
const RELOAD_RESOURCES: &str = "reload-resources";
const USAGE: &str = "usage: yog control hot-reload <mod.yog>... | yog control reload-resources";
/// How long a mod's swap may take.
const HOT_RELOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the game may take to reread its resources and data.
const RELOAD_TIMEOUT: Duration = Duration::from_secs(180);

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let (what, files) = rest.split_first().ok_or(USAGE)?;
    match what.as_str() {
        HOT_RELOAD if !files.is_empty() => {
            let mut client = connect(HOT_RELOAD_TIMEOUT)?;
            files
                .iter()
                .try_for_each(|f| hot_reload(&mut client, Path::new(f)))
        }
        RELOAD_RESOURCES => reload_resources(&mut connect(RELOAD_TIMEOUT)?),
        _ => Err(USAGE.into()),
    }
}

/// The game, greeted; its answers awaited at most `timeout`.
fn connect(timeout: Duration) -> Result<Client, String> {
    let var = |name: &str| {
        std::env::var(name)
            .map_err(|_| format!("{name} isn't set — start the game with ./dev.sh --run"))
    };
    let addr = var(yog_ids::env::CONTROL_SOCKET)?;
    let token = var(yog_ids::env::CONTROL_TOKEN)?;
    let (client, _ready) =
        Client::connect(&addr, &token).map_err(|e| format!("the game at {addr}: {e}"))?;
    client
        .stream()
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    Ok(client)
}

/// Swaps in the mod built as `yog` (its id the file's stem), saying first
/// what only a restart applies.
fn hot_reload(client: &mut Client, yog: &Path) -> Result<(), String> {
    let mod_id = yog
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("{}: no mod id", yog.display()))?
        .to_owned();
    let yog_path = std::path::absolute(yog).map_err(|e| e.to_string())?;
    let failed = |e: std::io::Error| format!("{mod_id}: {e}");
    let check = Command::HotReloadCheck {
        mod_id: mod_id.clone(),
        yog_path: yog_path.clone(),
    };
    let changes = client
        .ask(&check, |e| match e {
            Event::HotReloadCheck { changes, .. } => Some(changes),
            _ => None,
        })
        .map_err(failed)?;
    let restart = [
        ("added", &changes.added),
        ("removed", &changes.removed),
        ("changed", &changes.changed),
    ];
    for (what, entries) in restart {
        for entry in entries {
            println!("  {mod_id}: {entry} {what} — needs a restart");
        }
    }
    let swap = Command::HotReload {
        mod_id: mod_id.clone(),
        yog_path,
        resources: false,
    };
    let ok = client
        .ask(&swap, |e| match e {
            Event::HotReloadDone { ok, .. } => Some(ok),
            _ => None,
        })
        .map_err(failed)?;
    if !ok {
        return Err(format!("{mod_id}: hot reload failed (see the game's log)"));
    }
    println!("  {mod_id}: ok");
    Ok(())
}

/// The game rereads its resources and data (`/reload`, F3+T).
fn reload_resources(client: &mut Client) -> Result<(), String> {
    client
        .ask(&Command::ReloadResources, |e| {
            matches!(e, Event::ResourcesReloaded).then_some(())
        })
        .map_err(|e| e.to_string())?;
    println!("  resources and data reloaded");
    Ok(())
}
