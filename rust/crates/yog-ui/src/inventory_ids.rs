//! The ids inventory widgets carry, written and read in one place: a
//! slot's `<SLOT_PREFIX><index>` (and `=<accepts>`, `|`-joined, when it
//! only takes some items) and a machine marker's
//! `<MACHINE_PREFIX><kind>;<result slot>;` then its places, `,`-joined
//! `<kind's slot>/<index>=<inventory slot>`. A screen's widgets write them
//! (`widget::slot`, `widget::machine`); yog-kit reads them back to make
//! the inventory the game opens.

pub use yog_ids::yog::{MACHINE_PREFIX, PLAYER_INVENTORY, SLOT_PREFIX};

/// What follows a slot's index when it only takes some items, and what joins those.
const ACCEPTS: char = '=';
const ACCEPTS_SEP: &str = "|";
/// What separates a machine's kind, result and places; the places; a
/// place's slot from its index, and that from its inventory slot.
const PART_SEP: char = ';';
const PLACE_SEP: &str = ",";
const PLACE_INDEX: char = '/';
const PLACE_INV: char = '=';
/// A machine id's parts: kind, result, places.
const MACHINE_PARTS: usize = 3;

/// Slot `index`'s id, taking only `accepts` (none: anything).
pub fn slot_id(index: usize, accepts: &[&str]) -> String {
    match accepts {
        [] => format!("{SLOT_PREFIX}{index}"),
        _ => format!("{SLOT_PREFIX}{index}{ACCEPTS}{}", accepts.join(ACCEPTS_SEP)),
    }
}

/// A slot's index and what it accepts, if `id` is a slot's.
pub fn read_slot(id: &str) -> Option<(usize, Vec<String>)> {
    let rest = id.strip_prefix(SLOT_PREFIX)?;
    let (index, accepts) = rest.split_once(ACCEPTS).unwrap_or((rest, ""));
    let accepts = accepts
        .split(ACCEPTS_SEP)
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .collect();
    Some((index.parse().ok()?, accepts))
}

/// A machine making recipes of `kind`: its places (`(kind's slot, index,
/// inventory slot)`) and its result slot.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineId {
    pub kind: String,
    pub places: Vec<(String, usize, usize)>,
    pub result: usize,
}

impl MachineId {
    pub fn write(kind: &str, places: &[(&str, usize, usize)], result: usize) -> String {
        let places: Vec<String> = places
            .iter()
            .map(|(s, i, inv)| format!("{s}{PLACE_INDEX}{i}{PLACE_INV}{inv}"))
            .collect();
        format!(
            "{MACHINE_PREFIX}{kind}{PART_SEP}{result}{PART_SEP}{}",
            places.join(PLACE_SEP)
        )
    }

    /// The machine `id` names, if it's a machine's.
    pub fn read(id: &str) -> Option<MachineId> {
        let mut parts = id
            .strip_prefix(MACHINE_PREFIX)?
            .splitn(MACHINE_PARTS, PART_SEP);
        let kind = parts.next()?.to_string();
        let result = parts.next()?.parse().ok()?;
        let places = parts
            .next()
            .unwrap_or_default()
            .split(PLACE_SEP)
            .filter(|p| !p.is_empty())
            .filter_map(|p| {
                let (slot, rest) = p.split_once(PLACE_INDEX)?;
                let (index, inv) = rest.split_once(PLACE_INV)?;
                Some((slot.to_string(), index.parse().ok()?, inv.parse().ok()?))
            })
            .collect();
        Some(MachineId {
            kind,
            places,
            result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_read_back_as_written() {
        assert_eq!(read_slot(&slot_id(3, &[])), Some((3, vec![])));
        assert_eq!(
            read_slot(&slot_id(4, &["a:b", "#c:d"])),
            Some((4, vec!["a:b".into(), "#c:d".into()]))
        );
        let id = MachineId::write("m:k", &[("in", 0, 1), ("in", 1, 2)], 5);
        assert_eq!(
            MachineId::read(&id),
            Some(MachineId {
                kind: "m:k".into(),
                places: vec![("in".into(), 0, 1), ("in".into(), 1, 2)],
                result: 5
            })
        );
        assert_eq!(read_slot("other"), None);
    }
}
