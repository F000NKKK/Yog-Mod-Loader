//! Enough of AV1's bitstream to tell a keyframe: a packet's OBUs walked to
//! its frame header, whose first bits are `show_existing_frame` and
//! `frame_type` (as encoders write it, without the reduced still-picture
//! header).

/// OBU types that start with a frame header.
const OBU_FRAME_HEADER: u8 = 3;
const OBU_FRAME: u8 = 6;
/// An OBU header's fields: its type (4 bits after the forbidden one),
/// whether an extension byte follows, whether its size is written.
const TYPE_SHIFT: u8 = 3;
const TYPE_MASK: u8 = 0xF;
const HAS_EXTENSION: u8 = 0b100;
const HAS_SIZE: u8 = 0b10;
/// A frame header's first byte: `show_existing_frame`, then `frame_type`.
const SHOW_EXISTING: u8 = 0x80;
const FRAME_TYPE_SHIFT: u8 = 5;
const FRAME_TYPE_MASK: u8 = 0b11;
const KEY_FRAME: u8 = 0;
/// LEB128: 7 bits a byte, the top one saying more follow.
const LEB_BITS: u32 = 7;
const LEB_MORE: u8 = 0x80;
const LEB_VALUE: u8 = 0x7F;
const LEB_MAX_BYTES: usize = 8;

/// A LEB128 number at the start of `b`: its value and its length.
fn leb128(b: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    for (i, &byte) in b.iter().take(LEB_MAX_BYTES).enumerate() {
        value |= usize::from(byte & LEB_VALUE) << (LEB_BITS * i as u32);
        if byte & LEB_MORE == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

/// Whether `packet` (a temporal unit) holds a keyframe — a decoder can start at it.
pub(super) fn is_keyframe(packet: &[u8]) -> bool {
    let mut at = 0;
    while let Some(&header) = packet.get(at) {
        let kind = (header >> TYPE_SHIFT) & TYPE_MASK;
        at += 1 + usize::from(header & HAS_EXTENSION != 0);
        let size = if header & HAS_SIZE != 0 {
            let Some((size, len)) = packet.get(at..).and_then(leb128) else {
                return false;
            };
            at += len;
            size
        } else {
            packet.len().saturating_sub(at)
        };
        if kind == OBU_FRAME_HEADER || kind == OBU_FRAME {
            return packet.get(at).is_some_and(|&b| {
                b & SHOW_EXISTING == 0 && (b >> FRAME_TYPE_SHIFT) & FRAME_TYPE_MASK == KEY_FRAME
            });
        }
        at += size;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporal delimiter, then a frame OBU whose header starts with `first`.
    fn packet(first: u8) -> Vec<u8> {
        let delimiter = [2 << TYPE_SHIFT | HAS_SIZE, 0];
        let frame = [OBU_FRAME << TYPE_SHIFT | HAS_SIZE, 2, first, 0];
        [&delimiter[..], &frame[..]].concat()
    }

    #[test]
    fn a_key_frame_is_told_from_the_others() {
        assert!(is_keyframe(&packet(0b0001_0000)));
        // An inter frame, and a key frame shown again.
        assert!(!is_keyframe(&packet(1 << FRAME_TYPE_SHIFT)));
        assert!(!is_keyframe(&packet(SHOW_EXISTING)));
        assert!(!is_keyframe(&[]));
    }

    #[test]
    fn sizes_are_leb128() {
        assert_eq!(leb128(&[0x80 | 5, 1]), Some((5 + 128, 2)));
        assert_eq!(leb128(&[0x80]), None);
    }
}
