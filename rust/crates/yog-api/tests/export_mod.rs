//! `export_mod!` expanded as a mod writes it: everything it names is there,
//! and a content registrar the build generates is run with the mod's own.

struct Nothing;

impl yog_api::Mod for Nothing {
    fn register(_: &mut yog_api::Registry) {}
}

yog_api::export_mod!(Nothing);

fn content(_: &mut yog_api::Registry) {}

#[test]
fn a_mod_exports_its_entry_points_and_its_content() {
    // As the generated `yog-content.g.rs` does when the library loads.
    yog_api::__yog_content_registrars().lock().push(content);
    assert_eq!(yog_abi_version(), yog_api::ABI_VERSION);
    assert_eq!(yog_api::__yog_content_registrars().lock().len(), 1);
}
