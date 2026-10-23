//! A JVM class file read for what the class index needs (JVMS §4): its
//! name, access, what it extends and implements, its fields and methods by
//! name, descriptor and access. Code and other attributes are skipped.

mod pool;

use pool::Pool;
use yog_script_analyze::index::Member;

/// What every class file starts with.
const MAGIC: u32 = 0xCAFE_BABE;

/// A class as its file declares it (internal names).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassFile {
    pub(crate) name: String,
    pub(crate) access: u16,
    pub(crate) extends: Option<String>,
    pub(crate) implements: Vec<String>,
    pub(crate) fields: Vec<Member>,
    pub(crate) methods: Vec<Member>,
}

/// Bytes read in order; `None` past their end.
pub(super) struct Reader<'b> {
    bytes: &'b [u8],
    at: usize,
}

impl<'b> Reader<'b> {
    pub(super) fn take(&mut self, n: usize) -> Option<&'b [u8]> {
        let out = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(out)
    }

    pub(super) fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    pub(super) fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// `count` attributes (name, length, bytes), skipped.
    fn skip_attributes(&mut self) -> Option<()> {
        for _ in 0..self.u16()? {
            self.u16()?;
            let len = self.u32()? as usize;
            self.take(len)?;
        }
        Some(())
    }
}

/// The class file `bytes`; `None` for what isn't one.
pub(crate) fn read(bytes: &[u8]) -> Option<ClassFile> {
    let mut r = Reader { bytes, at: 0 };
    if r.u32()? != MAGIC {
        return None;
    }
    // The minor and major versions.
    r.take(4)?;
    let pool = Pool::read(&mut r)?;
    let access = r.u16()?;
    let name = pool.class(r.u16()?)?.to_owned();
    let extends = match r.u16()? {
        0 => None,
        i => Some(pool.class(i)?.to_owned()),
    };
    let implements = (0..r.u16()?)
        .map(|_| pool.class(r.u16()?).map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    let fields = members(&mut r, &pool)?;
    let methods = members(&mut r, &pool)?;
    Some(ClassFile {
        name,
        access,
        extends,
        implements,
        fields,
        methods,
    })
}

/// A count of fields or methods: access, name, descriptor, attributes.
fn members(r: &mut Reader<'_>, pool: &Pool) -> Option<Vec<Member>> {
    (0..r.u16()?)
        .map(|_| {
            let access = r.u16()?;
            let name = pool.utf8(r.u16()?)?.to_owned();
            let desc = pool.utf8(r.u16()?)?.to_owned();
            r.skip_attributes()?;
            Some(Member { name, desc, access })
        })
        .collect()
}

#[cfg(test)]
mod tests;
