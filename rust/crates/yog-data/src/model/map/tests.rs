use super::*;

/// A big map is searched by its index, which every change keeps current:
/// an insert, a removal (the later keys move up), a rewrite.
#[test]
fn a_big_maps_index_follows_its_changes() {
    let n = INDEXED_FROM * 4;
    let mut map: Map = (0..n)
        .map(|i| (format!("k{i}"), Value::Int(i as i64)))
        .collect();
    assert_eq!(map.get("k3"), Some(&Value::Int(3)));
    assert_eq!(map.remove("k1"), Some(Value::Int(1)));
    assert_eq!(map.index_of("k2"), Some(1));
    assert_eq!(map.get("k1"), None);
    map.insert("new", Value::Bool(true));
    assert_eq!(map.index_of("new"), Some(n - 1));
    map.insert("k5", Value::Null);
    assert_eq!(map.get("k5"), Some(&Value::Null));
    map.retain(|k, _| k != "k7");
    assert_eq!(map.get("k7"), None);
    assert_eq!(map.get("k8"), Some(&Value::Int(8)));
    // One removed, one added, one not kept.
    assert_eq!(map.keys().count(), n - 1);
}

/// Equal maps are equal whether one's indexed or not.
#[test]
fn equality_is_the_entries() {
    let make = || -> Map {
        (0..INDEXED_FROM * 2)
            .map(|i| (format!("k{i}"), Value::Int(i as i64)))
            .collect()
    };
    let (a, b) = (make(), make());
    let _ = a.get("k0");
    assert_eq!(a, b);
}

/// The index costs a value nothing: a map is no bigger than the biggest
/// other kind of value (a reference: two strings).
#[test]
fn a_map_is_no_bigger_than_a_reference() {
    assert!(std::mem::size_of::<Map>() <= std::mem::size_of::<super::super::value::Ref>());
}
