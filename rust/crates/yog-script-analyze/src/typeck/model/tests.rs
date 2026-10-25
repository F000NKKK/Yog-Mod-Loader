use super::*;

#[test]
fn conversions_follow_the_grammar() {
    assert!(Type::I32.converts_to(&Type::F64));
    assert!(!Type::F64.converts_to(&Type::I32));
    assert!(Type::Null.converts_to(&Type::own("Player").nullable()));
    assert!(!Type::Null.converts_to(&Type::own("Player")));
    assert!(Type::I32.converts_to(&Type::I32.nullable()));
    assert!(Type::Unit(SECONDS.into()).converts_to(&Type::Unit(TICKS.into())));
    assert!(!Type::Unit(TICKS.into()).converts_to(&Type::Unit(SECONDS.into())));
    assert!(!Type::Unit(TICKS.into()).converts_to(&Type::Unit(BLOCKS.into())));
    assert!(Type::String.converts_to(&Type::Text));
    assert!(!Type::Bool.converts_to(&Type::I32));
    assert!(Type::Unknown.converts_to(&Type::Bool));
    assert_eq!(
        Type::Array(Box::new(Type::I32.nullable())).to_string(),
        "i32?[]"
    );
}
