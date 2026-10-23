//! A frame's lists of floats, four at a time: whether a list of `f64` is
//! `f32` exactly (then it's kept as `f32`s, half the bytes), the narrowing
//! and the widening back — AVX2 where the processor has it, chosen once;
//! the same bytes either way.

use yog_sync::OnceLock;

const F32_BYTES: usize = 4;
const F64_BYTES: usize = 8;

struct Kernels {
    /// Whether every `f64` is an `f32` exactly (bit for bit back).
    fits: fn(&[f64]) -> bool,
    /// The `f64`s as `f32`s, little-endian, onto the bytes.
    narrow: fn(&[f64], &mut [u8]),
    /// `f32`s (little-endian bytes) as `f64`s.
    widen: fn(&[u8], &mut [f64]),
}

fn kernels() -> &'static Kernels {
    static KERNELS: OnceLock<Kernels> = OnceLock::new();
    KERNELS.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return Kernels {
                fits: avx2::fits,
                narrow: avx2::narrow,
                widen: avx2::widen,
            };
        }
        Kernels {
            fits: portable::fits,
            narrow: portable::narrow,
            widen: portable::widen,
        }
    })
}

/// Whether every one of `items` is an `f32` exactly.
pub fn fits_f32(items: &[f64]) -> bool {
    (kernels().fits)(items)
}

/// `items` as `f32`s onto `out`.
pub fn put_narrow(out: &mut Vec<u8>, items: &[f64]) {
    let at = out.len();
    out.resize(at + items.len() * F32_BYTES, 0);
    (kernels().narrow)(items, &mut out[at..]);
}

/// `items` as they are onto `out`.
pub fn put_wide(out: &mut Vec<u8>, items: &[f64]) {
    let at = out.len();
    out.resize(at + items.len() * F64_BYTES, 0);
    for (dst, f) in out[at..]
        .as_chunks_mut::<F64_BYTES>()
        .0
        .iter_mut()
        .zip(items)
    {
        *dst = f.to_le_bytes();
    }
}

/// `f32`s as they are onto `out`.
pub fn put_f32s(out: &mut Vec<u8>, items: &[f32]) {
    let at = out.len();
    out.resize(at + items.len() * F32_BYTES, 0);
    for (dst, f) in out[at..]
        .as_chunks_mut::<F32_BYTES>()
        .0
        .iter_mut()
        .zip(items)
    {
        *dst = f.to_le_bytes();
    }
}

/// The `f32`s of `raw` as `f64`s.
pub fn widen(raw: &[u8]) -> Vec<f64> {
    let mut out = vec![0.0; raw.len() / F32_BYTES];
    (kernels().widen)(raw, &mut out);
    out
}

mod portable;

/// Four at a time: a 256-bit vector of `f64`s, 128 bits of `f32`s.
#[cfg(target_arch = "x86_64")]
mod avx2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kernels_agree_with_the_portable_ones() {
        let exact: Vec<f64> = (0..37).map(|i| f64::from(i) * 0.5 - 3.0).collect();
        let mut odd = exact.clone();
        odd[33] = 0.1;
        let mut zero = exact.clone();
        zero[2] = -0.0;
        for items in [&exact, &odd, &zero] {
            assert_eq!(fits_f32(items), portable::fits(items));
        }
        assert!(fits_f32(&exact) && !fits_f32(&odd) && fits_f32(&zero));
        let (mut a, mut b) = (Vec::new(), vec![0; exact.len() * F32_BYTES]);
        put_narrow(&mut a, &exact);
        portable::narrow(&exact, &mut b);
        assert_eq!(a, b);
        assert_eq!(widen(&a), exact);
    }
}
