//! The callbacks still outside the bus: UI input and byte sinks.

use super::*;

/// UI event for a `register_ui` screen. `event_id` is one of
/// `click:X:Y`, `drag:X:Y`, `release:X:Y`, `scroll:DY` and (ABI minor 33)
/// `key:KEY:MODS` — a GLFW key code pressed or repeating, with GLFW modifier
/// bits — and `char:CODEPOINT` — a typed character (Unicode scalar value).
/// Call `YogApi::ui_consume_event` while handling `key:`/`char:` to stop the
/// host's default handling of that key (Esc closing the screen, ...).
pub type YogUIEventFn = unsafe extern "C" fn(ud: *mut c_void, ui_id: YogStr, event_id: YogStr);
/// Takes bytes a handler writes (copies them; valid for the call).
pub type YogBytesSink = unsafe extern "C" fn(sink: *mut c_void, data: *const u8, len: usize);
