//! A one-color picture as a mask: a bit a pixel, drawn in a color.

use yog_graphics::{BYTES_PER_PIXEL, Canvas, Color, Image, Mask, MaskOp, Op, Rect, Vec2};

/// A 9 × 2 picture: a diagonal of white, the rest clear.
fn diagonal() -> Image {
    let mut image = Image::new(9, 2);
    for (x, y) in [(0, 0), (8, 1)] {
        let at = (y * 9 + x) * BYTES_PER_PIXEL;
        image.rgba[at..at + BYTES_PER_PIXEL].copy_from_slice(&[0xFF; BYTES_PER_PIXEL]);
    }
    image
}

#[test]
fn a_one_color_picture_is_a_mask() {
    let (mask, color) = Mask::of(&diagonal()).unwrap();
    assert_eq!(color, Color::WHITE);
    // Rows whole bytes: 9 pixels take 2.
    assert_eq!(mask.bits.len(), 4);
    assert!(mask.get(0, 0) && mask.get(8, 1) && !mask.get(1, 0) && !mask.get(8, 0));
    assert_eq!(mask.image(Color::WHITE), diagonal());
}

#[test]
fn colors_and_soft_edges_stay_an_image() {
    let mut two = diagonal();
    two.rgba[0] = 0;
    assert!(Mask::of(&two).is_none(), "two colors");
    let mut soft = diagonal();
    soft.rgba[BYTES_PER_PIXEL - 1] = 0x80;
    assert!(Mask::of(&soft).is_none(), "a half-clear pixel");
}

#[test]
fn a_mask_is_drawn_in_the_current_color() {
    let (mask, _) = Mask::of(&diagonal()).unwrap();
    let op = Op::Mask(MaskOp {
        rect: Rect::new(0.0, 0.0, 9.0, 2.0),
        mask,
        color: None,
    });
    let mut canvas = Canvas::new(9, 2);
    let red = Color::from_rgba8([0xFF, 0, 0, 0xFF]);
    canvas.draw(&[op], |p| Vec2::new(p.x, 2.0 - p.y), 1.0, red);
    let image = canvas.into_image();
    let px = |x: usize, y: usize| &image.rgba[(y * 9 + x) * BYTES_PER_PIXEL..][..BYTES_PER_PIXEL];
    assert_eq!(px(0, 0), [0xFF, 0, 0, 0xFF]);
    assert_eq!(px(1, 0)[BYTES_PER_PIXEL - 1], 0);
}
