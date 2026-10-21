//! A block's properties one by one, on top of its whole state.

use yog_core::{BlockPos, join_state, split_state};
use yog_ids::sep::{PROPERTY_IS, PROPERTY_SEP};

use super::{set_state, state};
use crate::Arg;

/// Property `name` of the block at `pos` (`"energized"` → `"true"`).
pub fn property(dim: impl Arg<String>, pos: impl Arg<BlockPos>, name: &str) -> Option<String> {
    let state = state(dim, pos)?;
    split_state(&state)
        .1
        .into_iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned())
}

/// Sets some properties of the block at `pos` (`"energized=true,powered=false"`),
/// keeping the rest; `false` if there's no block or the state doesn't take.
pub fn set_properties(dim: impl Arg<String>, pos: impl Arg<BlockPos>, properties: &str) -> bool {
    let (dim, pos) = (dim.arg(), pos.arg());
    let Some(state) = state(dim.as_str(), pos) else {
        return false;
    };
    let (id, mut props) = split_state(&state);
    for pair in properties.split(PROPERTY_SEP).filter(|p| !p.is_empty()) {
        let Some((k, v)) = pair.split_once(PROPERTY_IS) else {
            return false;
        };
        match props.iter_mut().find(|(n, _)| *n == k) {
            Some(p) => p.1 = v,
            None => props.push((k, v)),
        }
    }
    set_state(dim, pos, join_state(id, &props))
}
