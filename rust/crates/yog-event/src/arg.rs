//! What an action's argument may be given as: the type itself, or a form
//! that's plainly the same value — text as any string, a position as an
//! array or a tuple, bytes as a slice. A number is only itself, so a
//! literal reads as the type the action takes (`give_item(p, id, 1)`).

use std::borrow::Cow;

use yog_core::{BlockPos, Point};
use yog_data::Atom;

/// An argument given as `Self` for a field of type `T`.
pub trait Arg<T> {
    fn arg(self) -> T;
}

impl<T> Arg<T> for T {
    fn arg(self) -> T {
        self
    }
}

macro_rules! args {
    ($($from:ty => $to:ty, |$v:ident| $make:expr;)*) => {
        $(impl Arg<$to> for $from {
            fn arg(self) -> $to {
                let $v = self;
                $make
            }
        })*
    };
}

args! {
    &str => String, |s| s.to_owned();
    &String => String, |s| s.clone();
    Cow<'_, str> => String, |s| s.into_owned();
    Atom => String, |a| a.as_str().to_owned();
    &Atom => String, |a| a.as_str().to_owned();
    Option<&str> => Option<String>, |s| s.map(str::to_owned);
    Option<&String> => Option<String>, |s| s.cloned();
    &[u8] => Vec<u8>, |b| b.to_vec();
    &Vec<u8> => Vec<u8>, |b| b.clone();
    &[&str] => Vec<String>, |l| l.iter().map(|s| (*s).to_owned()).collect();
    Vec<&str> => Vec<String>, |l| l.into_iter().map(str::to_owned).collect();
    &[String] => Vec<String>, |l| l.to_vec();
    [i32; 3] => BlockPos, |p| p.into();
    (i32, i32, i32) => BlockPos, |p| p.into();
    [f64; 3] => Point, |p| p.into();
    (f64, f64, f64) => Point, |p| p.into();
    BlockPos => Point, |p| p.into();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take<T>(v: impl Arg<T>) -> T {
        v.arg()
    }

    #[test]
    fn a_literal_is_the_number_taken_and_text_any_string() {
        let n: u32 = take(1);
        assert_eq!(n, 1);
        let s: String = take("a");
        assert_eq!(s, "a");
        let p: BlockPos = take([1, 2, 3]);
        assert_eq!(p, BlockPos::new(1, 2, 3));
        let o: Option<String> = take(Some("u"));
        assert_eq!(o.as_deref(), Some("u"));
    }
}
