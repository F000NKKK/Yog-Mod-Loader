//! Values kept on an entity — the Rust side of a Yog Script entity's
//! `saved` and `synced` members (`docs/design/storage.md`): the runtime's
//! entity data, at the same `<mod>:<key>` keys — saved with the entity,
//! sent to the players who see it, gone with it.

use std::marker::PhantomData;

use yog_data::Schema;

/// A value kept on each entity, by uuid, at its key (the mod's own:
/// `<mod>:<key>`); its default where the entity has none yet.
///
/// ```ignore
/// static HITS: EntityData<u32> = EntityData::new("hits");
/// HITS.with_mut(&damage.uuid, |n| *n += 1);
/// ```
pub struct EntityData<T> {
    key: &'static str,
    persist: bool,
    value: PhantomData<fn() -> T>,
}

impl<T: Schema + Default> EntityData<T> {
    /// Saved with the entity.
    pub const fn new(key: &'static str) -> Self {
        EntityData {
            key,
            persist: true,
            value: PhantomData,
        }
    }

    /// Kept while the entity is loaded, not saved (a Yog Script member
    /// `synced` and not `saved`).
    pub const fn session(key: &'static str) -> Self {
        EntityData {
            key,
            persist: false,
            value: PhantomData,
        }
    }

    /// What `read` makes of entity `uuid`'s value: the server's where it
    /// runs here, else what this client was sent.
    pub fn with<R>(&self, uuid: &str, read: impl FnOnce(&T) -> R) -> R {
        let key = self.entity_key();
        let bytes =
            crate::event::entity::data(uuid, &key).or_else(|| crate::body::entity_data(uuid, &key));
        read(
            &bytes
                .and_then(|b| yog_data::read_bytes(&b).ok())
                .unwrap_or_default(),
        )
    }

    /// Entity `uuid`'s value changed by `change`, and set (server side:
    /// sent to the players who see it by the tick's end).
    pub fn with_mut<R>(&self, uuid: &str, change: impl FnOnce(&mut T) -> R) -> R {
        let key = self.entity_key();
        let mut value: T = crate::event::entity::data(uuid, &key)
            .and_then(|b| yog_data::read_bytes(&b).ok())
            .unwrap_or_default();
        let out = change(&mut value);
        crate::event::entity::set_data(uuid, key, Some(&yog_data::to_bytes(&value)), self.persist);
        out
    }

    /// The key in an entity's data: the mod's, then its own (as Yog Script's are).
    fn entity_key(&self) -> String {
        format!("{}{}{}", crate::mod_id(), yog_ids::sep::NAMESPACE, self.key)
    }
}
