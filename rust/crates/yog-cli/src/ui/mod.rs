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
