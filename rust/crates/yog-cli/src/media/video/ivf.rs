//! IVF, the container ffmpeg writes AV1 packets in: a 32-byte header
//! (`DKIF`, the picture's size, the time base), then each frame's 12-byte
//! header (its packet's size, its time) and packet.

/// What an IVF file starts with.
const SIGNATURE: &[u8] = b"DKIF";
/// The file's header and each frame's, bytes.
const HEADER: usize = 32;
const FRAME_HEADER: usize = 12;
/// Where the header keeps the picture's width and height (`u16`s), and
/// the frame rate as `rate / scale` (`u32`s).
const WIDTH_AT: usize = 12;
const HEIGHT_AT: usize = 14;
const RATE_AT: usize = 16;
const SCALE_AT: usize = 20;

/// An IVF stream, its packets borrowed.
#[derive(Debug, PartialEq)]
pub(super) struct Ivf<'a> {
    pub(super) width: u32,
    pub(super) height: u32,
    /// Frames a second, `rate / scale`.
    pub(super) rate: u32,
    pub(super) scale: u32,
    pub(super) packets: Vec<&'a [u8]>,
}

fn u16_at(b: &[u8], at: usize) -> u32 {
    u32::from(u16::from_le_bytes([b[at], b[at + 1]]))
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// `bytes` read as IVF.
pub(super) fn parse(bytes: &[u8]) -> Result<Ivf<'_>, String> {
    if bytes.len() < HEADER || !bytes.starts_with(SIGNATURE) {
        return Err("ffmpeg's output isn't IVF".into());
    }
    let mut packets = Vec::new();
    let mut at = HEADER;
    while at + FRAME_HEADER <= bytes.len() {
        let size = u32_at(bytes, at) as usize;
        let start = at + FRAME_HEADER;
        let packet = bytes
            .get(start..start + size)
            .ok_or("ffmpeg's IVF output ends inside a frame")?;
        packets.push(packet);
        at = start + size;
    }
    Ok(Ivf {
        width: u16_at(bytes, WIDTH_AT),
        height: u16_at(bytes, HEIGHT_AT),
        rate: u32_at(bytes, RATE_AT),
        scale: u32_at(bytes, SCALE_AT),
        packets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An IVF file of a 4×2 picture at 20/1 frames a second holding `packets`.
    pub(crate) fn ivf(packets: &[&[u8]]) -> Vec<u8> {
        let mut b = vec![0u8; HEADER];
        b[..SIGNATURE.len()].copy_from_slice(SIGNATURE);
        b[WIDTH_AT..WIDTH_AT + 2].copy_from_slice(&4u16.to_le_bytes());
        b[HEIGHT_AT..HEIGHT_AT + 2].copy_from_slice(&2u16.to_le_bytes());
        b[RATE_AT..RATE_AT + 4].copy_from_slice(&20u32.to_le_bytes());
        b[SCALE_AT..SCALE_AT + 4].copy_from_slice(&1u32.to_le_bytes());
        for p in packets {
            b.extend((p.len() as u32).to_le_bytes());
            b.extend([0u8; FRAME_HEADER - 4]);
            b.extend(*p);
        }
        b
    }

    #[test]
    fn packets_come_out_in_order() {
        let bytes = ivf(&[&[1, 2], &[3]]);
        let v = parse(&bytes).unwrap();
        assert_eq!((v.width, v.height, v.rate, v.scale), (4, 2, 20, 1));
        assert_eq!(v.packets, vec![&[1u8, 2][..], &[3][..]]);
    }

    #[test]
    fn a_cut_file_is_an_error() {
        let mut bytes = ivf(&[&[1, 2, 3]]);
        bytes.pop();
        assert!(parse(&bytes).is_err());
        assert!(parse(b"RIFF").is_err());
    }
}
