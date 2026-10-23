//! Videos drawn as images (`video:mymod:intro`, a `.yui`'s `<Video>`): a
//! player each, opened and played when the video is first drawn, closed
//! once it hasn't been drawn for a while — drawn again, it starts over.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use yog_sync::Mutex;

use jni::JNIEnv;
use yog_event::video;

/// How long a video goes undrawn before its player is let go.
const LINGER: Duration = Duration::from_secs(1);

/// Video id → its player's handle, and when it was last drawn.
static DRAWN: Mutex<Option<HashMap<String, (u32, Instant)>>> = Mutex::new(None);

/// Video `id`'s picture now, its player opened (and played) if it has none;
/// players not drawn lately closed.
pub(crate) fn frame(env: &mut JNIEnv, id: &str) -> Option<video::Picture> {
    super::size(id)?;
    let now = Instant::now();
    let (handle, stale) = {
        let mut drawn = DRAWN.lock();
        let drawn = drawn.get_or_insert_with(HashMap::new);
        let stale: Vec<u32> = drawn
            .extract_if(|_, (_, at)| now.duration_since(*at) > LINGER)
            .map(|(_, (handle, _))| handle)
            .collect();
        let handle = match drawn.get_mut(id) {
            Some((handle, at)) => {
                *at = now;
                Some(*handle)
            }
            None => super::open(&video::Open { id: id.to_owned() }).inspect(|&h| {
                super::with_player(h, |p| p.play(true));
                drawn.insert(id.to_owned(), (h, now));
            }),
        };
        (handle, stale)
    };
    for handle in stale {
        super::close(&video::Close { handle });
    }
    super::with_player(handle?, |p| p.frame(env)).flatten()
}
