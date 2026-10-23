//! The same, a row a 256-bit vector.

use super::{BLOCK, ROWS, block_bytes, mask};
use std::arch::x86_64::{
    __m128i, __m256i, _mm_cvtsi32_si128, _mm256_and_si256, _mm256_loadu_si256, _mm256_or_si256,
    _mm256_set1_epi32, _mm256_setzero_si256, _mm256_sll_epi32, _mm256_srl_epi32,
    _mm256_storeu_si256,
};

pub(super) fn pack(src: &[u32; BLOCK], width: u32, out: &mut [u8]) {
    assert!(out.len() >= block_bytes(width), "a block's bytes");
    // `kernels` chose this only where the processor has AVX2.
    unsafe { pack_avx2(src, width, out) }
}

pub(super) fn unpack(src: &[u8], width: u32, out: &mut [u32; BLOCK]) {
    assert!(src.len() >= block_bytes(width), "a block's bytes");
    unsafe { unpack_avx2(src, width, out) }
}

fn count(n: u32) -> __m128i {
    unsafe { _mm_cvtsi32_si128(n as i32) }
}

#[target_feature(enable = "avx2")]
unsafe fn pack_avx2(src: &[u32; BLOCK], width: u32, out: &mut [u8]) {
    let rows = src.as_ptr().cast::<__m256i>();
    let dst = out.as_mut_ptr().cast::<__m256i>();
    let mut acc = _mm256_setzero_si256();
    let (mut filled, mut word) = (0u32, 0usize);
    for r in 0..ROWS {
        // In bounds: ROWS rows of LANES words; `width` words written.
        let row = unsafe { _mm256_loadu_si256(rows.add(r)) };
        acc = _mm256_or_si256(acc, _mm256_sll_epi32(row, count(filled)));
        filled += width;
        if filled >= u32::BITS {
            unsafe { _mm256_storeu_si256(dst.add(word), acc) };
            word += 1;
            filled -= u32::BITS;
            acc = if filled == 0 {
                _mm256_setzero_si256()
            } else {
                _mm256_srl_epi32(row, count(width - filled))
            };
        }
    }
}

#[target_feature(enable = "avx2")]
unsafe fn unpack_avx2(src: &[u8], width: u32, out: &mut [u32; BLOCK]) {
    let words = src.as_ptr().cast::<__m256i>();
    let rows = out.as_mut_ptr().cast::<__m256i>();
    let m = _mm256_set1_epi32(mask(width) as i32);
    // In bounds: `width` words read, ROWS rows written.
    let mut cur = unsafe { _mm256_loadu_si256(words) };
    let (mut used, mut word) = (0u32, 1usize);
    for r in 0..ROWS {
        let end = used + width;
        let v = if end > u32::BITS {
            let next = unsafe { _mm256_loadu_si256(words.add(word)) };
            word += 1;
            let v = _mm256_or_si256(
                _mm256_srl_epi32(cur, count(used)),
                _mm256_sll_epi32(next, count(u32::BITS - used)),
            );
            cur = next;
            used = end - u32::BITS;
            v
        } else {
            let v = _mm256_srl_epi32(cur, count(used));
            used = end;
            if used == u32::BITS && word < width as usize {
                cur = unsafe { _mm256_loadu_si256(words.add(word)) };
                word += 1;
                used = 0;
            }
            v
        };
        unsafe { _mm256_storeu_si256(rows.add(r), _mm256_and_si256(v, m)) };
    }
}
