use super::*;

#[test]
fn a_chunks_instance_reads_with_its_dimension() {
    let at = |instance: &str| Where {
        scope: SESSION.into(),
        instance: instance.into(),
    };
    assert!(
        store(&at("")).is_some(),
        "the session's store is always there"
    );
    let chunk = Where {
        scope: CHUNK.into(),
        instance: format!("minecraft{SEP}the{SEP}nether{SEP}-3{SEP}12"),
    };
    // No world open in a test: none, but the instance read without a panic.
    assert!(store(&chunk).is_none());
}

#[test]
fn owners_are_the_records_owners_once() {
    let keys = vec![
        "1_64_2/charge.Charger".to_owned(),
        "1_64_2/charge.Owned".to_owned(),
        "5_64_5/charge.Charger".to_owned(),
        "charge.World".to_owned(),
    ];
    assert_eq!(owners(keys), ["1_64_2", "5_64_5"]);
}
