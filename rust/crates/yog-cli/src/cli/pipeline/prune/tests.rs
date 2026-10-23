use super::*;

#[test]
fn only_the_units_a_build_used_stay() {
    let profile = std::env::temp_dir()
        .join(format!("yog-prune-{}", std::process::id()))
        .join("release");
    let deps = profile.join("deps");
    let fp = profile.join(".fingerprint");
    std::fs::create_dir_all(&deps).unwrap();
    std::fs::create_dir_all(fp.join("foo-1111111111111111")).unwrap();
    std::fs::create_dir_all(fp.join("foo-2222222222222222")).unwrap();
    for f in [
        "libfoo-1111111111111111.rlib",
        "libfoo-2222222222222222.rlib",
        "libmod.so",
    ] {
        std::fs::write(deps.join(f), b"x").unwrap();
    }
    let mut used = Used::default();
    used.note(&serde_json::json!({
        "reason": "compiler-artifact",
        "filenames": [deps.join("libfoo-1111111111111111.rlib")],
    }));
    used.prune();
    assert!(deps.join("libfoo-1111111111111111.rlib").exists());
    assert!(!deps.join("libfoo-2222222222222222.rlib").exists());
    assert!(deps.join("libmod.so").exists(), "no hash: not a unit");
    assert!(fp.join("foo-1111111111111111").exists());
    assert!(!fp.join("foo-2222222222222222").exists());
    std::fs::remove_dir_all(profile.parent().unwrap()).unwrap();
}

#[test]
fn a_hash_is_sixteen_hex_digits_after_the_last_hyphen() {
    assert_eq!(
        hash_of("libyog_api-0123456789abcdef.rmeta"),
        Some("0123456789abcdef")
    );
    assert_eq!(hash_of("build-script-build"), None);
    assert_eq!(hash_of("yog-pipes.yog"), None);
}
