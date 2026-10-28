//! A block's states: rotations at right angles, what facing, attach,
//! waterlogged and the block's own properties go with, and those
//! properties' names and values.

use std::collections::HashSet;

use yog_data::{Diagnostic, Path, field};
use yog_ids::block_state::{FACE, FACING, WATERLOGGED};

use super::error;
use crate::content::vocab::{MIN_PROPERTY_VALUES, RIGHT_ANGLES};
use crate::content::{Facing, Rotation, States};

impl States {
    /// Its rules, it being at `at`.
    pub(crate) fn rules_at(&self, at: Path, out: &mut Vec<Diagnostic>) {
        self.rotation_rules(at.key(field!(States, rotations)), out);
        self.facing_rules(&at, out);
        for (name, values) in &self.properties.0 {
            property_rules(
                at.key(field!(States, properties)).key(name),
                name,
                values,
                out,
            );
        }
    }

    fn rotation_rules(&self, at: Path, out: &mut Vec<Diagnostic>) {
        let Some(rs) = &self.rotations else { return };
        if rs.is_empty() {
            out.push(error(
                at.clone(),
                "a non-empty list of { x: 0, y: 90, weight: 1, uvlock: false }",
            ));
        }
        for (i, r) in rs.iter().enumerate() {
            for (key, angle) in [(field!(Rotation, x), r.x), (field!(Rotation, y), r.y)] {
                if angle.is_some_and(|n| !RIGHT_ANGLES.contains(&n)) {
                    out.push(error(at.index(i).key(key), "must be 0, 90, 180 or 270"));
                }
            }
        }
    }

    fn facing_rules(&self, at: &Path, out: &mut Vec<Diagnostic>) {
        let turns = self.facing.is_some()
            || self.attach
            || self.waterlogged
            || !self.properties.0.is_empty();
        if !turns {
            if self.placement.is_some() || !self.models.0.is_empty() || !self.variants.0.is_empty()
            {
                out.push(error(
                    at.clone(),
                    "placement, models and variants need facing, attach, waterlogged or properties",
                ));
            }
            return;
        }
        if self.rotations.is_some() || self.connect.is_some() {
            out.push(error(
                at.clone(),
                "facing, attach, waterlogged and properties can't go with rotations or connect",
            ));
        }
        if self.attach && self.facing == Some(Facing::All) {
            out.push(error(
                at.key(field!(States, facing)),
                "an attached block turns four ways — leave facing out",
            ));
        }
        if self.placement.is_some() && self.facing.is_none() && !self.attach {
            out.push(error(
                at.key(field!(States, placement)),
                "placement needs facing (or attach)",
            ));
        }
    }
}

/// A property of the block's own: named in lowercase, not one the game's
/// makes, two or more different values.
fn property_rules(at: Path, name: &str, values: &[String], out: &mut Vec<Diagnostic>) {
    if !yog_ids::valid_name(name) {
        out.push(error(
            at.clone(),
            format!("`{name}`: a property's name is lowercase letters, digits and _"),
        ));
    } else if [FACING, FACE, WATERLOGGED].contains(&name) {
        out.push(error(
            at.clone(),
            format!("`{name}` comes from facing, attach or waterlogged — name yours otherwise"),
        ));
    }
    let mut seen = HashSet::new();
    if values.len() < MIN_PROPERTY_VALUES
        || !values
            .iter()
            .all(|v| yog_ids::valid_name(v) && seen.insert(v.as_str()))
    {
        out.push(error(
            at,
            "two or more different values, lowercase letters, digits and _",
        ));
    }
}
