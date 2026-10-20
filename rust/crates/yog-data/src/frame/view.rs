//! Views: a frame read without copying — strings borrowed from it, packed
//! numbers read in place (no alignment asked), nested records viewed too.
//! One pass checks the frame and finds every field; nothing is allocated
//! for strings or number lists. A struct gets its view with
//! `#[derive(Schema)] #[data(view)]` (`MyStructView<'a>`).
//!
//! ```
//! #[derive(yog_data::Schema)]
//! #[data(view)]
//! struct Area { name: String, blocks: Vec<u16> }
//!
//! let frame = yog_data::frame::to_frame(&Area { name: "pit".into(), blocks: vec![0, 2, 1] });
//! let area = yog_data::frame::read_view::<Area>(&frame).unwrap();
//! assert_eq!(area.name, "pit");
//! assert_eq!(area.blocks.get(1), Some(2));
//! ```

use std::marker::PhantomData;

use super::Unpacker;

/// A type read as a view of a frame: what it is borrowed (`View`), and what
/// a list of it is (`ListView`).
pub trait View<'a>: Sized {
    type View;
    type ListView;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<Self::View>;
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView>;
}

/// `T`'s view of the frame `bytes`.
pub fn read_view<'a, T: View<'a>>(bytes: &'a [u8]) -> Option<T::View> {
    let mut u = Unpacker::open(bytes)?;
    let v = T::unpack_view(&mut u)?;
    u.at_end().then_some(v)
}

/// A list of anything else, read into a `Vec` of views.
fn list_of<'a, T: View<'a>>(u: &mut Unpacker<'a>) -> Option<Vec<T::View>> {
    let n = u.count()?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(T::unpack_view(u)?);
    }
    Some(out)
}

impl<'a> View<'a> for bool {
    type View = bool;
    type ListView = Vec<bool>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<bool> {
        u.get_bool()
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Vec<bool>> {
        list_of::<bool>(u)
    }
}

impl<'a> View<'a> for String {
    type View = &'a str;
    type ListView = Vec<&'a str>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<&'a str> {
        u.get_str()
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Vec<&'a str>> {
        list_of::<String>(u)
    }
}

impl<'a> View<'a> for Box<str> {
    type View = &'a str;
    type ListView = Vec<&'a str>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<&'a str> {
        u.get_str()
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Vec<&'a str>> {
        list_of::<Box<str>>(u)
    }
}

impl<'a> View<'a> for crate::Atom {
    type View = crate::Atom;
    type ListView = Vec<crate::Atom>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<crate::Atom> {
        <crate::Atom as crate::Schema>::unpack(u)
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Vec<crate::Atom>> {
        list_of::<crate::Atom>(u)
    }
}

impl<'a, T: View<'a>> View<'a> for Option<T> {
    type View = Option<T::View>;
    type ListView = Vec<Option<T::View>>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<Option<T::View>> {
        if u.get_bool()? { T::unpack_view(u).map(Some) } else { Some(None) }
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list_of::<Option<T>>(u)
    }
}

impl<'a, T: View<'a>> View<'a> for Box<T> {
    type View = T::View;
    type ListView = T::ListView;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<T::View> {
        T::unpack_view(u)
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<T::ListView> {
        T::unpack_list_view(u)
    }
}

impl<'a, T: View<'a>> View<'a> for Vec<T> {
    type View = T::ListView;
    type ListView = Vec<T::ListView>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<T::ListView> {
        T::unpack_list_view(u)
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list_of::<Vec<T>>(u)
    }
}

/// A packed list of ints, read in place: [`Ints::get`] takes one out of the bits.
#[derive(Debug, Clone, Copy)]
pub struct Ints<'a, T> {
    bytes: &'a [u8],
    len: usize,
    bits: u32,
    signed: bool,
    item: PhantomData<fn() -> T>,
}

/// Bytes a packed int may span (64 bits starting anywhere in a byte).
const INT_SPAN: usize = 9;
const BYTE_BITS: usize = 8;

impl<T: TryFrom<i128>> Ints<'_, T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Item `i`, if there's one.
    pub fn get(&self, i: usize) -> Option<T> {
        if i >= self.len {
            return None;
        }
        let bit = i * self.bits as usize;
        let (byte, shift) = (bit / BYTE_BITS, bit % BYTE_BITS);
        let mut window = [0u8; 16];
        let end = (byte + INT_SPAN).min(self.bytes.len());
        window[..end - byte].copy_from_slice(&self.bytes[byte..end]);
        let mask = u128::from(u64::MAX) >> (u64::BITS - self.bits);
        let raw = ((u128::from_le_bytes(window) >> shift) & mask) as u64;
        let n = if self.signed { i128::from(crate::binary::unzigzag(raw)) } else { i128::from(raw) };
        T::try_from(n).ok()
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        (0..self.len).map_while(|i| self.get(i))
    }
}

/// A list of floats, read in place.
#[derive(Debug, Clone, Copy)]
pub struct Floats<'a, T> {
    bytes: &'a [u8],
    len: usize,
    /// Kept as `f32`s.
    narrow: bool,
    item: PhantomData<fn() -> T>,
}

const F32_BYTES: usize = 4;
const F64_BYTES: usize = 8;

impl<T: From<f32> + FromF64> Floats<'_, T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, i: usize) -> Option<T> {
        if i >= self.len {
            return None;
        }
        if self.narrow {
            let b = self.bytes.get(i * F32_BYTES..(i + 1) * F32_BYTES)?;
            Some(T::from(f32::from_le_bytes(b.try_into().ok()?)))
        } else {
            let b = self.bytes.get(i * F64_BYTES..(i + 1) * F64_BYTES)?;
            Some(T::from_f64(f64::from_le_bytes(b.try_into().ok()?)))
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        (0..self.len).map_while(|i| self.get(i))
    }
}

/// A float out of a `f64` (a `f32` rounds).
pub trait FromF64 {
    fn from_f64(f: f64) -> Self;
}

impl FromF64 for f64 {
    fn from_f64(f: f64) -> Self {
        f
    }
}

impl FromF64 for f32 {
    fn from_f64(f: f64) -> Self {
        f as f32
    }
}

macro_rules! int_views {
    ($($t:ty => $signed:expr),*) => {$(
        impl<'a> View<'a> for $t {
            type View = $t;
            type ListView = Ints<'a, $t>;
            fn unpack_view(u: &mut Unpacker<'a>) -> Option<$t> {
                <$t as crate::Schema>::unpack(u)
            }
            fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Ints<'a, $t>> {
                let (len, bits, bytes) = u.packed_ints()?;
                Some(Ints { bytes, len, bits, signed: $signed, item: PhantomData })
            }
        }
    )*};
}
int_views!(i8 => true, i16 => true, i32 => true, i64 => true, u8 => false, u16 => false, u32 => false, u64 => false, usize => false);

macro_rules! float_views {
    ($($t:ty => $wide:expr),*) => {$(
        impl<'a> View<'a> for $t {
            type View = $t;
            type ListView = Floats<'a, $t>;
            fn unpack_view(u: &mut Unpacker<'a>) -> Option<$t> {
                <$t as crate::Schema>::unpack(u)
            }
            fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Floats<'a, $t>> {
                let (len, narrow, bytes) = u.packed_floats($wide)?;
                Some(Floats { bytes, len, narrow, item: PhantomData })
            }
        }
    )*};
}
float_views!(f32 => false, f64 => true);
