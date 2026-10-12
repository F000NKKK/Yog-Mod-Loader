// Compiled at test time as a binary (`rustc -g -C opt-level=0
// --edition=2024`, see tests/locals.rs). The `// LOCALS_HERE` line is where
// the test plants its breakpoint; the line number is looked up by scanning
// this file for the marker rather than hard-coded, so edits above it don't
// silently break the test.

use std::hint::black_box;

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]
pub enum Shape {
    Circle { radius: f64 },
    Rect(Point, Point),
    Empty,
}

#[inline(never)]
fn inspect(count: u32, label: &str) -> usize {
    let x: i32 = 42;
    let neg: i64 = -7;
    let flag = true;
    let ch = 'Z';
    let ratio: f64 = 1.5;
    let s = String::from("hi");
    let v = vec![1u8, 2, 3];
    let p = Point { x: 3, y: -4 };
    let shape = Shape::Rect(p, Point { x: 5, y: 6 });
    let circle = Shape::Circle { radius: 2.0 };
    let none: Option<u32> = None;
    let some: Option<u32> = Some(9);
    let arr = [10u16, 20, 30];
    let slice: &[u16] = &arr[1..];
    let tuple = (7u8, -1i16);
    let r = &x;
    let total = {
        let inner = 100u64;
        inner + count as u64 // LOCALS_HERE
    };
    black_box((
        &neg, &flag, &ch, &ratio, &s, &v, &shape, &circle, &none, &some, &slice, &tuple, &r,
        &label,
    ));
    total as usize + label.len()
}

fn main() {
    let n = black_box(5u32);
    let out = inspect(n, "label!");
    black_box(out);
    let _ = black_box(Shape::Empty);
}
