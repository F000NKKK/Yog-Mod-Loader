//! Block states as written (`"ns:id[k=v,...]"`).

/// What opens and closes a state's properties, separates them, and a
/// property's name from its value.
pub const PROPS_OPEN: char = '[';
pub const PROPS_CLOSE: char = ']';
pub const PROPERTY_SEP: char = ',';
pub const PROPERTY_IS: char = '=';

/// A block state (`"ns:id[k=v,...]"`) as its id and its properties.
pub fn split_state(state: &str) -> (&str, Vec<(&str, &str)>) {
    let (id, rest) = state.split_once(PROPS_OPEN).unwrap_or((state, ""));
    let props = rest
        .trim_end_matches(PROPS_CLOSE)
        .split(PROPERTY_SEP)
        .filter_map(|p| p.split_once(PROPERTY_IS))
        .collect();
    (id, props)
}

/// A block state from its id and properties: [`split_state`]'s inverse.
pub fn join_state(id: &str, props: &[(&str, &str)]) -> String {
    if props.is_empty() {
        return id.to_owned();
    }
    let props: Vec<String> = props.iter().map(|(k, v)| format!("{k}{PROPERTY_IS}{v}")).collect();
    format!("{id}{PROPS_OPEN}{}{PROPS_CLOSE}", props.join(&PROPERTY_SEP.to_string()))
}

#[cfg(test)]
mod tests {
    use super::split_state;

    #[test]
    fn a_state_joins_back() {
        assert_eq!(super::join_state("minecraft:stone", &[]), "minecraft:stone");
        assert_eq!(super::join_state("a:b", &[("x", "1"), ("y", "2")]), "a:b[x=1,y=2]");
    }

    #[test]
    fn a_state_splits_into_its_id_and_properties() {
        assert_eq!(split_state("minecraft:stone"), ("minecraft:stone", vec![]));
        assert_eq!(
            split_state("hexcasting:impetus/look[energized=false,facing=up]"),
            ("hexcasting:impetus/look", vec![("energized", "false"), ("facing", "up")])
        );
    }
}
