//! `checked` and `unchecked` on the JVM: whole numbers wrap by default,
//! panic on overflow in a checked context.

mod support;

const SRC: &str = r#"public static class C {
    public static string Wraps() {
        let big = i32.MaxValue;
        let wrapped = big + 1;
        let n = 300;
        let narrowed = (i8) n;
        let fine = checked(big - (u64) 1);
        let mut t = 0;
        unchecked { t = big * 2; }
        return wrapped + " " + narrowed + " " + fine + " " + t + " " + checked(unchecked(big + 1));
    }
    public static i32 AddOver() => checked(i32.MaxValue + 1);
    public static i64 MulOver() {
        checked {
            let x = i64.MaxValue;
            return x * 2;
        }
    }
    public static i8 CastOver() {
        let n = 300;
        return checked((i8) n);
    }
    public static i32 CompoundOver() {
        let mut x = i32.MaxValue;
        checked { x += 1; }
        return x;
    }
    public static i32 Negated() => checked(-i32.MinValue);
    public static string Unsigned() {
        let big = u64.MaxValue;
        let four = (u64) 4000000000;
        return checked(big - (u64) 1) + " " + checked(four * four) + " " + checked((i32) (four / (u64) 2)) + " " + checked((u64) 7);
    }
    public static u64 UnsignedAddOver() => checked(u64.MaxValue + (u64) 1);
    public static u64 UnsignedSubOver() {
        let zero = (u64) 0;
        return checked(zero - (u64) 1);
    }
    public static u64 UnsignedMulOver() {
        let big = (u64) 5000000000;
        return checked(big * big);
    }
    public static u64 UnsignedFromNegative() {
        let n = -1;
        return checked((u64) n);
    }
    public static i64 UnsignedTooBig() => checked((i64) u64.MaxValue);
}
"#;

/// What `run` (Java, a call of `C`'s) gives, or its panic's message.
fn caught(call: &str) -> String {
    format!(
        "((java.util.function.Supplier<String>) () -> {{ try {{ return String.valueOf(t.CScript.C.{call}()); }} catch (dev.yog.script.YsPanic p) {{ return \"panic\"; }} }}).get()"
    )
}

#[test]
fn unchecked_wraps() {
    let Some(out) = support::run(&[("C.ys", SRC)], &caught("Wraps")) else {
        return;
    };
    assert_eq!(out, "-2147483648 44 2147483646 -2 -2147483648");
}

#[test]
fn checked_panics_on_overflow() {
    for call in [
        "AddOver",
        "MulOver",
        "CastOver",
        "CompoundOver",
        "Negated",
        "UnsignedAddOver",
        "UnsignedSubOver",
        "UnsignedMulOver",
        "UnsignedFromNegative",
        "UnsignedTooBig",
    ] {
        let Some(out) = support::run(&[("C.ys", SRC)], &caught(call)) else {
            return;
        };
        assert_eq!(out, "panic", "{call}");
    }
}

/// `u64` in a checked context: unsigned results kept, its range from both ends.
#[test]
fn checked_unsigned() {
    let Some(out) = support::run(&[("C.ys", SRC)], &caught("Unsigned")) else {
        return;
    };
    assert_eq!(out, "18446744073709551614 16000000000000000000 2000000000 7");
}
