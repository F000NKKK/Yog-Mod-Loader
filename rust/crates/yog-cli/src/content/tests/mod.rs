use super::*;

fn project(files: &[(&str, &str)]) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "yog-content-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

/// 1×1 pictures named `names` in `resources/Images.yres` of the mod at `root`, and the refs its resources give.
fn pictures(root: &Path, names: &[&str]) -> yres::Refs {
    let png = yog_media::image::encode_png(&yog_media::image::Image { width: 1, height: 1, rgba: vec![255, 0, 0, 255] }).unwrap();
    let entries: Vec<yres::Entry> = names.iter().map(|n| yres::Entry::picture(n, png.clone())).collect();
    std::fs::create_dir_all(root.join("resources")).unwrap();
    std::fs::write(root.join("resources/Images.yres"), yres::write(&entries)).unwrap();
    yres::compile(root, "mymod").unwrap().refs
}

fn output(c: &Compiled, path: &str) -> Value {
    let (_, b) = c
        .outputs
        .iter()
        .find(|(p, _)| p == path)
        .unwrap_or_else(|| {
            panic!(
                "no {path} in {:?}",
                c.outputs.iter().map(|(p, _)| p).collect::<Vec<_>>()
            )
        });
    serde_json::from_slice(b).unwrap()
}

mod blocks;
mod recipes;
mod books;
mod items;
mod data;
mod bodies;
