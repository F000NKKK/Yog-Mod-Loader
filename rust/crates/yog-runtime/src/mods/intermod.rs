//! The `YogApi` table a mod registers through.
use yog_abi::ABI_VERSION;
use yog_abi::YogApi;

pub(crate) fn build_api_table() -> YogApi {
    YogApi {
        abi_version: ABI_VERSION,
        size: std::mem::size_of::<YogApi>() as u32,
        bus: crate::bus::table(),
        gfx: crate::gfx::frame::api_gfx,
    }
}
