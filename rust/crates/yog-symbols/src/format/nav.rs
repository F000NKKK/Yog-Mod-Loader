//! Navigation over a located value: the rows a lazily expanded view shows
//! one level at a time, and the `*x`, `x.field` and `x[i]` steps of a
//! watch expression. Follows the same Rust-aware layout rules as the eager
//! formatter — std wrappers seen through, enums as their active variant,
//! `Vec`s and slices as their elements, smart pointers dereferenceable.

use super::*;

/// A value somewhere in the stopped target: where its bytes are, its type,
/// the row name a variables view shows for it, and an expression that
/// evaluates back to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) name: String,
    pub(crate) expr: String,
    pub(crate) ty: Option<Off>,
    pub(crate) place: Place,
}

/// What a std wrapper type stands for.
enum Wrapped {
    /// Shown as this value (a `Cell`'s payload, an `Rc`'s pointee...).
    Inner(Slot),
    /// A smart pointer: shown as its pointee (`NonNull`, `Unique`).
    Deref,
    /// Nothing to show (`PhantomData`, an empty `OnceCell`).
    Opaque,
}

/// Elements of an array, slice or `Vec`.
struct Sequence {
    elem: Off,
    base: Place,
    stride: u64,
    len: u64,
}

/// Pointers and wrappers `.field`, `[i]` and integer reads see through.
const MAX_AUTODEREF: usize = 8;
/// Wrappers around the `Option` inside a `OnceCell`.
const MAX_WRAPPER_LAYERS: usize = 4;
const NULL_POINTER: &str = "null pointer";

/// `parent` as the operand of a postfix `.field` / `[i]`.
fn postfix(parent: &str) -> String {
    if parent.starts_with('*') {
        format!("({parent})")
    } else {
        parent.to_string()
    }
}

