#[path = "../benches/support/mod.rs"]
mod support;
use yog_ui::layout;
#[test]
fn dump() {
    let Ok(path) = std::env::var("DUMP") else {
        return;
    };
    let mut out = String::new();
    for n in [10, 100, 300] {
        for t in [support::screen(n), support::grid(n), support::list(n)] {
            for (w, h) in [(640.0, 360.0), (200.0, 100.0), (f32::MAX, 300.0)] {
                out += &format!("{:?}\n", layout::compute(&t, w, h));
            }
        }
    }
    for d in [1, 3, 6] {
        out += &format!("{:?}\n", layout::compute(&support::nested(d), 640.0, 360.0));
    }
    std::fs::write(path, out).unwrap();
}
