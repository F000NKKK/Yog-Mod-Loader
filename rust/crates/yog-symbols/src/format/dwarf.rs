//! Reading the debug info's types: entries, names, sizes, members, the shapes of arrays and wrappers.

use super::{
    Die, ELLIPSIS, Formatter, MAX_CHILDREN, MAX_PEEL, MAX_QUALIFIERS, MAX_TYPE_DEPTH, Member, Off,
    PTR_SIZE,
};
use crate::locals::{Place, unit_ref};
use gimli::{Reader as _, UnitOffset};

impl<'a> Formatter<'a> {
    pub(super) fn entry(&self, off: Off) -> Option<Die> {
        self.info.unit.entry(off).ok()
    }

    pub(super) fn name_of(&self, entry: &Die) -> Option<String> {
        let attr = entry.attr_value(gimli::DW_AT_name)?;
        let s = self.table.dwarf.attr_string(&self.info.unit, attr).ok()?;
        Some(s.to_string_lossy().ok()?.into_owned())
    }

    pub(super) fn ty_of(&self, entry: &Die) -> Option<Off> {
        unit_ref(self.info, entry.attr_value(gimli::DW_AT_type)?)
    }

    pub(super) fn udata(entry: &Die, at: gimli::DwAt) -> Option<u64> {
        entry.attr_value(at)?.udata_value()
    }

    /// Strips typedefs and cv-qualifiers down to the type that decides
    /// the representation.
    pub(super) fn peel(&self, mut off: Off) -> Option<(Off, Die)> {
        for _ in 0..MAX_PEEL {
            let entry = self.entry(off)?;
            match entry.tag() {
                gimli::DW_TAG_typedef
                | gimli::DW_TAG_const_type
                | gimli::DW_TAG_volatile_type
                | gimli::DW_TAG_restrict_type
                | gimli::DW_TAG_atomic_type => match self.ty_of(&entry) {
                    Some(next) => off = next,
                    None => return Some((off, entry)),
                },
                _ => return Some((off, entry)),
            }
        }
        None
    }

    pub(super) fn byte_size(&self, off: Off) -> Option<u64> {
        let (_, entry) = self.peel(off)?;
        if let Some(size) = Self::udata(&entry, gimli::DW_AT_byte_size) {
            return Some(size);
        }
        match entry.tag() {
            gimli::DW_TAG_pointer_type
            | gimli::DW_TAG_reference_type
            | gimli::DW_TAG_rvalue_reference_type => Some(PTR_SIZE),
            gimli::DW_TAG_array_type => {
                let (elem, count) = self.array_shape(&entry)?;
                self.array_stride(&entry, elem?)?.checked_mul(count)
            }
            _ => None,
        }
    }

    pub(super) fn children(&self, off: Off) -> Vec<Die> {
        let mut out = Vec::new();
        let Ok(mut tree) = self.info.unit.entries_tree(Some(off)) else {
            return out;
        };
        let Ok(root) = tree.root() else {
            return out;
        };
        let mut iter = root.children();
        while let Ok(Some(child)) = iter.next() {
            out.push(child.entry().clone());
            if out.len() > MAX_CHILDREN {
                break;
            }
        }
        out
    }

    pub(super) fn members(&self, off: Off) -> Vec<Member> {
        self.children(off)
            .iter()
            .filter(|e| e.tag() == gimli::DW_TAG_member)
            .filter(|e| !e.has_attr(gimli::DW_AT_external) && !e.has_attr(gimli::DW_AT_declaration))
            .map(|e| self.member(e))
            .collect()
    }

    pub(super) fn member(&self, e: &Die) -> Member {
        Member {
            name: self.name_of(e).unwrap_or_default(),
            ty: self.ty_of(e),
            offset: Self::udata(e, gimli::DW_AT_data_member_location).unwrap_or(0),
        }
    }

