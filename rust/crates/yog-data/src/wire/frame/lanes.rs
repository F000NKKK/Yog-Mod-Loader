//! Numbers bit-packed in lanes: what a frame's list of ints is made of.
//!
//! A list of `n` numbers `width` bits each (1..=32) is `n / BLOCK` blocks,
//! then the rest packed one after another (little-endian bits). A block is
//! [`BLOCK`] numbers in [`LANES`] lanes of 32-bit words: number `j` goes to
//! lane `j % LANES`, the `j / LANES`-th in it; a lane's numbers are packed
//! one after another into its words, and word `k` of lane `l` is the block's
//! `k * LANES + l`-th. A row of `LANES` numbers is shifted and merged as one
//! 256-bit vector, so packing and unpacking run a block at a time — AVX2
//! where the processor has it, chosen once at start; the same bytes either
//! way. Numbers wider than 32 bits go one after another, no blocks.
//!
//! Number `i` is found in O(1) ([`get`]), so views read in place.

use yog_sync::OnceLock;

/// Lanes of a block: a 256-bit vector of 32-bit words.
pub const LANES: usize = 8;
/// Numbers a lane holds in a block: as many as its word has bits.
const ROWS: usize = u32::BITS as usize;
/// Numbers in a block.
pub const BLOCK: usize = LANES * ROWS;
/// The widest numbers packed in blocks.
pub const BLOCK_MAX_BITS: u32 = u32::BITS;
const WORD_BYTES: usize = 4;
const BYTE_BITS: usize = 8;

/// A block's bytes, its numbers `width` bits each.
pub const fn block_bytes(width: u32) -> usize {
    LANES * width as usize * WORD_BYTES
}

/// The bytes `n` numbers `width` bits each take.
pub fn packed_bytes(n: usize, width: u32) -> Option<usize> {
    let (blocks, rest) = split(n, width);
    blocks
        .checked_mul(block_bytes(width))?
        .checked_add(rest.checked_mul(width as usize)?.div_ceil(BYTE_BITS))
}

/// How many whole blocks `n` numbers make, and how many go after them.
fn split(n: usize, width: u32) -> (usize, usize) {
    if width > BLOCK_MAX_BITS {
        (0, n)
    } else {
        (n / BLOCK, n % BLOCK)
    }
}

/// One block packed: `src`'s numbers (each fitting `width` bits) onto
/// `out` ([`block_bytes`] long).
type PackFn = fn(&[u32; BLOCK], u32, &mut [u8]);
/// One block unpacked.
type UnpackFn = fn(&[u8], u32, &mut [u32; BLOCK]);

/// This processor's kernels: AVX2 if it has it, chosen once.
fn kernels() -> &'static (PackFn, UnpackFn) {
    static KERNELS: OnceLock<(PackFn, UnpackFn)> = OnceLock::new();
    KERNELS.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return (avx2::pack, avx2::unpack);
        }
        (portable::pack, portable::unpack)
    })
}

/// A word's mask of `width` low bits.
fn mask(width: u32) -> u32 {
    u32::MAX >> (u32::BITS - width)
}

/// The kernels any processor runs: rows as `[u32; LANES]`.
mod portable;

/// The same, a row a 256-bit vector.
#[cfg(target_arch = "x86_64")]
mod avx2;

/// Packs `items` (`n` of them, each fitting `width` bits) onto `out`.
pub fn pack(out: &mut Vec<u8>, n: usize, width: u32, items: impl Iterator<Item = u64>) {
    let (blocks, rest) = split(n, width);
    let start = out.len();
    out.resize(start + packed_bytes(n, width).unwrap_or(0), 0);
    let mut items = items;
    let (pack_block, _) = kernels();
    let mut block = [0u32; BLOCK];
    let mut at = start;
    for _ in 0..blocks {
        for (slot, v) in block.iter_mut().zip(items.by_ref()) {
            *slot = v as u32;
        }
        let bytes = block_bytes(width);
        pack_block(&block, width, &mut out[at..at + bytes]);
        at += bytes;
    }
    pack_sequential(&mut out[at..], rest, width, items);
}

/// Numbers one after another, little-endian bits.
fn pack_sequential(out: &mut [u8], n: usize, width: u32, items: impl Iterator<Item = u64>) {
    let mut acc: u128 = 0;
    let mut have = 0u32;
    let mut at = 0usize;
    for v in items.take(n) {
        acc |= u128::from(v) << have;
        have += width;
        while have >= u8::BITS {
            out[at] = acc as u8;
            at += 1;
            acc >>= u8::BITS;
            have -= u8::BITS;
        }
    }
    if have > 0 {
        out[at] = acc as u8;
    }
}

