//! The bus described (`bus/declare`, `describe`, `list`, `find`): every
//! address's descriptor — the vocabulary's at start, mods' as they provide.

use std::collections::BTreeMap;
use std::sync::RwLock;
use substrate_lock::ReadWrite;

use yog_data::Shape;
use yog_event::bus::{AddressKind, Describe, Descriptor, Find, List};
use yog_hot_reload::ModuleGeneration;

use super::duties::provide;

/// Every descriptor, and the generation of the mod that declared it (none: the vocabulary's).
static DESCRIPTORS: RwLock<BTreeMap<String, (Descriptor, Option<ModuleGeneration>)>> =
    RwLock::new(BTreeMap::new());

/// A shape's doc: a struct's own.
fn doc(shape: &Shape) -> String {
    match shape {
        Shape::Struct(s) => s.doc.to_owned(),
        _ => String::new(),
    }
}

fn declare(d: Descriptor, owner: Option<ModuleGeneration>) {
    DESCRIPTORS
        .write_locked()
        .insert(d.address.clone(), (d, owner));
}

/// Forgets what the mod of `generation` declared (a hot reload declares it again).
pub(crate) fn purge(generation: ModuleGeneration) {
    DESCRIPTORS
        .write_locked()
        .retain(|_, (_, owner)| *owner != Some(generation));
}

/// `address`'s descriptor, or its nearest declared parent's (a keyed
/// event's branch, `…/tick/mymod:golem`, is its event's).
fn describe(address: &str) -> Option<Descriptor> {
    let all = DESCRIPTORS.read_locked();
    let mut at = address;
    loop {
        if let Some((d, _)) = all.get(at) {
            return Some(d.clone());
        }
        at = &at[..at.rfind(yog_ids::sep::PATH)?];
    }
}

/// The vocabulary's addresses: every event (and its before) and service.
fn declare_vocabulary() {
    for e in yog_event::all() {
        let shape = e.shape.describe();
        for address in std::iter::once(e.address).chain(e.pre) {
            declare(
                Descriptor {
                    address: address.to_owned(),
                    kind: AddressKind::Event,
                    doc: doc(&e.shape),
                    shape: shape.clone(),
                    reply: None,
                    interfaces: Vec::new(),
                },
                None,
            );
        }
    }
    for s in yog_event::services() {
        declare(
            Descriptor {
                address: s.address.to_owned(),
                kind: AddressKind::Service,
                doc: doc(&s.shape),
                shape: s.shape.describe(),
                reply: Some(s.reply.describe()),
                interfaces: Vec::new(),
            },
            None,
        );
    }
}

/// The bus's own services; once, at start.
pub(crate) fn register() {
    declare_vocabulary();
    provide::<Descriptor>(|d| {
        declare(d.clone(), super::owner().map(|m| m.generation()));
        true
    });
    provide::<Describe>(|d| describe(&d.address));
    provide::<List>(|l| {
        DESCRIPTORS
            .read_locked()
            .keys()
            .filter(|a| a.starts_with(&l.prefix))
            .cloned()
            .collect()
    });
    provide::<Find>(|f| {
        DESCRIPTORS
            .read_locked()
            .values()
            .filter(|(d, _)| d.interfaces.contains(&f.interface))
            .map(|(d, _)| d.address.clone())
            .collect()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keyed_branch_is_described_by_its_event() {
        declare_vocabulary();
        let tick = <yog_event::entity::game::Tick as yog_event::Event>::ADDRESS;
        let d = describe(&yog_event::keyed(tick, "mymod:golem")).expect("its event's descriptor");
        assert_eq!((d.address.as_str(), d.kind), (tick, AddressKind::Event));
        assert!(describe("nobody:nothing").is_none());
    }

    #[test]
    fn a_reloaded_mod_s_descriptors_go() {
        let generation = yog_hot_reload::GenerationAllocator::new().next();
        let d = Descriptor {
            address: "test:gone".into(),
            kind: AddressKind::Service,
            doc: String::new(),
            shape: <() as yog_data::Schema>::shape().describe(),
            reply: None,
            interfaces: Vec::new(),
        };
        declare(d, Some(generation));
        assert!(describe("test:gone").is_some());
        purge(generation);
        assert!(describe("test:gone").is_none());
    }
}
