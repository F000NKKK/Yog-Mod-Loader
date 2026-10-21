//! The protocol's frames as the other side reads them.

use super::{Changes, Command, Event, ModInfo, receive, send};

/// `message` sent, then read back.
fn through<T: yog_data::Schema>(message: &T) -> T {
    let mut pipe = Vec::new();
    send(&mut pipe, message).unwrap();
    receive(&mut pipe.as_slice()).unwrap().unwrap()
}

#[test]
fn commands_cross_whole() {
    let hot_reload = Command::HotReload {
        mod_id: "m".into(),
        yog_path: "/m.yog".into(),
        resources: true,
    };
    assert_eq!(through(&hot_reload), hot_reload);
    assert_eq!(through(&Command::ListMods), Command::ListMods);
}

#[test]
fn events_cross_whole() {
    let check = Event::HotReloadCheck {
        mod_id: "m".into(),
        changes: Changes {
            added: vec!["item m:a".into()],
            ..Changes::default()
        },
    };
    assert_eq!(through(&check), check);
    let ready = Event::Ready {
        pid: 7,
        mods: vec![ModInfo {
            id: "m".into(),
            ..ModInfo::default()
        }],
        ptracer: true,
    };
    assert_eq!(through(&ready), ready);
    assert_eq!(through(&Event::ResourcesReloaded), Event::ResourcesReloaded);
}

#[test]
fn the_tags_name_the_variants() {
    let v = yog_data::Schema::write(&Command::ReloadResources);
    assert_eq!(
        v.get("cmd").and_then(|c| c.as_str()),
        Some("reload_resources")
    );
}
