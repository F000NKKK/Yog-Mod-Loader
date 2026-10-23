//! Yog's surface as a `.ys` sees it, made from yog-event's own description
//! (what Java's and the Rust API's are made from): the handles (`Player`,
//! `Entity`, `World`), each action whose first field is a handle's subject
//! its method without that field; every other action a static method of
//! its domain's group (`Chat.Broadcast`, `Client.Control.Attack`); and the
//! structs and enums the actions answer with, as types.

use std::collections::HashMap;

use yog_data::Shape;
use yog_data::schema::{EnumShape, Field, StructShape};
use yog_sync::LazyLock;

/// A method of a group: its parameters (by name) and its answer.
#[derive(Clone)]
pub(crate) struct YogMethod {
    pub params: Vec<Field>,
    pub reply: Shape,
}

/// A handle's or a domain's methods, by name (overloads side by side).
#[derive(Default)]
pub(crate) struct Group {
    pub methods: HashMap<String, Vec<YogMethod>>,
}

/// The whole surface: groups by dotted name, data types by name.
#[derive(Default)]
pub(crate) struct Surface {
    pub groups: HashMap<String, Group>,
    pub structs: HashMap<&'static str, StructShape>,
    pub enums: HashMap<&'static str, EnumShape>,
}

/// What separates a domain's segments (`client/control`), and the groups'
/// in a source (`Client.Control`).
const DOMAIN_SEP: char = '/';
const GROUP_SEP: &str = ".";

/// Yog's surface, made once a process.
pub(crate) static SURFACE: LazyLock<Surface> = LazyLock::new(Surface::make);

impl Surface {
    fn make() -> Self {
        let mut out = Surface::default();
        for s in yog_event::services() {
            let Shape::Struct(request) = &s.shape else {
                continue;
            };
            let subject = request.fields.first().and_then(|f| {
                yog_event::SUBJECTS
                    .iter()
                    .find(|(field, _)| *field == f.name)
            });
            let (group, params) = match subject {
                Some((_, handle)) => (handle.to_string(), request.fields[1..].to_vec()),
                None => (group_name(s.domain), request.fields.clone()),
            };
            for f in &params {
                out.note(&(f.shape)());
            }
            out.note(&s.reply);
            let method = YogMethod {
                params,
                reply: s.reply.clone(),
            };
            out.groups
                .entry(group)
                .or_default()
                .methods
                .entry(s.name.to_string())
                .or_default()
                .push(method);
        }
        out
    }

    /// The structs and enums `shape` names (and theirs), by name.
    fn note(&mut self, shape: &Shape) {
        match shape {
            Shape::Struct(s) if !self.structs.contains_key(s.name) => {
                self.structs.insert(s.name, s.clone());
                for f in &s.fields {
                    self.note(&(f.shape)());
                }
            }
            Shape::Enum(e) if !self.enums.contains_key(e.name) => {
                self.enums.insert(e.name, e.clone());
            }
            Shape::List(inner) | Shape::Map(inner) | Shape::Optional(inner) => self.note(&inner()),
            _ => {}
        }
    }

    /// Whether `name` is a group, a struct or an enum of the surface.
    pub fn knows(&self, name: &str) -> bool {
        self.groups.contains_key(name)
            || self.structs.contains_key(name)
            || self.enums.contains_key(name)
    }
}

/// `client/control` → `Client.Control`.
fn group_name(domain: &str) -> String {
    domain
        .split(DOMAIN_SEP)
        .map(yog_codegen::pascal)
        .collect::<Vec<_>>()
        .join(GROUP_SEP)
}

/// The group a member `name` of group `group` is (`Client` + `Control`).
pub(crate) fn subgroup(group: &str, name: &str) -> String {
    format!("{group}{GROUP_SEP}{name}")
}
