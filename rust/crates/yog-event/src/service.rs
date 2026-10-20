//! Services on the bus: whoever provides an address (the host, the runtime,
//! a Yog mod, a Java mod) answers calls to it with a reply.

use yog_data::{Schema, Shape};
use yog_ids as ids;

use crate::{block, command, item, key_binding, menu};
use crate::message::{Message, branch, last_segment};

/// A call the bus carries: its payload is `Self`, its answer a [`Service::Reply`].
pub trait Service: Message {
    /// Where it's provided.
    const ADDRESS: &'static str;
    /// The field whose value picks the provider's branch (`block_id`: each
    /// block answers its own), if it's keyed — see [`crate::keyed`].
    const KEY: Option<&'static str>;
    /// What the provider answers.
    type Reply: Schema;

    /// The key field's value, if it's keyed.
    fn key(&self) -> Option<&str>;

    /// Where this call goes: the address, or its branch for the key.
    fn target(&self) -> String {
        crate::message::target(Self::ADDRESS, self.key())
    }
}

/// A service's descriptor: its domain and name, address and shapes.
pub struct ServiceInfo {
    /// Its place in the tree: `block`, `world`…
    pub domain: &'static str,
    pub name: &'static str,
    pub address: &'static str,
    pub key: Option<&'static str>,
    pub shape: Shape,
    pub reply: Shape,
    /// The Java host's method answering it (`NativeBridge.<host>`), for an action.
    pub host: Option<&'static str>,
}

/// Each domain's actions.
const ACTIONS: &[fn() -> Vec<ServiceInfo>] = &[crate::chat::actions, crate::world::actions];

macro_rules! services {
    ($($ty:path => $address:expr, $reply:ty $(, key = $key:ident)?;)*) => {
        $(impl Service for $ty {
            const ADDRESS: &'static str = $address;
            const KEY: Option<&'static str> = key!($($key)?);
            type Reply = $reply;

            fn key(&self) -> Option<&str> {
                key_value!(self, $($key)?)
            }
        })*

        /// The services the runtime, the mods or the host's own code provide.
        fn listed() -> Vec<ServiceInfo> {
            vec![$(ServiceInfo {
                domain: branch($address, ids::service::PREFIX),
                name: last_segment(stringify!($ty)),
                address: $address,
                key: key!($($key)?),
                shape: <$ty as Schema>::shape(),
                reply: <$reply as Schema>::shape(),
                host: None,
            }),*]
        }
    };
}

/// Every service, domain by domain: the listed ones, then the actions.
pub fn services() -> Vec<ServiceInfo> {
    let mut all = listed();
    for actions in ACTIONS {
        all.extend(actions());
    }
    all
}

services! {
    block::Insert => ids::service_block::INSERT, bool, key = block_id;
    item::Text => ids::service_item::TEXT, Option<item::Shown>, key = item;
    item::Picture => ids::service_item::PICTURE, Option<[f32; 2]>, key = item;
    command::CommandSpec => ids::service_command::DECLARE, bool;
    command::Declared => ids::service_command::DECLARED, Vec<command::CommandSpec>;
    menu::MenuEntry => ids::service_menu::DECLARE, bool;
    menu::Declared => ids::service_menu::DECLARED, Vec<menu::MenuEntry>;
    key_binding::KeyBindingSpec => ids::service_key_binding::DECLARE, bool;
    key_binding::Declared => ids::service_key_binding::DECLARED, Vec<key_binding::CategorizedBinding>;
}
