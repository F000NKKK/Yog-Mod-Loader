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

/// A list of anything but numbers, read lazily: its bytes and how many
/// items — each viewed as it's reached ([`List::iter`]), nothing allocated.
pub struct List<'a, T> {
    u: Unpacker<'a>,
    len: usize,
    item: PhantomData<fn() -> T>,
}

impl<'a, T> Clone for List<'a, T> {
    fn clone(&self) -> Self {
        List {
            u: self.u.fork(self.u.position()),
            len: self.len,
            item: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for List<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "List({} items)", self.len)
    }
}

impl<'a, T: View<'a>> List<'a, T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Each item's view, read as it's reached.
    pub fn iter(&self) -> ListIter<'a, T> {
        ListIter {
            u: self.u.fork(self.u.position()),
            left: self.len,
            item: PhantomData,
        }
    }

    /// Item `i` (the ones before it stepped over).
    pub fn get(&self, i: usize) -> Option<T::View> {
        self.iter().nth(i)
    }
}

/// [`List::iter`].
pub struct ListIter<'a, T> {
    u: Unpacker<'a>,
    left: usize,
    item: PhantomData<fn() -> T>,
}

impl<'a, T: View<'a>> Iterator for ListIter<'a, T> {
    type Item = T::View;
    fn next(&mut self) -> Option<T::View> {
        self.left = self.left.checked_sub(1)?;
        T::unpack_view(&mut self.u)
    }
}

/// A list ([`super::Packer::begin_list`]) as a [`List`]: stepped over, read later.
pub fn list<'a, T: View<'a>>(u: &mut Unpacker<'a>) -> Option<List<'a, T>> {
    let (len, end) = u.list()?;
    let at = u.position();
    u.skip_to(end)?;
    Some(List {
        u: u.fork(at),
        len,
        item: PhantomData,
    })
}

impl<'a> View<'a> for bool {
    type View = bool;
    type ListView = List<'a, bool>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<bool> {
        u.get_bool()
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list(u)
    }
}

impl<'a> View<'a> for String {
    type View = &'a str;
    type ListView = List<'a, Self>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<&'a str> {
        u.get_str()
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list(u)
    }
}

impl<'a> View<'a> for Box<str> {
    type View = &'a str;
    type ListView = List<'a, Self>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<&'a str> {
        u.get_str()
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list(u)
    }
}

impl<'a> View<'a> for crate::Atom {
    type View = crate::Atom;
    type ListView = List<'a, crate::Atom>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<crate::Atom> {
        <crate::Atom as crate::Schema>::unpack(u)
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list(u)
    }
}

impl<'a, T: View<'a>> View<'a> for Option<T> {
    type View = Option<T::View>;
    type ListView = List<'a, Option<T>>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<Option<T::View>> {
        if u.get_bool()? {
            T::unpack_view(u).map(Some)
        } else {
            Some(None)
        }
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list(u)
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
    type ListView = List<'a, Vec<T>>;
    fn unpack_view(u: &mut Unpacker<'a>) -> Option<T::ListView> {
        T::unpack_list_view(u)
    }
    fn unpack_list_view(u: &mut Unpacker<'a>) -> Option<Self::ListView> {
        list(u)
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

impl<T: TryFrom<i128>> Ints<'_, T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Item `i`, if there's one: read in place, O(1).
    pub fn get(&self, i: usize) -> Option<T> {
        if i >= self.len {
            return None;
        }
        let raw = super::lanes::get(self.bytes, self.len, self.bits, i);
        let n = if self.signed {
            i128::from(crate::binary::unzigzag(raw))
        } else {
            i128::from(raw)
        };
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
