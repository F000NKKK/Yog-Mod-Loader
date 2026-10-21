//! The game's item definitions (`assets/<ns>/items/<id>.json`, 1.21.4+):
//! their fields and the kinds of model and tint the build writes.

pub(crate) const MODEL: &str = yog_ids::word::MODEL;
pub(crate) const TYPE: &str = yog_ids::word::TYPE;
pub(crate) const TINTS: &str = "tints";
pub(crate) const VALUE: &str = yog_ids::word::VALUE;
/// A plain model.
pub(crate) const PLAIN: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, yog_ids::word::MODEL);
/// A tint of one colour.
pub(crate) const CONSTANT: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "constant");
