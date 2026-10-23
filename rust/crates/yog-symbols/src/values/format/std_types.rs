//! Formatting the standard library's own types: wrappers (Box, Rc, Pin…), Vec and String.

use super::{
    ARC, Formatter, MAX_TYPE_DEPTH, Member, NON_NULL, NULL, ONCE_CELL, ONCE_LOCK, Off,
    PHANTOM_DATA, PHANTOM_PINNED, RC, TRANSPARENT_WRAPPERS, UNINIT, UNIQUE, generic_base,
    unwrap_option,
};
use crate::values::locals::{Place, Value};

impl<'a> Formatter<'a> {
    /// std wrappers shown as what they wrap; `None` to fall back to the
    /// plain field view (unknown type, or a layout this doesn't recognize).
    pub(super) fn format_wrapper(
        &self,
        off: Off,
        qualified: &str,
        members: &[Member],
        place: &Place,
        depth: usize,
    ) -> Option<(String, Vec<Value>)> {
        let base = generic_base(qualified);
        let member = |n: &str| Self::member_named(members, n);
        match base {
            _ if TRANSPARENT_WRAPPERS.contains(&base) => {
                Some(self.unwrap_member(self.wrapped_member(members)?, place, depth))
            }
            NON_NULL | UNIQUE => {
                let (ptr, pointee) = self.find_pointer(off, place, 0)?;
                Some(self.format_pointer("", ptr, pointee, depth))
            }
            PHANTOM_DATA | PHANTOM_PINNED => {
                Some((base.rsplit("::").next()?.to_string(), Vec::new()))
            }
            ONCE_CELL => {
                let (value, children) = self.unwrap_member(member("inner")?, place, depth);
                Some(unwrap_option(value, children))
            }
            ONCE_LOCK => {
                let value = member("value")?;
                match self.once_lock_complete(members, place) {
                    Some(false) => Some((UNINIT.into(), Vec::new())),
                    // Initialized, or a state this can't read: show the slot.
                    _ => Some(self.unwrap_member(value, place, depth)),
                }
            }
            RC | ARC => {
                let (ptr, inner_ty) = self.find_pointer(off, place, 0)?;
                if ptr == 0 {
                    return Some((NULL.into(), Vec::new()));
                }
                let (inner_off, _) = self.peel(inner_ty?)?;
                let inner = self.members(inner_off);
                let strong = Self::member_named(&inner, "strong")?;
                let data = Self::rc_payload(&inner)?;
                let count = self.first_uint(strong.ty?, &Place::Mem(ptr + strong.offset), 0);
                let count = count.map_or_else(|| "?".into(), |n| n.to_string());
                let (value, children) = self.unwrap_member(data, &Place::Mem(ptr), depth);
                Some((format!("{value} (strong={count})"), children))
            }
            _ => None,
        }
    }

    /// `member` of the struct at `place`, formatted at the struct's own
    /// depth (the wrapper doesn't count as a nesting level).
    pub(super) fn unwrap_member(
        &self,
        member: &Member,
        place: &Place,
        depth: usize,
    ) -> (String, Vec<Value>) {
        let inner = self.format(
            member.name.clone(),
            member.ty,
            &place.offset(member.offset),
            depth,
        );
        (inner.value, inner.children)
    }

    /// The first integer reachable through leading struct members (an
    /// atomic's or `Cell`'s payload).
    pub(super) fn first_uint(&self, ty: Off, place: &Place, depth: usize) -> Option<u64> {
        if depth > MAX_TYPE_DEPTH {
            return None;
        }
        let (off, entry) = self.peel(ty)?;
        match entry.tag() {
            gimli::DW_TAG_base_type => {
                self.read_uint(place, Self::udata(&entry, gimli::DW_AT_byte_size)?)
            }
            gimli::DW_TAG_structure_type | gimli::DW_TAG_union_type => {
                let first = self.members(off).into_iter().next()?;
                self.first_uint(first.ty?, &place.offset(first.offset), depth + 1)
            }
            _ => None,
        }
    }
}
