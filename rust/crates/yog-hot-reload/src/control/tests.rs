//! The protocol's lines as the other side writes them.

use super::{Changes, Command, Event, line, parse};

#[test]
fn commands_are_tagged_by_cmd() {
    let hot_reload = Command::HotReload {
        mod_id: "m".into(),
        yog_path: "/m.yog".into(),
        resources: true,
    };
    assert_eq!(
        line(&hot_reload),
        "{\"cmd\":\"hot_reload\",\"mod_id\":\"m\",\"yog_path\":\"/m.yog\",\"resources\":true}\n"
    );
    assert_eq!(
        parse::<Command>("{\"cmd\":\"list_mods\"}"),
        Some(Command::ListMods)
    );
}

#[test]
fn events_are_tagged_by_event() {
    let check = "{\"event\":\"hot-reload-check\",\"mod_id\":\"m\",\"added\":[\"item m:a\"],\"removed\":[],\"changed\":[]}";
    let changes = Changes {
        added: vec!["item m:a".into()],
        ..Changes::default()
    };
    assert_eq!(
        parse::<Event>(check),
        Some(Event::HotReloadCheck {
            mod_id: "m".into(),
            changes
        })
    );
    assert_eq!(
        parse::<Event>(&line(&Event::ResourcesReloaded)),
        Some(Event::ResourcesReloaded)
    );
}

#[test]
fn a_ready_without_ptracer_still_reads() {
    let ready = parse::<Event>("{\"event\":\"ready\",\"pid\":7,\"mods\":[]}");
    assert_eq!(
        ready,
        Some(Event::Ready {
            pid: 7,
            mods: Vec::new(),
            ptracer: None
        })
    );
}
