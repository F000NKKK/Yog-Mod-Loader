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

/// Heap-allocated UTF-8 string owned by the RUNTIME.
/// `ptr == null` encodes `None` / "not found".
/// When `ptr` is non-null the caller MUST free it via `YogApi::free_str`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogOwnedStr {
    pub ptr: *mut u8,
    pub len: u32,
}

impl YogOwnedStr {
    pub const NONE: Self = Self {
        ptr: std::ptr::null_mut(),
        len: 0,
    };

    /// Allocate a new `YogOwnedStr` from a Rust `String`.
    pub fn from_string(s: String) -> Self {
        let len = s.len() as u32;
        let ptr = Box::into_raw(s.into_bytes().into_boxed_slice()) as *mut u8;
        Self { ptr, len }
    }

    #[inline]
    pub fn is_none(self) -> bool {
        self.ptr.is_null()
    }
}

/// Integer 3-D block position.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogBlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Float 3-D vector (position, velocity, …).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
