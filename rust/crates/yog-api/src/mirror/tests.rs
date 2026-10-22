//! What `#[yog_mirror]` makes of a struct.

use crate::mirror::{Kind, Mirror};
use crate::yog_mirror;
use yog_ids::{fact, key_space};

#[yog_mirror(space = key_space::ENTITY, owner = "pipes")]
pub struct PipeFlow {
    #[fact(fact::ENTITY_POS)]
    pub pos: [f64; 3],
    pub level: f32,
}

#[test]
fn a_struct_is_a_mirror_with_a_bit_a_field() {
    assert_eq!((PipeFlow::POS, PipeFlow::LEVEL, PipeFlow::ALL), (1, 2, 3));
    let spec = PipeFlow::spec();
    assert_eq!(spec.name, "yog:mirror/pipes/pipe_flow");
    assert_eq!(spec.space, key_space::ENTITY);
    let [pos, level] = &spec.fields[..] else {
        panic!("two fields: {:?}", spec.fields);
    };
    assert_eq!(
        (pos.fact.as_deref(), pos.kind(), pos.count),
        (Some(fact::ENTITY_POS), Some(Kind::F64), 3)
    );
    assert_eq!(
        (level.fact.as_deref(), level.kind(), level.count),
        (None, Some(Kind::F32), 1)
    );
}
