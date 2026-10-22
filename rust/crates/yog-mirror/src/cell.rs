//! What a column holds a row of: a fixed-width value, little-endian, the
//! same bytes Java reads ([`Kind`] and a count describe it to both sides).

/// The number a cell is made of.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    U8,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl Kind {
    /// Bytes one takes.
    pub const fn size(self) -> usize {
        match self {
            Kind::U8 => 1,
            Kind::I32 | Kind::U32 | Kind::F32 => 4,
            Kind::I64 | Kind::U64 | Kind::F64 => 8,
        }
    }

    /// Its number on the wire (what [`Kind::from_code`] reads back).
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn from_code(code: u8) -> Option<Kind> {
        Some(match code {
            0 => Kind::U8,
            1 => Kind::I32,
            2 => Kind::U32,
            3 => Kind::I64,
            4 => Kind::U64,
            5 => Kind::F32,
            6 => Kind::F64,
            _ => return None,
        })
    }
}

/// A column's row: `COUNT` numbers of one [`Kind`].
pub trait Cell: Copy + Default + 'static {
    const KIND: Kind;
    const COUNT: u32;
    /// Bytes a row.
    const WIDTH: usize = Self::KIND.size() * Self::COUNT as usize;
    /// Writes itself into `out` (`WIDTH` bytes).
    fn put(&self, out: &mut [u8]);
    /// Reads one back from `bytes` (`WIDTH` bytes).
    fn take(bytes: &[u8]) -> Self;
}

macro_rules! numbers {
    ($($t:ty => $kind:ident),* $(,)?) => {$(
        impl Cell for $t {
            const KIND: Kind = Kind::$kind;
            const COUNT: u32 = 1;
            #[inline]
            fn put(&self, out: &mut [u8]) {
                out[..Self::WIDTH].copy_from_slice(&self.to_le_bytes());
            }
            #[inline]
            fn take(bytes: &[u8]) -> Self {
                let mut b = [0u8; std::mem::size_of::<$t>()];
                b.copy_from_slice(&bytes[..Self::WIDTH]);
                <$t>::from_le_bytes(b)
            }
        }
    )*};
}

numbers!(u8 => U8, i32 => I32, u32 => U32, i64 => I64, u64 => U64, f32 => F32, f64 => F64);

impl Cell for bool {
    const KIND: Kind = Kind::U8;
    const COUNT: u32 = 1;
    #[inline]
    fn put(&self, out: &mut [u8]) {
        out[0] = u8::from(*self);
    }
    #[inline]
    fn take(bytes: &[u8]) -> Self {
        bytes[0] != 0
    }
}

impl<T: Cell, const N: usize> Cell for [T; N]
where
    [T; N]: Default,
{
    const KIND: Kind = T::KIND;
    const COUNT: u32 = T::COUNT * N as u32;
    #[inline]
    fn put(&self, out: &mut [u8]) {
        let out = &mut out[..Self::WIDTH];
        for (i, v) in self.iter().enumerate() {
            v.put(&mut out[i * T::WIDTH..]);
        }
    }
    #[inline]
    fn take(bytes: &[u8]) -> Self {
        let bytes = &bytes[..Self::WIDTH];
        std::array::from_fn(|i| T::take(&bytes[i * T::WIDTH..]))
    }
}

/// A cell holding no atom.
pub const NO_ATOM: u32 = u32::MAX;

/// An atom (a name kept once a process: an entity type, a dimension) as
/// its number; [`NO_ATOM`] for none.
impl Cell for Option<yog_data::Atom> {
    const KIND: Kind = Kind::U32;
    const COUNT: u32 = 1;

    #[inline]
    fn put(&self, out: &mut [u8]) {
        self.map_or(NO_ATOM, yog_data::Atom::number).put(out);
    }

    #[inline]
    fn take(bytes: &[u8]) -> Self {
        match u32::take(bytes) {
            NO_ATOM => None,
            n => yog_data::Atom::from_number(n),
        }
    }
}
