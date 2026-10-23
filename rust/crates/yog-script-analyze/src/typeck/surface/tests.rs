use super::*;

/// Every one of yog-event's events is a group's, none hiding another of
/// the same name (`player/session` `Join` and `player/life` `Death` are
/// both `Player`'s).
#[test]
fn every_event_has_a_place_of_its_own() {
    let events: usize = SURFACE.groups.values().map(|g| g.events.len()).sum();
    assert_eq!(events, yog_event::all().len());
    assert!(SURFACE.groups["Player"].events.contains_key("Join"));
    assert!(SURFACE.structs.contains_key("Player.JoinArgs"));
}
