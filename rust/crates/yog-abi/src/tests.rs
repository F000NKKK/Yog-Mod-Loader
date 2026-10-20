//! The ABI's layout and version checks.

use super::*;
use std::mem::{align_of, size_of};

/// Sizes and alignments of every type crossing the ABI, folded together.
fn layout_fingerprint() -> u64 {
    macro_rules! layouts {
        ($($t:ty),* $(,)?) => { [$((stringify!($t), size_of::<$t>(), align_of::<$t>())),*] };
    }
    let all = layouts!(
        YogStr, YogOwnedStr, YogBlockPos, YogVec3,
        YogGfxApi, YogServer, YogApi, YogBytes, YogReply, YogBus,
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
    assert_eq!((ABI_VERSION, layout_fingerprint()), (40000, 0xa91a_d009_e18e_fa6b), "layout fingerprint: {:#x}", layout_fingerprint());
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
    let owned = YogOwnedStr::from_string("runtime".to_string());
    assert!(!owned.is_none() && owned.len == 7);
    assert!(YogOwnedStr::NONE.is_none());
    // What `free_str` does with it.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(owned.ptr, owned.len as usize)) });
}
