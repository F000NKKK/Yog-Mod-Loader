//! The bus described: what each address carries, asked of the bus itself
//! (services the runtime provides) — discovery, checking, bindings.

use yog_data::Schema;

/// What an address is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub enum AddressKind {
    /// Emitted; anyone listens.
    Event,
    /// Called; one provider answers.
    Service,
}

/// An address's descriptor.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct Descriptor {
    pub address: String,
    pub kind: AddressKind,
    pub doc: String,
    /// The payload's shape.
    pub shape: yog_data::ShapeDoc,
    /// A service's reply's shape.
    pub reply: Option<yog_data::ShapeDoc>,
    /// The interfaces it implements (`yog:energy/storage`): what [`Find`] finds it by.
    #[data(default)]
    pub interfaces: Vec<String>,
}

/// Every address under `prefix` (`pipes:`, `yog:event/player/`; empty: all).
#[derive(Debug, Clone, Schema)]
pub struct List {
    pub prefix: String,
}

/// An address's descriptor.
#[derive(Debug, Clone, Schema)]
pub struct Describe {
    pub address: String,
}

/// The addresses implementing `interface`.
#[derive(Debug, Clone, Schema)]
pub struct Find {
    pub interface: String,
}
