//! What a screen is made of: widgets and their styles, a mod's own
//! components, surfaces that draw themselves, the attributes `.yui`
//! elements take, themes, images, inventory widgets' ids.

pub mod attr;
pub mod component;
pub mod image;
pub mod inventory_ids;
#[cfg(test)]
mod state_tests;
pub mod surface;
pub mod theme;
pub mod widget;
