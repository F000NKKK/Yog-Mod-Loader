//! The primitive types: strings, owned strings, positions, vectors.

// ── Primitive types ───────────────────────────────────────────────────────────

/// Borrowed UTF-8 byte slice passed to a function. NOT null-terminated.
/// Valid only for the duration of the call that provides it — never store.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogStr {
    pub ptr: *const u8,
    pub len: u32,
}

impl YogStr {
    pub const EMPTY: Self = Self {
        ptr: std::ptr::null(),
        len: 0,
    };

    /// Borrow `s` — not `FromStr`, which can't return a borrowing value.
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn from_str(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len() as u32,
        }
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.len == 0 || self.ptr.is_null()
    }

    /// # Safety
    ///
    /// `ptr` must be valid UTF-8 of `len` bytes for at least the duration of
    /// the call that provided this `YogStr`.
    #[inline]
    pub unsafe fn as_str<'a>(self) -> &'a str {
        unsafe {
            if self.ptr.is_null() || self.len == 0 {
                return "";
            }
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.ptr, self.len as usize))
        }
    }
}
