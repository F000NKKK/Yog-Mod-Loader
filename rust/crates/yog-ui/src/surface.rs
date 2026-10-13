//! Surfaces — widgets that draw themselves.
//!
//! A [`Surface`] paints its box with any `yog-gfx` call (shaders, buffers,
//! [`Draw2D`](yog_gfx::draw2d::Draw2D)) and may take the pointer: a
//! casting grid, a graph, a minimap. The tree refers to one by id
//! ([`widget::surface`](crate::widget::surface), `.yui` `<Surface
//! source="mymod:grid"/>`); the code registers the drawing object under
//! that id with [`register`]. An id nothing is registered under draws
//! nothing.
//!
//! ```ignore
//! struct Graph;
//! impl yog_ui::Surface for Graph {
//!     fn paint(&self, gfx: &GfxContext, rect: Rect) {
//!         gfx.draw2d().rect(rect.x, rect.y, rect.x + rect.w, rect.y + 1.0, 0xFFFFFFFF);
//!     }
//! }
//! yog_ui::surface::register("mymod:graph", std::sync::Arc::new(Graph));
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use yog_gfx::GfxContext;

use crate::layout::Rect;

/// A pointer event on a surface, in coordinates local to its box (`0,0` =
/// top-left corner).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pointer {
    /// A button went down over the surface; it gets the drags and the
    /// release that follow, wherever the cursor goes.
    Down { x: f32, y: f32 },
    /// The cursor moved with the button held.
    Drag { x: f32, y: f32 },
    /// The button came up.
    Up { x: f32, y: f32 },
    /// The cursor moved over the surface, no button held.
    Move { x: f32, y: f32 },
    /// The cursor left the surface.
    Leave,
}

/// What a surface widget draws and how it takes the pointer — see the
/// [module docs](self).
pub trait Surface: Send + Sync {
    /// Draws into `rect` (screen GUI pixels, the widget's box less its
    /// padding). Called every frame the widget is visible.
    fn paint(&self, gfx: &GfxContext, rect: Rect);

    /// A pointer event; `true` = used (a `Down` that returns `false` lets
    /// the click through to what's under the surface). `size` is the box's
    /// width and height.
    fn pointer(&self, ev: Pointer, size: (f32, f32)) -> bool {
        let _ = (ev, size);
        false
    }
}

type Registry = HashMap<String, Arc<dyn Surface>>;

static SURFACES: Mutex<Option<Registry>> = Mutex::new(None);

/// Makes `surface` what `<Surface source="id"/>` widgets draw (replacing
/// the one registered before).
pub fn register(id: impl Into<String>, surface: Arc<dyn Surface>) {
    SURFACES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(id.into(), surface);
}

/// Drops the surface registered under `id`.
pub fn unregister(id: &str) {
    if let Some(map) = SURFACES.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        map.remove(id);
    }
}

/// The surface registered under `id`.
pub fn get(id: &str) -> Option<Arc<dyn Surface>> {
    SURFACES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()?
        .get(id)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Counter(AtomicU32);
    impl Surface for Counter {
        fn paint(&self, _: &GfxContext, _: Rect) {}
        fn pointer(&self, _: Pointer, _: (f32, f32)) -> bool {
            self.0.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    #[test]
    fn registry_finds_by_id() {
        assert!(get("test:none").is_none());
        let c = Arc::new(Counter(AtomicU32::new(0)));
        register("test:counter", c.clone());
        let s = get("test:counter").unwrap();
        assert!(s.pointer(Pointer::Leave, (1.0, 1.0)));
        assert_eq!(c.0.load(Ordering::Relaxed), 1);
        unregister("test:counter");
        assert!(get("test:counter").is_none());
    }
}
