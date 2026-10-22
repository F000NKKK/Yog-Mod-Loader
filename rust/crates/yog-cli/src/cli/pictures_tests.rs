use super::*;

/// A `w × w` PNG of `colors` colors.
fn png(w: u32, colors: u32) -> Vec<u8> {
    let rgba = (0..w * w)
        .flat_map(|i| [(i * 7919 % colors) as u8, (i * 31 % colors) as u8, 9, 0xFF])
        .collect();
    image::encode_png(&image::Image {
        width: w,
        height: w,
        rgba,
    })
    .unwrap()
}

#[test]
fn pixel_art_goes_as_a_yimage_and_the_rest_stays() {
    let few = png(16, 4);
    let many = png(64, 251 * 251);
    let mut assets = vec![
        ("assets/m/textures/block/a.png".to_string(), few.clone()),
        ("assets/m/textures/photo.png".to_string(), many.clone()),
        ("data/m/structure.png".to_string(), few.clone()),
    ];
    pack(&mut assets);
    assert!(yimage::is(&assets[0].1), "pixel art packed");
    assert_eq!(
        image::decode(&yimage::to_png(&assets[0].1).unwrap()).unwrap(),
        image::decode(&few).unwrap()
    );
    assert_eq!(assets[1].1, many, "more colors than a palette: a PNG still");
    assert_eq!(assets[2].1, few, "only assets/ are the game's pictures");
}
