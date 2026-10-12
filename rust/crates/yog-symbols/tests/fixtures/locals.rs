// Compiled at test time as a binary (`rustc -g -C opt-level=0
// --edition=2024`, see tests/locals.rs). The `// LOCALS_HERE` line is where
// the test plants its breakpoint; the line number is looked up by scanning
// this file for the marker rather than hard-coded, so edits above it don't
// silently break the test.

use std::cell::{Cell, OnceCell, RefCell};
use std::hint::black_box;
use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub struct Tagged {
    pub id: u32,
    pub _marker: PhantomData<u8>,
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
    let cell = Cell::new(11i32);
    let refcell = RefCell::new(vec![4u8, 5]);
    let once: OnceLock<u32> = OnceLock::new();
    once.get_or_init(|| 77);
    let empty_once: OnceLock<u32> = OnceLock::new();
    let once_cell: OnceCell<u8> = OnceCell::new();
    let _ = once_cell.set(8);
    let rc = Rc::new(String::from("shared"));
    let rc2 = Rc::clone(&rc);
    let arc = Arc::new(31i32);
    let nn = NonNull::from(&x);
    let pinned: Pin<Box<i32>> = Box::pin(12);
    let tagged = Tagged { id: 3, _marker: PhantomData };
    let mu = MaybeUninit::new(64u32);
    let md = ManuallyDrop::new(16u16);
    let total = {
        let inner = 100u64;
        inner + count as u64 // LOCALS_HERE
    };
    black_box((
        &neg, &flag, &ch, &ratio, &s, &v, &shape, &circle, &none, &some, &slice, &tuple, &r,
        &label, &cell, &refcell, &once, &empty_once, &once_cell, &rc, &rc2, &arc, &nn, &pinned,
        &tagged, &mu, &md,
    ));
    total as usize + label.len()
}

/// Like a UI layout helper: `lines` only comes into scope after its own
/// `let` line, so a stop on that line must still list it (not yet live).
#[inline(never)]
fn row_height(len: usize, expanded: bool, row_w: f32) -> f32 {
    if !expanded {
        return 10.0;
    }
    let lines = black_box(len / (row_w as usize).max(1)) as f32; // PENDING_HERE
    let padded = lines * 2.0 + 6.0;
    black_box(padded) // LIVE_HERE
}

/// Forced inline even at -O0, so the marker line sits in an
/// `inlined_subroutine` inside `caller`.
#[inline(always)]
fn helper(a: u32) -> u32 {
    let doubled = black_box(a.wrapping_mul(2));
    black_box(doubled) // INLINE_HERE
}

#[inline(never)]
fn caller(b: u32) -> u32 {
    let before = black_box(b + 1);
    let r = helper(before);
    let after = black_box(r + 1);
    after
}

fn main() {
    let n = black_box(5u32);
    let out = inspect(n, "label!");
    black_box(out);
    black_box(row_height(black_box(40), black_box(true), black_box(8.0)));
    black_box(caller(black_box(3)));
    let _ = black_box(Shape::Empty);
}
