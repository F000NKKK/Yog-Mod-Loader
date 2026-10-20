//! Where a variable is: its DWARF attributes followed to their origin, its location expression evaluated in a frame (frame base, CFA, pieces).

use super::{
    FP_TO_CFA, Frame, Located, MAX_EXPR_STEPS, MAX_ORIGIN_CHAIN, MAX_PIECE, Place, UnitInfo,
    WORD_SIZE, unit_ref,
};
use crate::{R, SymbolTable};
use gimli::{
    AttributeValue, DebuggingInformationEntry, EvaluationResult, Location, Reader as _, UnitOffset,
};
use std::sync::OnceLock;

impl SymbolTable {
    /// `entry`'s attribute, falling back along `DW_AT_abstract_origin` /
    /// `DW_AT_specification` (inlined and out-of-line instances keep their
    /// name and type on the abstract DIE).
    pub(crate) fn attr_with_origin(
        &self,
        info: &UnitInfo,
        entry: &DebuggingInformationEntry<R>,
        name: gimli::DwAt,
    ) -> Option<AttributeValue<R>> {
        if let Some(v) = entry.attr_value(name) {
            return Some(v);
        }
        let mut current = entry.clone();
        for _ in 0..MAX_ORIGIN_CHAIN {
            let origin = current
                .attr_value(gimli::DW_AT_abstract_origin)
                .or_else(|| current.attr_value(gimli::DW_AT_specification))?;
            let offset = unit_ref(info, origin)?;
            current = info.unit.entry(offset).ok()?;
            if let Some(v) = current.attr_value(name) {
                return Some(v);
            }
        }
        None
    }