    /// The member named `name` (a struct's own, not inherited).
    pub(super) fn member_named<'m>(members: &'m [Member], name: &str) -> Option<&'m Member> {
        members.iter().find(|m| m.name == name)
    }

    /// The only member of a wrapper that holds data: `value` for the cells
    /// and `MaybeUninit` (a union whose other member is the `()` "uninit"
    /// view), otherwise the one non-zero-sized field.
    pub(super) fn wrapped_member<'m>(&self, members: &'m [Member]) -> Option<&'m Member> {
        Self::member_named(members, "value").or_else(|| {
            let mut data = members.iter().filter(|m| !self.is_zero_sized(m.ty));
            match (data.next(), data.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            }
        })
    }

    /// Whether the `OnceLock` whose members are `members` at `place` has
    /// been initialized; `None` if its state can't be read. Both `Once`
    /// implementations (futex and queue) keep the state in the low two
    /// bits; COMPLETE is 0 (since Rust 1.80, when INCOMPLETE became 3).
    pub(super) fn once_lock_complete(&self, members: &[Member], place: &Place) -> Option<bool> {
        const STATE_MASK: u64 = 3;
        let once = Self::member_named(members, "once")?;
        let state = self.first_uint(once.ty?, &place.offset(once.offset), 0)?;
        Some(state & STATE_MASK == 0)
    }

    /// The payload member of an `Rc`/`Arc` allocation (`RcInner` /
    /// `ArcInner`).
    pub(super) fn rc_payload(members: &[Member]) -> Option<&Member> {
        Self::member_named(members, "value").or_else(|| Self::member_named(members, "data"))
    }

    /// Whether a pointer to `pointee` can be followed: not a function or an
    /// opaque/unsized type.
    pub(super) fn is_derefable(&self, pointee: Off) -> bool {
        self.peel(pointee).is_some_and(|(off, e)| {
            !matches!(
                e.tag(),
                gimli::DW_TAG_subroutine_type | gimli::DW_TAG_unspecified_type
            ) && self.byte_size(off).is_some_and(|s| s > 0)
        })
    }

    /// Distance between elements of the array type `entry` of `elem`s.
    pub(super) fn array_stride(&self, entry: &Die, elem: Off) -> Option<u64> {
        Self::udata(entry, gimli::DW_AT_byte_stride).or_else(|| self.byte_size(elem))
    }

    pub(super) fn template_param(&self, off: Off, name: &str) -> Option<Off> {
        self.children(off)
            .iter()
            .filter(|e| e.tag() == gimli::DW_TAG_template_type_parameter)
            .find(|e| self.name_of(e).as_deref() == Some(name))
            .and_then(|e| self.ty_of(e))
    }

    /// Element type and element count of an array type (first dimension).
    pub(super) fn array_shape(&self, entry: &Die) -> Option<(Option<Off>, u64)> {
        let elem = self.ty_of(entry);
        let sub = self
            .children(entry.offset())
            .into_iter()
            .find(|e| e.tag() == gimli::DW_TAG_subrange_type)?;
        let count = match Self::udata(&sub, gimli::DW_AT_count) {
            Some(c) => c,
            None => {
                let lower = Self::udata(&sub, gimli::DW_AT_lower_bound).unwrap_or(0);
                let upper = sub.attr_value(gimli::DW_AT_upper_bound)?.udata_value()?;
                upper.checked_sub(lower)?.checked_add(1)?
            }
        };
        Some((elem, count))
    }

    /// `entry`'s name with the namespaces and types enclosing it
    /// (`alloc::vec::Vec<u8, alloc::alloc::Global>`), memoized per unit.
    pub(crate) fn qualified_name(&self, off: Off, entry: &Die) -> String {
        let cache = || {
            self.info
                .qualified_names
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        };
        if let Some(name) = cache().get(&off.0) {
            return name.clone();
        }
        let name = self.build_qualified_name(off, entry);
        cache().insert(off.0, name.clone());
        name
    }

    pub(super) fn build_qualified_name(&self, off: Off, entry: &Die) -> String {
        let mut parts = vec![self.name_of(entry).unwrap_or_else(|| "<anon>".into())];
        let parents = self.info.parents();
        let mut cur = off.0;
        for _ in 0..MAX_QUALIFIERS {
            let Some(&parent) = parents.get(&cur) else {
                break;
            };
            let Some(p) = self.entry(UnitOffset(parent)) else {
                break;
            };
            match p.tag() {
                gimli::DW_TAG_namespace
                | gimli::DW_TAG_structure_type
                | gimli::DW_TAG_union_type
                | gimli::DW_TAG_enumeration_type
                | gimli::DW_TAG_class_type => {
                    parts.push(self.name_of(&p).unwrap_or_else(|| "{anon}".into()));
                }
                _ => break,
            }
            cur = parent;
        }
        parts.reverse();
        parts.join("::")
    }

    // ---- type names --------------------------------------------------

    pub(crate) fn type_name(&self, ty: Option<Off>) -> String {
        self.type_name_depth(ty, 0)
    }

    pub(super) fn type_name_depth(&self, ty: Option<Off>, depth: usize) -> String {
        let Some(off) = ty else {
            return "()".into();
        };
        if depth > MAX_TYPE_DEPTH {
            return ELLIPSIS.into();
        }
        let Some(entry) = self.entry(off) else {
            return "?".into();
        };
        match entry.tag() {
            gimli::DW_TAG_structure_type
            | gimli::DW_TAG_union_type
            | gimli::DW_TAG_enumeration_type
            | gimli::DW_TAG_class_type
            | gimli::DW_TAG_typedef => {
                let name = self.name_of(&entry);
                // Rust's fat pointers (`&str`, `&[T]`, `&dyn Trait`) and
                // tuples are named structs at CU level: keep them bare.
                match name {
                    Some(n) if n.starts_with(['&', '*', '(', '[']) => n,
                    _ => self.qualified_name(off, &entry),
                }
            }
            gimli::DW_TAG_pointer_type
            | gimli::DW_TAG_reference_type
            | gimli::DW_TAG_rvalue_reference_type => self.name_of(&entry).unwrap_or_else(|| {
                let sigil = if entry.tag() == gimli::DW_TAG_pointer_type {
                    "*"
                } else {
                    "&"
                };
                format!(
                    "{sigil}{}",
                    self.type_name_depth(self.ty_of(&entry), depth + 1)
                )
            }),
            gimli::DW_TAG_const_type => format!(
                "const {}",
                self.type_name_depth(self.ty_of(&entry), depth + 1)
            ),
            gimli::DW_TAG_volatile_type
            | gimli::DW_TAG_restrict_type
            | gimli::DW_TAG_atomic_type => self.type_name_depth(self.ty_of(&entry), depth + 1),
            gimli::DW_TAG_array_type => self.name_of(&entry).unwrap_or_else(|| {
                let elem = self.type_name_depth(self.ty_of(&entry), depth + 1);
                match self.array_shape(&entry) {
                    Some((_, n)) => format!("[{elem}; {n}]"),
                    None => format!("[{elem}]"),
                }
            }),
            gimli::DW_TAG_subroutine_type => self.name_of(&entry).unwrap_or_else(|| "fn".into()),
            _ => self.name_of(&entry).unwrap_or_else(|| "?".into()),
        }
    }

    // ---- values ------------------------------------------------------

    pub(super) fn read(&self, place: &Place, len: u64) -> Option<Vec<u8>> {
        place.read(self.mem, usize::try_from(len).ok()?)
    }

    pub(super) fn read_uint(&self, place: &Place, size: u64) -> Option<u64> {
        if size == 0 || size > 8 {
            return None;
        }
        let bytes = self.read(place, size)?;
        let mut buf = [0u8; 8];
        buf[..bytes.len()].copy_from_slice(&bytes);
        Some(u64::from_le_bytes(buf))
    }
}
