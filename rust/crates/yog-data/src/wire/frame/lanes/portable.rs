//! The kernels any processor runs: rows as `[u32; LANES]`.
use super::{BLOCK, LANES, WORD_BYTES, mask};

fn store(out: &mut [u8], word: usize, acc: &[u32; LANES]) {
    let at = word * LANES * WORD_BYTES;
    for (dst, w) in out[at..at + LANES * WORD_BYTES]
        .as_chunks_mut::<WORD_BYTES>()
        .0
        .iter_mut()
        .zip(acc)
    {
        *dst = w.to_le_bytes();
    }
}

fn load(src: &[u8], word: usize) -> [u32; LANES] {
    let at = word * LANES * WORD_BYTES;
    let words = src[at..at + LANES * WORD_BYTES].as_chunks::<WORD_BYTES>().0;
    std::array::from_fn(|l| u32::from_le_bytes(words[l]))
}

pub(super) fn pack(src: &[u32; BLOCK], width: u32, out: &mut [u8]) {
    let mut acc = [0u32; LANES];
    let (mut filled, mut word) = (0u32, 0usize);
    for row in src.as_chunks::<LANES>().0 {
        for l in 0..LANES {
            acc[l] |= row[l] << filled;
        }
        filled += width;
        if filled >= u32::BITS {
            store(out, word, &acc);
            word += 1;
            filled -= u32::BITS;
            // What didn't fit starts the next word.
            for l in 0..LANES {
                acc[l] = if filled == 0 {
                    0
                } else {
                    row[l] >> (width - filled)
                };
            }
        }
    }
}

pub(super) fn unpack(src: &[u8], width: u32, out: &mut [u32; BLOCK]) {
    let m = mask(width);
    let mut cur = load(src, 0);
    let (mut used, mut word) = (0u32, 1usize);
    for row in out.as_chunks_mut::<LANES>().0 {
        let end = used + width;
        if end > u32::BITS {
            // The number runs into the next word.
            let next = load(src, word);
            word += 1;
            for l in 0..LANES {
                row[l] = ((cur[l] >> used) | (next[l] << (u32::BITS - used))) & m;
            }
            cur = next;
            used = end - u32::BITS;
            continue;
        }
        for l in 0..LANES {
            row[l] = (cur[l] >> used) & m;
        }
        used = end;
        if used == u32::BITS && word < width as usize {
            cur = load(src, word);
            word += 1;
            used = 0;
        }
    }
}
