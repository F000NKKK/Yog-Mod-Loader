//! An AV1 decoder: rav1d (dav1d ported to Rust), through its C API — the
//! only one it has. Every call into it is in this file.

use std::mem::MaybeUninit;
use std::ptr::NonNull;

use rav1d::include::dav1d::data::Dav1dData;
use rav1d::include::dav1d::dav1d::{Dav1dContext, Dav1dSettings};
use rav1d::include::dav1d::headers::DAV1D_PIXEL_LAYOUT_I420;
use rav1d::include::dav1d::picture::Dav1dPicture;
use rav1d::src::lib::{
    dav1d_close, dav1d_data_create, dav1d_default_settings, dav1d_flush, dav1d_get_picture,
    dav1d_open, dav1d_picture_unref, dav1d_send_data,
};

use super::yuv::{CHROMA_STEP, Plane, Planes};

/// What dav1d answers when all went well.
const OK: i32 = 0;
/// The bits per colour channel the build encodes.
const BITS: i32 = 8;
/// One thread, one frame in flight: each packet gives its picture at once.
const THREADS: i32 = 1;
const FRAME_DELAY: i32 = 1;

/// Why decoding failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The decoder couldn't start.
    Open(i32),
    /// A packet was refused.
    Packet(i32),
    /// No memory for a packet.
    Memory,
    /// A picture isn't 8-bit 4:2:0.
    Format,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Open(e) => write!(f, "the AV1 decoder couldn't start ({e})"),
            DecodeError::Packet(e) => write!(f, "an AV1 packet was refused ({e})"),
            DecodeError::Memory => write!(f, "no memory for an AV1 packet"),
            DecodeError::Format => write!(f, "a frame isn't 8-bit 4:2:0"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A decoder of one video's packets, fed in order from a keyframe.
pub struct Decoder {
    ctx: Option<Dav1dContext>,
}

// SAFETY: the context is only used through `&mut self`; dav1d contexts may
// move between threads, never be shared.
unsafe impl Send for Decoder {}

impl Decoder {
    pub fn new() -> Result<Decoder, DecodeError> {
        let mut s = MaybeUninit::<Dav1dSettings>::uninit();
        // SAFETY: `dav1d_default_settings` fills every field of `s`.
        let mut s = unsafe {
            dav1d_default_settings(NonNull::from(&mut s).cast());
            s.assume_init()
        };
        s.n_threads = THREADS;
        s.max_frame_delay = FRAME_DELAY;
        let mut ctx = None;
        // SAFETY: both pointers are to live locals; on success `ctx` holds
        // a context `Drop` closes.
        let r = unsafe { dav1d_open(Some(NonNull::from(&mut ctx)), Some(NonNull::from(&mut s))) };
        if r.0 != OK {
            return Err(DecodeError::Open(r.0));
        }
        Ok(Decoder { ctx })
    }

    /// Forgets the frames it has: the next packet must be a keyframe.
    pub fn reset(&mut self) {
        if let Some(ctx) = self.ctx {
            // SAFETY: `ctx` is the open context.
            unsafe { dav1d_flush(ctx) };
        }
    }

    /// `packet` decoded; its picture handed to `show` (as planes and their
    /// size) when it has one to show.
    pub fn decode<R>(
        &mut self,
        packet: &[u8],
        show: impl FnOnce(Planes, (usize, usize)) -> R,
    ) -> Result<Option<R>, DecodeError> {
        self.send(packet)?;
        let mut pic = Dav1dPicture::default();
        // SAFETY: `pic` is a live default picture dav1d fills.
        let r = unsafe { dav1d_get_picture(self.ctx, Some(NonNull::from(&mut pic))) };
        if r.0 != OK {
            // No picture yet (or one not shown): not an error.
            return Ok(None);
        }
        let shown = planes(&pic).map(|(planes, size)| show(planes, size));
        // SAFETY: `pic` was filled by `dav1d_get_picture`; its planes
        // aren't used past here.
        unsafe { dav1d_picture_unref(Some(NonNull::from(&mut pic))) };
        shown.map(Some)
    }

    /// `packets` decoded in order, the last one's picture — its top-left
    /// `size` — written into `rgba` as RGBA.
    pub fn decode_last<'a>(
        &mut self,
        packets: impl IntoIterator<Item = &'a [u8]>,
        size: (usize, usize),
        rgba: &mut Vec<u8>,
    ) -> Result<(), DecodeError> {
        let mut packets = packets.into_iter().peekable();
        while let Some(packet) = packets.next() {
            let last = packets.peek().is_none();
            self.decode(packet, |planes, _| {
                if last {
                    super::yuv::to_rgba(planes, size, rgba);
                }
            })?;
        }
        Ok(())
    }

    fn send(&mut self, packet: &[u8]) -> Result<(), DecodeError> {
        let mut data = Dav1dData::default();
        // SAFETY: dav1d allocates `packet.len()` bytes for `data` and
        // returns them; the copy fills exactly those.
        unsafe {
            let buf = dav1d_data_create(Some(NonNull::from(&mut data)), packet.len());
            if buf.is_null() {
                return Err(DecodeError::Memory);
            }
            std::ptr::copy_nonoverlapping(packet.as_ptr(), buf, packet.len());
        }
        // SAFETY: `data` holds the packet; dav1d takes it over.
        let r = unsafe { dav1d_send_data(self.ctx, Some(NonNull::from(&mut data))) };
        // A full queue (`EAGAIN`, negative) can't happen one frame in flight.
        if r.0 != OK {
            return Err(DecodeError::Packet(r.0));
        }
        Ok(())
    }
}

/// `pic`'s planes and size, borrowed while it lives.
fn planes(pic: &Dav1dPicture) -> Result<(Planes<'_>, (usize, usize)), DecodeError> {
    if pic.p.bpc != BITS || pic.p.layout != DAV1D_PIXEL_LAYOUT_I420 {
        return Err(DecodeError::Format);
    }
    let (w, h) = (pic.p.w.max(0) as usize, pic.p.h.max(0) as usize);
    let (ys, cs) = (pic.stride[0].unsigned_abs(), pic.stride[1].unsigned_abs());
    let plane = |i: usize, stride: usize, rows: usize| -> Result<Plane<'_>, DecodeError> {
        let ptr = pic.data[i].ok_or(DecodeError::Format)?;
        // SAFETY: dav1d's planes are `stride` bytes a row, `rows` rows,
        // alive until the picture is unreferenced (after the borrow ends).
        let bytes = unsafe { std::slice::from_raw_parts(ptr.as_ptr().cast::<u8>(), stride * rows) };
        Ok(Plane { bytes, stride })
    };
    let ch = h.div_ceil(CHROMA_STEP);
    Ok((
        Planes {
            y: plane(0, ys, h)?,
            u: plane(1, cs, ch)?,
            v: plane(2, cs, ch)?,
        },
        (w, h),
    ))
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: `ctx` is the open context (or none); closing sets it none.
        unsafe { dav1d_close(Some(NonNull::from(&mut self.ctx))) };
    }
}
