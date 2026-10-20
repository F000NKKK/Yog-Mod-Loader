//! The `YogApi` table a mod registers through.

use super::*;

pub(super) fn build_api_table() -> YogApi {
    YogApi {
        abi_version: ABI_VERSION,
        size: std::mem::size_of::<YogApi>() as u32,
        bus: crate::bus::table(),
        gfx: crate::gfx_frame::api_gfx,
    }
}
