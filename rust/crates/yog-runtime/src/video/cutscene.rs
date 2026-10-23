//! Cutscenes: a video full screen over the game — the runtime's own screen
//! `yog:cutscene`, the video letterboxed on black. It closes at the video's
//! end, or on Esc when it may be skipped (else Esc does nothing).

use yog_sync::Mutex;

use jni::JNIEnv;
use yog_event::{ui, video};
use yog_gfx::image::{Stretch, fit};
use yog_gfx::{Color, Rect, Vec2};
use yog_ui::UiEvent;
use yog_ui::event::keys;

use yog_ids::yog::CUTSCENE_SCREEN;

/// The cutscene showing.
struct Showing {
    handle: u32,
    skippable: bool,
}

static SHOWING: Mutex<Option<Showing>> = Mutex::new(None);

/// Starts cutscene `c`, ending the one showing; whether it started.
pub(super) fn start(c: &video::Cutscene) -> bool {
    let Some(handle) = super::open(&video::Open { id: c.id.clone() }) else {
        return false;
    };
    super::with_player(handle, |p| p.play(true));
    let showing = Showing {
        handle,
        skippable: c.skippable,
    };
    if let Some(old) = SHOWING.lock().replace(showing) {
        super::close(&video::Close { handle: old.handle });
    }
    // Modal, not paused: a paused game pauses its sounds.
    crate::bus::call(&ui::OpenWithFlags {
        ui_id: CUTSCENE_SCREEN.to_owned(),
        flags: ui::flags::MODAL as i32,
    }) == Some(true)
}

/// Draws screen `id` when it's the cutscene's (`w`×`h` GUI pixels); whether it was.
pub(crate) fn render(env: &mut JNIEnv, id: &str, (w, h): (f32, f32)) -> bool {
    if id != CUTSCENE_SCREEN {
        return false;
    }
    let Some(handle) = SHOWING.lock().as_ref().map(|s| s.handle) else {
        return true;
    };
    // SAFETY: called while the screen draws, as every `draw2d_*` is.
    unsafe { crate::gfx::table::gfx_draw2d_rect(0.0, 0.0, w, h, Color::BLACK.0) };
    let Some(p) = super::with_player(handle, |p| p.frame(env)).flatten() else {
        return true;
    };
    let size = Vec2::new(p.width as f32, p.height as f32);
    let (dest, _) = fit(Stretch::Uniform, size, Rect::new(0.0, 0.0, w, h));
    crate::gfx::textures::draw(env, &p.texture, (p.width, p.height), dest, Color::WHITE);
    if !p.playing && p.seconds >= p.length {
        crate::bus::call(&ui::Close {
            ui_id: CUTSCENE_SCREEN.to_owned(),
        });
    }
    true
}

/// Screen `id`'s input `event` when it's the cutscene's: whether it's used
/// (Esc is, unless the cutscene may be skipped); none for other screens.
pub(crate) fn input(id: &str, event: &str) -> Option<bool> {
    if id != CUTSCENE_SCREEN {
        return None;
    }
    Some(match UiEvent::parse(event) {
        UiEvent::Close => {
            if let Some(s) = SHOWING.lock().take() {
                super::close(&video::Close { handle: s.handle });
            }
            true
        }
        UiEvent::Key { key, .. } if key == keys::ESCAPE => {
            !SHOWING.lock().as_ref().is_some_and(|s| s.skippable)
        }
        _ => true,
    })
}
