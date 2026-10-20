pub(super) fn fits(items: &[f64]) -> bool {
    items
        .iter()
        .all(|f| f64::from(*f as f32).to_bits() == f.to_bits())
}
use super::F32_BYTES;

pub(super) fn narrow(items: &[f64], out: &mut [u8]) {
    for (dst, f) in out.as_chunks_mut::<F32_BYTES>().0.iter_mut().zip(items) {
        *dst = (*f as f32).to_le_bytes();
    }
}

pub(super) fn widen(raw: &[u8], out: &mut [f64]) {
    for (dst, b) in out.iter_mut().zip(raw.as_chunks::<F32_BYTES>().0) {
        *dst = f64::from(f32::from_le_bytes(*b));
    }
}