impl Formatter<'_> {
    /// Display name of `ty`, as the variables view shows it.
    pub(crate) fn display_type(&self, ty: Option<Off>) -> String {
        short_type_name(&self.type_name(ty))
    }

    /// The rows under `slot` when it's expanded — at most `limit` elements
    /// of a sequence (struct fields and pointer targets are always listed).
    pub(crate) fn child_slots(&self, slot: &Slot, limit: u64) -> Vec<Slot> {
        let Some((off, entry)) = slot.ty.and_then(|t| self.peel(t)) else {
            return Vec::new();
        };
        match entry.tag() {
            gimli::DW_TAG_pointer_type
            | gimli::DW_TAG_reference_type
            | gimli::DW_TAG_rvalue_reference_type => self.deref(slot).ok().into_iter().collect(),
            gimli::DW_TAG_structure_type | gimli::DW_TAG_class_type | gimli::DW_TAG_union_type => {
                self.struct_slots(slot, off, &entry, limit)
            }
            gimli::DW_TAG_array_type => match self.sequence(slot) {
                Some(seq) => element_slots(slot, &seq, limit),
                None => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    fn struct_slots(&self, slot: &Slot, off: Off, entry: &Die, limit: u64) -> Vec<Slot> {
        if let Some(vp) = self
            .children(off)
            .into_iter()
            .find(|e| e.tag() == gimli::DW_TAG_variant_part)
        {
            return match self.active_variant(&vp, &slot.place) {
                Some(VariantPick::Found(v)) => match v.member.ty {
                    Some(vty) => self.field_slots(
                        slot,
                        &self.members(vty),
                        &slot.place.offset(v.member.offset),
                    ),
                    None => Vec::new(),
                },
                _ => Vec::new(),
            };
        }
        let short = self.name_of(entry).unwrap_or_default();
        let qualified = self.qualified_name(off, entry);
        if short.ends_with("str") && self.is_fat_pointer(off) || qualified == STRING {
            return Vec::new();
        }
        if let Some(seq) = self.sequence(slot) {
            return element_slots(slot, &seq, limit);
        }
        if generic_base(&qualified) == BOX {
            return self.deref(slot).ok().into_iter().collect();
        }
        match self.wrapper(slot) {
            Some(Wrapped::Inner(inner)) => self.child_slots(&inner, limit),
            Some(Wrapped::Deref) => self.deref(slot).ok().into_iter().collect(),
            Some(Wrapped::Opaque) => Vec::new(),
            None => self.field_slots(slot, &self.members(off), &slot.place),
        }
    }

    fn is_fat_pointer(&self, off: Off) -> bool {
        let members = self.members(off);
        Self::member_named(&members, FAT_DATA).is_some()
            && Self::member_named(&members, FAT_LEN).is_some()
    }

    /// One row per member (tuple fields named `0`, `1`...), phantom
    /// markers left out.
    fn field_slots(&self, parent: &Slot, members: &[Member], base: &Place) -> Vec<Slot> {
        let members: Vec<&Member> = members.iter().filter(|m| !self.is_phantom(m.ty)).collect();
        let tuple = !members.is_empty() && is_tuple(members.iter().copied());
        members
            .into_iter()
            .map(|m| {
                let name = field_label(m, tuple);
                Slot {
                    expr: format!("{}.{name}", postfix(&parent.expr)),
                    name,
                    ty: m.ty,
                    place: base.offset(m.offset),
                }
            })
            .collect()
    }

    /// The elements of an array, slice (`&[T]`, `*const [T]`) or `Vec<T>`.
    fn sequence(&self, slot: &Slot) -> Option<Sequence> {
        let (off, entry) = self.peel(slot.ty?)?;
        match entry.tag() {
            gimli::DW_TAG_array_type => {
                let (elem, len) = self.array_shape(&entry)?;
                let elem = elem?;
                Some(Sequence {
                    elem,
                    base: slot.place.clone(),
                    stride: self.array_stride(&entry, elem)?,
                    len,
                })
            }
            gimli::DW_TAG_structure_type => {
                let short = self.name_of(&entry).unwrap_or_default();
                let members = self.members(off);
                let member = |n: &str| Self::member_named(&members, n);
                if let (Some(data), Some(len)) = (member(FAT_DATA), member(FAT_LEN)) {
                    if short.ends_with("str") || !short.contains('[') {
                        return None;
                    }
                    let ptr = self.read_uint(&slot.place.offset(data.offset), PTR_SIZE)?;
                    let len = self.read_uint(&slot.place.offset(len.offset), PTR_SIZE)?;
                    let elem = self.ty_of(&self.peel(data.ty?)?.1)?;
                    return Some(Sequence {
                        elem,
                        base: Place::Mem(ptr),
                        stride: self.byte_size(elem)?,
                        len,
                    });
                }
                if generic_base(&self.qualified_name(off, &entry)) == VEC {
                    let elem = self.template_param(off, "T")?;
                    let (ptr, len) = self.vec_parts(off, &slot.place)?;
                    return Some(Sequence {
                        elem,
                        base: Place::Mem(ptr),
                        stride: self.byte_size(elem)?,
                        len,
                    });
                }
                None
            }
            _ => None,
        }
    }

    /// What a std wrapper at `slot` stands for; `None` if it isn't one (or
    /// has a layout this doesn't recognize).
    fn wrapper(&self, slot: &Slot) -> Option<Wrapped> {
        let (off, entry) = self.peel(slot.ty?)?;
        if !matches!(
            entry.tag(),
            gimli::DW_TAG_structure_type | gimli::DW_TAG_union_type
        ) {
            return None;
        }
        let qualified = self.qualified_name(off, &entry);
        let base = generic_base(&qualified);
        let members = self.members(off);
        let member = |n: &str| Self::member_named(&members, n);
        // The wrapped value stands in for the wrapper: same row name, same
        // expression (the evaluator sees through wrappers too).
        let inner = |m: &Member, place: Place| Slot {
            name: slot.name.clone(),
            expr: slot.expr.clone(),
            ty: m.ty,
            place,
        };
        match base {
            _ if TRANSPARENT_WRAPPERS.contains(&base) => {
                let m = self.wrapped_member(&members)?;
                Some(Wrapped::Inner(inner(m, slot.place.offset(m.offset))))
            }
            NON_NULL | UNIQUE => Some(Wrapped::Deref),
            PHANTOM_DATA | PHANTOM_PINNED => Some(Wrapped::Opaque),
            ONCE_CELL => {
                let option = member("inner")?;
                let mut option = inner(option, slot.place.offset(option.offset));
                // `UnsafeCell<Option<T>>` in newer std.
                for _ in 0..MAX_WRAPPER_LAYERS {
                    match self.wrapper(&option) {
                        Some(Wrapped::Inner(next)) => option = next,
                        _ => break,
                    }
                }
                let (opt_off, _) = self.peel(option.ty?)?;
                let vp = self
                    .children(opt_off)
                    .into_iter()
                    .find(|e| e.tag() == gimli::DW_TAG_variant_part)?;
                let VariantPick::Found(v) = self.active_variant(&vp, &option.place)? else {
                    return Some(Wrapped::Opaque);
                };
                if v.member.name != SOME {
                    return Some(Wrapped::Opaque);
                }
                let payload = self.members(v.member.ty?).into_iter().next()?;
                let place = option.place.offset(v.member.offset + payload.offset);
                Some(Wrapped::Inner(inner(&payload, place)))
            }
            ONCE_LOCK => {
                let value = member("value")?;
                if self.once_lock_complete(&members, &slot.place) == Some(false) {
                    return Some(Wrapped::Opaque);
                }
                Some(Wrapped::Inner(inner(
                    value,
                    slot.place.offset(value.offset),
                )))
            }
            RC | ARC => {
                let (ptr, inner_ty) = self.find_pointer(off, &slot.place, 0)?;
                if ptr == 0 {
                    return Some(Wrapped::Opaque);
                }
                let (inner_off, _) = self.peel(inner_ty?)?;
                let fields = self.members(inner_off);
                let data = Self::rc_payload(&fields)?;
                Some(Wrapped::Inner(inner(data, Place::Mem(ptr + data.offset))))
            }
            _ => None,
        }
    }

    /// `*slot`: the target of a pointer, reference, `Box`, `NonNull`,
    /// `Rc`/`Arc` or `Pin`.
    pub(crate) fn deref(&self, slot: &Slot) -> Result<Slot, String> {
        let cannot = || {
            format!(
                "type `{}` cannot be dereferenced",
                self.display_type(slot.ty)
            )
        };
        let (off, entry) = slot.ty.and_then(|t| self.peel(t)).ok_or_else(cannot)?;
        let (ptr, pointee) = match entry.tag() {
            gimli::DW_TAG_pointer_type
            | gimli::DW_TAG_reference_type
            | gimli::DW_TAG_rvalue_reference_type => {
                let ptr = self
                    .read_uint(&slot.place, PTR_SIZE)
                    .ok_or_else(|| format!("can't read `{}`", slot.expr))?;
                (ptr, self.ty_of(&entry))
            }
            gimli::DW_TAG_structure_type => {
                let qualified = self.qualified_name(off, &entry);
                match generic_base(&qualified) {
                    BOX | NON_NULL | UNIQUE => self
                        .find_pointer(off, &slot.place, 0)
                        .ok_or_else(|| format!("can't read `{}`", slot.expr))?,
                    PIN => match self.wrapper(slot) {
                        Some(Wrapped::Inner(inner)) => return self.deref(&inner),
                        _ => return Err(cannot()),
                    },
                    RC | ARC => {
                        return match self.wrapper(slot) {
                            Some(Wrapped::Inner(inner)) => Ok(Slot {
                                name: format!("*{}", slot.name),
                                expr: format!("*{}", slot.expr),
                                ..inner
                            }),
                            _ => Err(NULL_POINTER.into()),
                        };
                    }
                    _ => return Err(cannot()),
                }
            }
            _ => return Err(cannot()),
        };
        if ptr == 0 {
            return Err(NULL_POINTER.into());
        }
        let pointee = pointee.ok_or_else(cannot)?;
        if !self.is_derefable(pointee) {
            return Err(cannot());
        }
        Ok(Slot {
            name: format!("*{}", slot.name),
            expr: format!("*{}", slot.expr),
            ty: Some(pointee),
            place: Place::Mem(ptr),
        })
    }

    /// `slot.name`: a field (or tuple field `0`, `1`...) of a struct,
    /// tuple or the active enum variant, auto-dereferencing pointers and
    /// seeing through std wrappers as Rust's `.` does.
    pub(crate) fn field(&self, slot: &Slot, name: &str) -> Result<Slot, String> {
        let expr = format!("{}.{name}", postfix(&slot.expr));
        let mut cur = slot.clone();
        for _ in 0..MAX_AUTODEREF {
            if let Some(found) = self
                .child_slots(&cur, 0)
                .into_iter()
                .find(|s| s.name == name)
            {
                return Ok(Slot { expr, ..found });
            }
            // The raw layout too (`String`'s `vec`, a `RefCell`'s `borrow`).
            if let Some((off, entry)) = cur.ty.and_then(|t| self.peel(t))
                && matches!(
                    entry.tag(),
                    gimli::DW_TAG_structure_type
                        | gimli::DW_TAG_class_type
                        | gimli::DW_TAG_union_type
                )
            {
                let tuple_name = format!("{TUPLE_FIELD}{name}");
                if let Some(m) = self
                    .members(off)
                    .into_iter()
                    .find(|m| m.name == name || m.name == tuple_name)
                {
                    return Ok(Slot {
                        name: name.to_string(),
                        expr,
                        ty: m.ty,
                        place: cur.place.offset(m.offset),
                    });
                }
            }
            match self.deref(&cur) {
                Ok(next) => cur = next,
                Err(_) => break,
            }
        }
        Err(format!(
            "no field `{name}` on type `{}`",
            self.display_type(slot.ty)
        ))
    }

    /// `slot[index]` into an array, slice or `Vec` (through references,
    /// smart pointers and std wrappers).
    pub(crate) fn index(&self, slot: &Slot, index: i128) -> Result<Slot, String> {
        let mut cur = slot.clone();
        for _ in 0..MAX_AUTODEREF {
            if let Some(seq) = self.sequence(&cur) {
                let Ok(i) = u64::try_from(index) else {
                    return Err(format!("negative index {index}"));
                };
                if i >= seq.len {
                    return Err(format!(
                        "index out of bounds: the len is {} but the index is {i}",
                        seq.len
                    ));
                }
                return Ok(Slot {
                    name: format!("[{i}]"),
                    expr: format!("{}[{i}]", postfix(&slot.expr)),
                    ty: Some(seq.elem),
                    place: seq.base.offset(i.wrapping_mul(seq.stride)),
                });
            }
            cur = match self.wrapper(&cur) {
                Some(Wrapped::Inner(inner)) => inner,
                _ => match self.deref(&cur) {
                    Ok(next) => next,
                    Err(_) => break,
                },
            };
        }
        Err(format!(
            "cannot index into a value of type `{}`",
            self.display_type(slot.ty)
        ))
    }

    /// The value of an integer (or `bool`/`char`) at `slot`, for use as an
    /// index — through references and wrappers like `Cell<usize>`.
    pub(crate) fn integer(&self, slot: &Slot) -> Result<i128, String> {
        let mut cur = slot.clone();
        for _ in 0..MAX_AUTODEREF {
            let Some((_, entry)) = cur.ty.and_then(|t| self.peel(t)) else {
                break;
            };
            if entry.tag() == gimli::DW_TAG_base_type {
                let size = Self::udata(&entry, gimli::DW_AT_byte_size).unwrap_or(0);
                let encoding = match entry.attr_value(gimli::DW_AT_encoding) {
                    Some(AttributeValue::Encoding(e)) => e,
                    _ => break,
                };
                if size == 0 || size > 8 || encoding == gimli::DW_ATE_float {
                    break;
                }
                let raw = self
                    .read_uint(&cur.place, size)
                    .ok_or_else(|| format!("can't read `{}`", slot.expr))?;
                let signed = matches!(encoding, gimli::DW_ATE_signed | gimli::DW_ATE_signed_char);
                return Ok(if signed {
                    let shift = 64 - 8 * size as u32;
                    (((raw << shift) as i64) >> shift) as i128
                } else {
                    raw as i128
                });
            }
            cur = match self.wrapper(&cur) {
                Some(Wrapped::Inner(inner)) => inner,
                _ => match self.deref(&cur) {
                    Ok(next) => next,
                    Err(_) => break,
                },
            };
        }
        Err(format!(
            "`{}` is not an integer (type `{}`)",
            slot.expr,
            self.display_type(slot.ty)
        ))
    }
}

fn element_slots(parent: &Slot, seq: &Sequence, limit: u64) -> Vec<Slot> {
    let base = postfix(&parent.expr);
    (0..seq.len.min(limit))
        .map(|i| Slot {
            name: format!("[{i}]"),
            expr: format!("{base}[{i}]"),
            ty: Some(seq.elem),
            place: seq.base.offset(i.wrapping_mul(seq.stride)),
        })
        .collect()
}
