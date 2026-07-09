//! Pre-fetched inventory slot data — populated by the JNI layer before each
//! render frame, consumed by [`WidgetKind::InvSlot`] widgets.

use std::sync::Mutex;

/// One slot's render data, fetched once per frame.
#[derive(Debug, Clone, Default)]
pub struct SlotData {
    /// `"namespace:item_id"` — empty string if slot is empty.
    pub item_id: String,
    /// Stack count (0 if empty).
    pub count: u32,
    /// Screen-space pixel position (x, y).
    pub x: i32,
    pub y: i32,
}

/// Global slot cache, populated by `yog-runtime` before each inventory render.
static SLOTS: Mutex<Vec<SlotData>> = Mutex::new(Vec::new());

/// Replace the cached slot data (called by the JNI bridge before rendering).
pub fn set_slot_cache(data: Vec<SlotData>) {
    *SLOTS.lock().unwrap() = data;
}

/// Read cached slot data for the given slot index. Returns `None` if out of range.
pub fn get_slot(index: usize) -> Option<SlotData> {
    SLOTS.lock().unwrap().get(index).cloned()
}

/// Number of cached slots.
pub fn slot_count() -> usize {
    SLOTS.lock().unwrap().len()
}
