//! The ABI's layout and version checks.

use super::*;
use std::mem::{align_of, size_of};

/// Sizes and alignments of every type crossing the ABI, folded together.
fn layout_fingerprint() -> u64 {
    macro_rules! layouts {
        ($($t:ty),* $(,)?) => { [$((stringify!($t), size_of::<$t>(), align_of::<$t>())),*] };
    }
    let all = layouts!(
        YogStr,
        YogGfxApi, YogApi, YogBytes, YogReply, YogBus,
    );
    // FNV-1a over (name, size, align).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (name, size, align) in all {
        for b in name.bytes().chain((size as u64).to_le_bytes()).chain((align as u64).to_le_bytes()) {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// A type crossing the ABI changed without `ABI_VERSION` changing: bump
/// `ABI_MINOR` (fields appended at the end) or `ABI_MAJOR` (anything
/// else), then record the new fingerprint here.
#[test]
#[cfg(target_pointer_width = "64")]
fn the_layout_matches_its_version() {
    assert_eq!((ABI_VERSION, layout_fingerprint()), (40001, 0x363d_3a40_d0b0_f211), "layout fingerprint: {:#x}", layout_fingerprint());
}

#[test]
fn older_minors_of_the_major_are_compatible() {
    assert!(compatible(ABI_VERSION));
    assert!(compatible(ABI_MAJOR * 10_000));
    assert!(!compatible(ABI_VERSION + 1), "a newer minor may use what isn't here");
    assert!(!compatible((ABI_MAJOR - 1) * 10_000 + ABI_MINOR), "another major");
    assert!(!compatible((ABI_MAJOR + 1) * 10_000));
}

#[test]
fn strings_cross_and_come_back() {
    let s = YogStr::from_str("Ünïcode ✓");
    assert_eq!(unsafe { s.as_str() }, "Ünïcode ✓");
    assert!(YogStr::EMPTY.is_empty());
    assert_eq!(unsafe { YogStr::EMPTY.as_str() }, "");
}
