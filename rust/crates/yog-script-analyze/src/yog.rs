//! Yog's surface as compiled code writes it (`yog-event`'s Java records,
//! `dev.yog.bus`): a service's record called, an event's listened to, a
//! payload's fields read.

use yog_event::java;

use crate::typeck::surface::SURFACE;

/// A Yog method's call in Java.
pub struct Service {
    /// Its record, in full (`dev.yog.bus.YogServices.Chat.Broadcast`).
    pub record: String,
    /// It's a handle's (`player.Teleport(…)`): the handle is the record's
    /// first field.
    pub on_handle: bool,
}

/// Group (or handle) `group`'s method `method`.
pub fn service(group: &str, method: &str) -> Option<Service> {
    let m = SURFACE.groups.get(group)?.methods.get(method)?.first()?;
    Some(Service {
        record: format!(
            "{}.{}.{}",
            java::PACKAGE,
            java::SERVICES,
            java::record_path(m.domain, method)
        ),
        on_handle: is_handle(group),
    })
}

/// The record of the service at `address`, in full (what calls it).
pub fn service_at(address: &str) -> Option<String> {
    let info = yog_event::services()
        .into_iter()
        .find(|s| s.address == address)?;
    Some(format!(
        "{}.{}.{}",
        java::PACKAGE,
        java::SERVICES,
        java::record_path(info.domain, info.name)
    ))
}

/// The record registering content (`Content.Register`), in full.
pub fn content_register() -> Option<String> {
    service_at(<yog_event::content::Register as yog_event::Service>::ADDRESS)
}

/// Group `group`'s event `name`'s record, in full (`…YogEvents.Block.Break`).
pub fn event(group: &str, name: &str) -> Option<String> {
    let plane = SURFACE.groups.get(group)?.planes.get(name)?;
    Some(format!(
        "{}.{}.{}",
        java::PACKAGE,
        java::EVENTS,
        java::record_path(plane, name)
    ))
}

/// A payload type's Java class: an event's arguments' record, a struct's.
pub fn data(name: &str) -> Option<String> {
    if let Some((plane, event)) = SURFACE.args_planes.get(name) {
        return Some(format!(
            "{}.{}.{}",
            java::PACKAGE,
            java::EVENTS,
            java::record_path(plane, event)
        ));
    }
    let s = SURFACE.structs.get(name)?;
    Some(format!("{}.{}.{}", java::PACKAGE, java::TYPES, s.name))
}

/// A payload field written `name` in a source (`PlayerName`): its Java
/// accessor's name (`playerName`).
pub fn field(name: &str) -> String {
    java::field(&yog_codegen::snake(name))
}

/// Whether `name` is a handle (`Player`, `Entity`, `World`): in Java, its
/// subject (a string).
pub fn is_handle(name: &str) -> bool {
    yog_event::SUBJECTS.iter().any(|(_, h)| *h == name)
}
