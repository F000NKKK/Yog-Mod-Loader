//! What mods register: content the hosts make into the game's.

use yog_data::Schema;
use yog_data::with::Raw;

/// Content of `kind` (`item`, `block`, `recipe`, `book`, … — `yog_ids::kind`)
/// named `id`, its definition as Yog Data (binary): the runtime keeps it
/// for the hosts. One service for every kind — new kinds need nothing new.
#[derive(Debug, Clone, Schema)]
pub struct Register {
    pub kind: String,
    pub id: String,
    #[data(with = "Raw")]
    pub data: Vec<u8>,
}
