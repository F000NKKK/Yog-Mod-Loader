//! Screens: `.yui` read and compiled to Rust (`yui`), components, the
//! grid and resources they use, their view models (`vm`), a mod's book
//! page kinds and recipe areas.

pub mod book_page;
pub mod component;
pub mod grid;
pub mod recipe_area;
pub(crate) mod resources;
pub(crate) mod vm;
pub mod yui;

/// The header of the Rust generated from `src/<name>.yui`.
pub(crate) fn generated_header(name: &str) -> String {
    yog_codegen::Header {
        tool: "`yog build`",
        source: &format!("{}/{name}.{}", yog_ids::project::SRC, yog_ids::Format::Ui.ext()),
        regenerate: yog_codegen::Regenerate::EveryBuild,
    }
    .comment::<yog_codegen_rust::Rust>()
}