    pub(crate) fn locate(
        &self,
        info: &UnitInfo,
        entry: &DebuggingInformationEntry<R>,
        subprogram: UnitOffset<usize>,
        frame: &Frame<'_>,
    ) -> Located {
        let expr = match entry.attr_value(gimli::DW_AT_location) {
            Some(AttributeValue::Exprloc(expr)) => expr,
            Some(other) => {
                let Ok(Some(mut list)) = self.dwarf.attr_locations(&info.unit, other) else {
                    return Located::Unavailable;
                };
                let mut found = None;
                loop {
                    match list.next() {
                        Ok(Some(e)) if e.range.begin <= frame.pc && frame.pc < e.range.end => {
                            found = Some(e.data);
                            break;
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(_) => return Located::Unavailable,
                    }
                }
                match found {
                    Some(expr) => expr,
                    None => return Located::OptimizedOut,
                }
            }
            None => {
                return match self.attr_with_origin(info, entry, gimli::DW_AT_const_value) {
                    Some(v) => match const_bytes(v) {
                        Some(bytes) => Located::Place(Place::Bytes(bytes.into(), 0)),
                        None => Located::Unavailable,
                    },
                    None => Located::OptimizedOut,
                };
            }
        };

        let frame_base = OnceLock::new();
        let get_frame_base =
            || *frame_base.get_or_init(|| self.frame_base(info, subprogram, frame));
        let Some(pieces) = self.evaluate_location(info, expr, frame, &get_frame_base) else {
            return Located::Unavailable;
        };
        assemble(pieces, frame)
    }

    pub(super) fn frame_base(
        &self,
        info: &UnitInfo,
        subprogram: UnitOffset<usize>,
        frame: &Frame<'_>,
    ) -> Option<u64> {
        let entry = info.unit.entry(subprogram).ok()?;
        let Some(AttributeValue::Exprloc(expr)) = entry.attr_value(gimli::DW_AT_frame_base) else {
            return None;
        };
        let pieces = self.evaluate_location(info, expr, frame, &|| None)?;
        match pieces.first()?.location {
            Location::Register { register } => frame.regs.get(register.0),
            Location::Address { address } => Some(address),
            Location::Value { value } => value.to_u64(!0).ok(),
            _ => None,
        }
    }

    /// The canonical frame address at `frame.pc`, from `.eh_frame` (or
    /// `.debug_frame`) unwind rows; falls back to `rbp + 16` for frames
    /// with a conventional frame pointer.
    pub(super) fn cfa(&self, frame: &Frame<'_>) -> Option<u64> {
        self.with_unwind_row(frame.pc, |row| match row.cfa() {
            gimli::CfaRule::RegisterAndOffset { register, offset } => frame
                .regs
                .get(register.0)
                .map(|v| v.wrapping_add_signed(*offset)),
            gimli::CfaRule::Expression(_) => None,
        })
        .or_else(|| frame.regs.fp().map(|fp| fp.wrapping_add(FP_TO_CFA)))
    }

    pub(super) fn evaluate_location(
        &self,
        info: &UnitInfo,
        expr: gimli::Expression<R>,
        frame: &Frame<'_>,
        frame_base: &dyn Fn() -> Option<u64>,
    ) -> Option<Vec<gimli::Piece<R>>> {
        let mut eval = expr.evaluation(info.unit.encoding());
        eval.set_max_iterations(MAX_EXPR_STEPS);
        let mut state = eval.evaluate().ok()?;
        loop {
            state = match state {
                EvaluationResult::Complete => break,
                EvaluationResult::RequiresMemory { address, size, .. } => {
                    let size = (size as usize).min(WORD_SIZE);
                    let mut buf = [0u8; WORD_SIZE];
                    if !frame.mem.read(address, &mut buf[..size]) {
                        return None;
                    }
                    eval.resume_with_memory(gimli::Value::Generic(u64::from_le_bytes(buf)))
                }
                EvaluationResult::RequiresRegister { register, .. } => {
                    let value = frame.regs.get(register.0)?;
                    eval.resume_with_register(gimli::Value::Generic(value))
                }
                EvaluationResult::RequiresFrameBase => eval.resume_with_frame_base(frame_base()?),
                EvaluationResult::RequiresCallFrameCfa => {
                    eval.resume_with_call_frame_cfa(self.cfa(frame)?)
                }
                EvaluationResult::RequiresRelocatedAddress(addr) => {
                    eval.resume_with_relocated_address(addr.wrapping_add(frame.load_base))
                }
                EvaluationResult::RequiresIndexedAddress { index, relocate } => {
                    let addr = self.dwarf.address(&info.unit, index).ok()?;
                    let addr = if relocate {
                        addr.wrapping_add(frame.load_base)
                    } else {
                        addr
                    };
                    eval.resume_with_indexed_address(addr)
                }
                EvaluationResult::RequiresBaseType(offset) => {
                    let entry = info.unit.entry(offset).ok()?;
                    let encoding = match entry.attr_value(gimli::DW_AT_encoding)? {
                        AttributeValue::Encoding(e) => e,
                        _ => return None,
                    };
                    let size = entry.attr_value(gimli::DW_AT_byte_size)?.udata_value()?;
                    let ty = gimli::ValueType::from_encoding(encoding, size)?;
                    eval.resume_with_base_type(ty)
                }
                // TLS, entry values, DW_OP_call*, parameter refs, wasm:
                // nothing we can answer from a single stopped frame.
                _ => return None,
            }
            .ok()?;
        }
        Some(eval.result())
    }
}

pub(super) fn assemble(pieces: Vec<gimli::Piece<R>>, frame: &Frame<'_>) -> Located {
    if pieces.is_empty() {
        return Located::OptimizedOut;
    }
    if pieces.len() == 1 && pieces[0].size_in_bits.is_none() {
        return match location_bytes(&pieces[0].location, frame, None) {
            PieceData::Mem(addr) => Located::Place(Place::Mem(addr)),
            PieceData::Bytes(b) => Located::Place(Place::Bytes(b.into(), 0)),
            PieceData::Empty => Located::OptimizedOut,
            PieceData::Unavailable => Located::Unavailable,
        };
    }
    let mut out = Vec::new();
    let mut any = false;
    for piece in &pieces {
        let Some(bits) = piece.size_in_bits else {
            return Located::Unavailable;
        };
        let len = bits.div_ceil(8) as usize;
        if len > MAX_PIECE {
            return Located::Unavailable;
        }
        let bytes = match location_bytes(&piece.location, frame, Some(len)) {
            PieceData::Mem(addr) => {
                let mut buf = vec![0u8; len];
                if !frame.mem.read(addr, &mut buf) {
                    return Located::Unavailable;
                }
                any = true;
                buf
            }
            PieceData::Bytes(mut b) => {
                let shift = piece.bit_offset.unwrap_or(0) / 8;
                b.drain(..(shift as usize).min(b.len()));
                b.resize(len, 0);
                any = true;
                b
            }
            PieceData::Empty => vec![0; len],
            PieceData::Unavailable => return Located::Unavailable,
        };
        out.extend_from_slice(&bytes[..len.min(bytes.len())]);
    }
    if any {
        Located::Place(Place::Bytes(out.into(), 0))
    } else {
        Located::OptimizedOut
    }
}

pub(super) enum PieceData {
    Mem(u64),
    Bytes(Vec<u8>),
    Empty,
    Unavailable,
}

pub(super) fn location_bytes(
    loc: &Location<R>,
    frame: &Frame<'_>,
    len: Option<usize>,
) -> PieceData {
    match loc {
        Location::Empty => PieceData::Empty,
        Location::Address { address } => PieceData::Mem(*address),
        Location::Register { register } => match frame.regs.get(register.0) {
            Some(v) => PieceData::Bytes(v.to_le_bytes().to_vec()),
            None => PieceData::Unavailable,
        },
        Location::Value { value } => PieceData::Bytes(value_bytes(*value)),
        Location::Bytes { value } => {
            let mut bytes = value.to_slice().map(|s| s.into_owned()).unwrap_or_default();
            if let Some(len) = len {
                bytes.resize(len, 0);
            }
            PieceData::Bytes(bytes)
        }
        Location::ImplicitPointer { .. } => PieceData::Unavailable,
    }
}

pub(super) fn value_bytes(value: gimli::Value) -> Vec<u8> {
    use gimli::Value as V;
    match value {
        V::Generic(v) | V::U64(v) => v.to_le_bytes().to_vec(),
        V::I8(v) => v.to_le_bytes().to_vec(),
        V::U8(v) => v.to_le_bytes().to_vec(),
        V::I16(v) => v.to_le_bytes().to_vec(),
        V::U16(v) => v.to_le_bytes().to_vec(),
        V::I32(v) => v.to_le_bytes().to_vec(),
        V::U32(v) => v.to_le_bytes().to_vec(),
        V::I64(v) => v.to_le_bytes().to_vec(),
        V::F32(v) => v.to_le_bytes().to_vec(),
        V::F64(v) => v.to_le_bytes().to_vec(),
    }
}

pub(super) fn const_bytes(v: AttributeValue<R>) -> Option<Vec<u8>> {
    Some(match v {
        AttributeValue::Block(r) => r.to_slice().ok()?.into_owned(),
        AttributeValue::Data1(x) => vec![x],
        AttributeValue::Data2(x) => x.to_le_bytes().to_vec(),
        AttributeValue::Data4(x) => x.to_le_bytes().to_vec(),
        AttributeValue::Data8(x) => x.to_le_bytes().to_vec(),
        AttributeValue::Sdata(x) => x.to_le_bytes().to_vec(),
        AttributeValue::Udata(x) => x.to_le_bytes().to_vec(),
        _ => return None,
    })
}
