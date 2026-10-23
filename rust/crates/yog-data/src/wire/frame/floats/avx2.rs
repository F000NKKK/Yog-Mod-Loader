//! Four at a time: a 256-bit vector of `f64`s, 128 bits of `f32`s.

use super::{F32_BYTES, portable};
use std::arch::x86_64::{
    _mm_loadu_ps, _mm_storeu_ps, _mm256_castpd_si256, _mm256_cmpeq_epi64, _mm256_cvtpd_ps,
    _mm256_cvtps_pd, _mm256_loadu_pd, _mm256_movemask_epi8, _mm256_storeu_pd,
};

/// Floats a vector holds.
const STEP: usize = 4;

pub(super) fn fits(items: &[f64]) -> bool {
    // `kernels` chose this only where the processor has AVX2.
    unsafe { fits_avx2(items) }
}

pub(super) fn narrow(items: &[f64], out: &mut [u8]) {
    assert!(out.len() >= items.len() * F32_BYTES, "room for the f32s");
    unsafe { narrow_avx2(items, out) }
}

pub(super) fn widen(raw: &[u8], out: &mut [f64]) {
    assert!(raw.len() >= out.len() * F32_BYTES, "the f32s");
    unsafe { widen_avx2(raw, out) }
}

#[target_feature(enable = "avx2")]
unsafe fn fits_avx2(items: &[f64]) -> bool {
    let (chunks, rest) = items.as_chunks::<STEP>();
    for c in chunks {
        // In bounds: a chunk is STEP f64s.
        let v = unsafe { _mm256_loadu_pd(c.as_ptr()) };
        let back = _mm256_cvtps_pd(_mm256_cvtpd_ps(v));
        // Bit for bit: -0.0 and NaNs' payloads count.
        let same = _mm256_cmpeq_epi64(_mm256_castpd_si256(v), _mm256_castpd_si256(back));
        if _mm256_movemask_epi8(same) != -1 {
            return false;
        }
    }
    portable::fits(rest)
}

#[target_feature(enable = "avx2")]
unsafe fn narrow_avx2(items: &[f64], out: &mut [u8]) {
    let (chunks, rest) = items.as_chunks::<STEP>();
    for (k, c) in chunks.iter().enumerate() {
        // In bounds: STEP f64s read, STEP f32s written (checked by the caller).
        unsafe {
            let v = _mm256_cvtpd_ps(_mm256_loadu_pd(c.as_ptr()));
            _mm_storeu_ps(out.as_mut_ptr().add(k * STEP * F32_BYTES).cast(), v);
        }
    }
    portable::narrow(rest, &mut out[chunks.len() * STEP * F32_BYTES..]);
}

#[target_feature(enable = "avx2")]
unsafe fn widen_avx2(raw: &[u8], out: &mut [f64]) {
    let n = out.len() / STEP * STEP;
    for k in (0..n).step_by(STEP) {
        // In bounds: STEP f32s read (checked by the caller), STEP f64s written.
        unsafe {
            let v = _mm_loadu_ps(raw.as_ptr().add(k * F32_BYTES).cast());
            _mm256_storeu_pd(out.as_mut_ptr().add(k), _mm256_cvtps_pd(v));
        }
    }
    portable::widen(&raw[n * F32_BYTES..], &mut out[n..]);
}