/// Each of the `n` numbers in `bytes` (`width` bits each), in order, to `f`.
pub fn unpack(bytes: &[u8], n: usize, width: u32, mut f: impl FnMut(u64)) {
    let (blocks, rest) = split(n, width);
    let (_, unpack_block) = kernels();
    let mut block = [0u32; BLOCK];
    let size = block_bytes(width);
    for b in 0..blocks {
        unpack_block(&bytes[b * size..(b + 1) * size], width, &mut block);
        block.iter().for_each(|&v| f(u64::from(v)));
    }
    let tail = &bytes[blocks * size..];
    for i in 0..rest {
        f(sequential(tail, i, width));
    }
}

/// Number `i` of `bytes` (`n` numbers, `width` bits each) — `i < n`.
pub fn get(bytes: &[u8], n: usize, width: u32, i: usize) -> u64 {
    let (blocks, _) = split(n, width);
    if i >= blocks * BLOCK {
        let tail = &bytes[blocks * block_bytes(width)..];
        return sequential(tail, i - blocks * BLOCK, width);
    }
    let (block, j) = (i / BLOCK, i % BLOCK);
    let (lane, row) = (j % LANES, j / LANES);
    let bit = row * width as usize;
    let (word, shift) = (bit / u32::BITS as usize, bit % u32::BITS as usize);
    let at = block * block_bytes(width);
    let word_at = |k: usize| {
        let o = at + (k * LANES + lane) * WORD_BYTES;
        bytes.get(o..o + WORD_BYTES).map_or(0, |b| {
            u64::from(u32::from_le_bytes(b.try_into().unwrap_or_default()))
        })
    };
    let two = word_at(word)
        | if shift + width as usize > u32::BITS as usize {
            word_at(word + 1) << u32::BITS
        } else {
            0
        };
    (two >> shift) & (u64::MAX >> (u64::BITS - width))
}

/// Number `i` of numbers packed one after another.
fn sequential(bytes: &[u8], i: usize, width: u32) -> u64 {
    // A number spans at most 9 bytes (64 bits starting anywhere in a byte).
    const SPAN: usize = 9;
    let bit = i * width as usize;
    let (byte, shift) = (bit / BYTE_BITS, bit % BYTE_BITS);
    let mut window = [0u8; 16];
    let end = (byte + SPAN).min(bytes.len());
    if byte < end {
        window[..end - byte].copy_from_slice(&bytes[byte..end]);
    }
    let mask = u128::from(u64::MAX) >> (u64::BITS - width);
    ((u128::from_le_bytes(window) >> shift) & mask) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(n: usize, width: u32) {
        let max = u64::MAX >> (u64::BITS - width);
        let items: Vec<u64> = (0..n as u64)
            .map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15) & max)
            .collect();
        let mut out = Vec::new();
        pack(&mut out, n, width, items.iter().copied());
        assert_eq!(Some(out.len()), packed_bytes(n, width));
        let mut back = Vec::new();
        unpack(&out, n, width, |v| back.push(v));
        assert_eq!(back, items, "n {n}, width {width}");
        for (i, v) in items.iter().enumerate() {
            assert_eq!(
                get(&out, n, width, i),
                *v,
                "get {i} of n {n}, width {width}"
            );
        }
    }

    #[test]
    fn every_width_round_trips_in_blocks_and_tail() {
        for width in 1..=u64::BITS {
            for n in [0, 1, 7, BLOCK - 1, BLOCK, BLOCK + 3, 3 * BLOCK + 100] {
                roundtrip(n, width);
            }
        }
    }

    #[test]
    fn the_kernels_agree() {
        let (pack_block, unpack_block) = *kernels();
        let src: [u32; BLOCK] = std::array::from_fn(|i| (i as u32).wrapping_mul(2_654_435_761));
        for width in 1..=BLOCK_MAX_BITS {
            let src = src.map(|v| v & mask(width));
            let (mut a, mut b) = (vec![0; block_bytes(width)], vec![0; block_bytes(width)]);
            portable::pack(&src, width, &mut a);
            pack_block(&src, width, &mut b);
            assert_eq!(a, b, "width {width}");
            let (mut x, mut y) = ([0; BLOCK], [0; BLOCK]);
            portable::unpack(&a, width, &mut x);
            unpack_block(&a, width, &mut y);
            assert_eq!((x, y), (src, src), "width {width}");
        }
    }
}
