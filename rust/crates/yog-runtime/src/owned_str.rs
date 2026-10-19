//! The allocator behind `YogOwnedStr`.

// ── Free-str allocator used by YogOwnedStr ────────────────────────────────────

pub(super) unsafe extern "C" fn yog_free_str(ptr: *mut u8, len: u32) {
    unsafe {
        if !ptr.is_null() {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                ptr,
                len as usize,
            )));
        }
    }
}
