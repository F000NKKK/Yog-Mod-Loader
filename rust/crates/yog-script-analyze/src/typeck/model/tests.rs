use super::*;

#[test]
fn conversions_follow_the_grammar() {
    assert!(Type::Int.converts_to(&Type::Double));
    assert!(!Type::Double.converts_to(&Type::Int));
    assert!(Type::Null.converts_to(&Type::Own("Player".into()).nullable()));
    assert!(!Type::Null.converts_to(&Type::Own("Player".into())));
    assert!(Type::Int.converts_to(&Type::Int.nullable()));
    assert!(Type::Unit(SECONDS.into()).converts_to(&Type::Unit(TICKS.into())));
    assert!(!Type::Unit(TICKS.into()).converts_to(&Type::Unit(SECONDS.into())));
    assert!(!Type::Unit(TICKS.into()).converts_to(&Type::Unit(BLOCKS.into())));
    assert!(Type::String.converts_to(&Type::Text));
    assert!(!Type::Bool.converts_to(&Type::Int));
    assert!(Type::Unknown.converts_to(&Type::Bool));
    assert_eq!(
        Type::List(Box::new(Type::Int.nullable())).to_string(),
        "List<int?>"
    );
}
