//! Mirrors' shared memory (`docs/design/bus.md`, "Mirrors"): what the
//! runtime lays out and every mod and the Java host read in place. A mod
//! gets these through the bus (`yog:mirror/declare` answers their
//! addresses); Java sees each as a direct buffer at the same offsets.
//!
//! A mirror's rows (which keys its provider fills) are a bit column of
//! its key space.
//!
//! Every block is guarded by a seqlock: its writer makes `seq` odd, writes,
//! makes it even again; a reader reads `seq`, the values, `seq` again, and
//! retries when it changed or was odd. Readers never block the writer.

use yog_sync::atomic::{AtomicPtr, AtomicU32};

/// One fact's values in one key space, `width` bytes a row: an inode,
/// stored and written once however many mirrors link to it.
#[repr(C)]
pub struct YogColumn {
    /// The seqlock (odd while written).
    pub seq: AtomicU32,
    /// Bytes a row.
    pub width: u32,
    /// Rows `data` holds.
    pub capacity: AtomicU32,
    /// Bumped whenever `data` moves (it grew): a view over the old one is stale.
    pub generation: AtomicU32,
    /// `capacity * width` bytes. An old buffer stays readable until the
    /// column is freed, so a reader racing a growth reads stale bytes, not
    /// freed ones, and its seqlock check sends it round again.
    pub data: AtomicPtr<u8>,
}

/// A key's slot in a key space's index (open addressing, linear probing).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct YogKeySlot {
    pub key: u64,
    /// The key's row, or [`EMPTY_SLOT`] / [`GONE_SLOT`].
    pub row: u32,
    pub _pad: u32,
}

/// A slot no key ever took: a probe stops here.
pub const EMPTY_SLOT: u32 = u32::MAX;
/// A slot whose key left: a probe goes on past it.
pub const GONE_SLOT: u32 = u32::MAX - 1;

/// The things keyed alike (entities by network id, players, blocks by
/// position): key → row, shared by every column of the space.
#[repr(C)]
pub struct YogKeySpace {
    /// The seqlock over all of it (its columns' own `seq`s aren't used).
    pub seq: AtomicU32,
    /// Rows ever used: a reader walks `0..rows`, skipping the dead.
    pub rows: AtomicU32,
    /// A row's key (`u64`).
    pub keys: *const YogColumn,
    /// A bit a row: whether it holds a key now (`u64` words).
    pub live: *const YogColumn,
    /// [`YogKeySlot`]s, a power of two of them.
    pub index: *const YogColumn,
    /// The space it's part of (a player is an entity), null if none.
    pub parent: *const YogKeySpace,
    /// A row's key in `parent` (`u64`), when it has one.
    pub parent_keys: *const YogColumn,
}

unsafe impl Send for YogColumn {}
unsafe impl Sync for YogColumn {}
unsafe impl Send for YogKeySpace {}
unsafe impl Sync for YogKeySpace {}
