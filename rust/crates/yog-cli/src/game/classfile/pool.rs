//! A class file's constant pool: the strings and class names the rest of
//! the file refers to by index; the other constants skipped.

use super::Reader;

/// Constant tags (JVMS §4.4) and the bytes each takes after its tag.
const UTF8: u8 = 1;
const CLASS: u8 = 7;
/// Long and Double: eight bytes, and they take two slots.
const LONG: u8 = 5;
const DOUBLE: u8 = 6;
const WIDE: usize = 8;
/// The rest by the bytes they take.
const SIZES: [(u8, usize); 13] = [
    (3, 4),  // Integer
    (4, 4),  // Float
    (8, 2),  // String
    (9, 4),  // Fieldref
    (10, 4), // Methodref
    (11, 4), // InterfaceMethodref
    (12, 4), // NameAndType
    (15, 3), // MethodHandle
    (16, 2), // MethodType
    (17, 4), // Dynamic
    (18, 4), // InvokeDynamic
    (19, 2), // Module
    (20, 2), // Package
];

enum Entry {
    Utf8(String),
    /// A class: its name's index.
    Class(u16),
    Other,
}

pub(super) struct Pool {
    entries: Vec<Entry>,
}

impl Pool {
    pub(super) fn read(r: &mut Reader<'_>) -> Option<Pool> {
        let count = r.u16()?;
        // Slot 0 is unused; a long or a double takes two.
        let mut entries = vec![Entry::Other];
        while entries.len() < usize::from(count) {
            let tag = r.u8()?;
            let entry = match tag {
                UTF8 => {
                    let len = usize::from(r.u16()?);
                    Entry::Utf8(String::from_utf8_lossy(r.take(len)?).into_owned())
                }
                CLASS => Entry::Class(r.u16()?),
                LONG | DOUBLE => {
                    r.take(WIDE)?;
                    entries.push(Entry::Other);
                    Entry::Other
                }
                _ => {
                    let size = SIZES.iter().find(|(t, _)| *t == tag)?.1;
                    r.take(size)?;
                    Entry::Other
                }
            };
            entries.push(entry);
        }
        Some(Pool { entries })
    }

    pub(super) fn utf8(&self, i: u16) -> Option<&str> {
        match self.entries.get(usize::from(i))? {
            Entry::Utf8(s) => Some(s),
            _ => None,
        }
    }

    pub(super) fn class(&self, i: u16) -> Option<&str> {
        match self.entries.get(usize::from(i))? {
            Entry::Class(name) => self.utf8(*name),
            _ => None,
        }
    }
}
