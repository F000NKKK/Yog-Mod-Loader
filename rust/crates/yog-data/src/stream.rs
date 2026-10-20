//! [`Stream`]: a list kept in the binary form — its items written as they
//! come (no `Vec` of them on the way) and read back one at a time, when
//! asked. For the big ones: an area of blocks, a picture's pixels, a log.
//!
//! ```
//! use yog_data::{Schema, Stream};
//!
//! #[derive(Schema)]
//! struct Area {
//!     name: String,
//!     blocks: Stream<u16>,
//! }
//!
//! let area = Area { name: "pit".into(), blocks: (0..1000u16).map(|i| i % 3).collect() };
//! let back: Area = yog_data::read_bytes(&yog_data::to_bytes(&area)).unwrap();
//! assert_eq!(back.blocks.len(), 1000);
//! assert_eq!(back.blocks.iter().filter(|b| *b == 2).count(), 333);
//! ```

use std::fmt;
use std::marker::PhantomData;

use crate::binary::{Decoder, Writer};
use crate::borrowed::ValueRef;
use crate::schema::{Reader, Schema, Shape};
use crate::value::{Seg, Value};

/// A list of `T` kept written: its body in the binary form, its strings its own.
pub struct Stream<T> {
    /// The items, written one after another.
    items: Vec<u8>,
    len: usize,
    item: PhantomData<fn() -> T>,
}

impl<T: Schema> Stream<T> {
    /// Writes items one by one ([`StreamWriter::push`]).
    pub fn writer() -> StreamWriter<T> {
        StreamWriter { w: Writer::body(), len: 0, item: PhantomData }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The items, read as they're asked for; it stops at one it can't read.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let mut d = Decoder::body(&self.items);
        (0..self.len).map_while(move |_| T::decode(&mut d))
    }

    /// Every item, read now.
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().collect()
    }

    /// How many bytes it takes.
    pub fn bytes_len(&self) -> usize {
        self.items.len()
    }
}

/// Writes a [`Stream`]'s items as they come.
pub struct StreamWriter<T> {
    w: Writer,
    len: usize,
    item: PhantomData<fn() -> T>,
}

impl<T: Schema> StreamWriter<T> {
    pub fn push(&mut self, item: &T) {
        item.encode(&mut self.w);
        self.len += 1;
    }

    pub fn finish(self) -> Stream<T> {
        Stream { items: self.w.finish(), len: self.len, item: PhantomData }
    }
}

impl<T: Schema> FromIterator<T> for Stream<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut w = Stream::writer();
        for item in items {
            w.push(&item);
        }
        w.finish()
    }
}

impl<T: Schema> From<Vec<T>> for Stream<T> {
    fn from(items: Vec<T>) -> Self {
        items.into_iter().collect()
    }
}

impl<T> Clone for Stream<T> {
    fn clone(&self) -> Self {
        Stream { items: self.items.clone(), len: self.len, item: PhantomData }
    }
}

impl<T> PartialEq for Stream<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.items == other.items
    }
}

impl<T> Default for Stream<T> {
    fn default() -> Self {
        Stream { items: Vec::new(), len: 0, item: PhantomData }
    }
}

impl<T> fmt::Debug for Stream<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stream({} items, {} bytes)", self.len, self.items.len())
    }
}

impl<T: Schema> Schema for Stream<T> {
    fn shape() -> Shape {
        Shape::List(T::shape)
    }
    /// The slow way (a text file, a value): read whole, then written.
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let ValueRef::List(items) = v else {
            cx.expected("a list", v);
            return None;
        };
        let mut w = Stream::writer();
        let mut ok = true;
        for (i, item) in items.iter().enumerate() {
            match cx.at(Seg::Index(i), |cx| T::read(item, cx)) {
                Some(x) => w.push(&x),
                None => ok = false,
            }
        }
        ok.then(|| w.finish())
    }
    fn write(&self) -> Value {
        Value::List(self.iter().map(|item| item.write()).collect())
    }
    fn encode(&self, w: &mut Writer) {
        w.stream_body(self.len, &self.items);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        let mut body = Decoder::body(d.stream()?);
        let len = body.stream_count()?;
        Some(Stream { items: body.rest().to_vec(), len, item: PhantomData })
    }
}
