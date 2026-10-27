//! Jumps and verbatim strings run on the JVM: `goto` to a label before or
//! after it, out of loops, past a block's locals, beside a loop's `break`
//! and `continue`; `goto case`/`goto default`; `@"…"`.

mod support;

const SRC: &str = r#"public static class G {
    static i32 Retry() {
        let mut tries = 0;
    again:
        tries += 1;
        if (tries < 3) goto again;
        return tries;
    }
    static string Find() {
        let mut found = "none";
        for (let mut i = 0; i < 5; i += 1) {
            for (let mut j = 0; j < 5; j += 1) {
                if (i * j == 6) {
                    found = "{i},{j}";
                    goto done;
                }
            }
        }
    done:
        return found;
    }
    static i32 Hoisted() {
        let mut n = 0;
    top:
        let k = n * 2;
        n += 1;
        if (n < 4) goto top;
        return k + n;
    }
    static string Outer() {
        let mut s = "";
        foreach (let i in 0..5) {
        mark:
            if (i == 1) continue;
            if (i == 3) break;
            s = s + i;
        }
        return s;
    }
    static string Steps(i32 n) {
        let mut s = "";
        switch (n) {
            case 1:
                s = s + "one ";
                goto case 2;
            case 2:
                s = s + "two ";
                goto default;
            default:
                s = s + "end";
                break;
        }
        return s;
    }
    static string Path(string name) => @"C:\mods\{name}\""q""";
    public static string Run() => Retry() + " " + Find() + " " + Hoisted() + " " + Outer() + " " + Steps(1) + "|" + Steps(5) + "|" + Path("x");
}
"#;

/// What each jump gives, and a verbatim string's backslashes, quotes and hole.
#[test]
fn gotos_and_verbatim_strings() {
    let Some(out) = support::run(&[("G.ys", SRC)], "t.GScript.G.Run()") else {
        return;
    };
    assert_eq!(out, r#"3 2,3 10 02 one two end|end|C:\mods\x\"q""#);
}

const MACHINES: &str = r#"using System.Collections.Generic;
using System.Threading.Tasks;
public static class A {
    static async Task<i32> Retry() {
        let mut tries = 0;
    again:
        await Task.Yield();
        tries += 1;
        if (tries < 3) goto again;
        return tries;
    }
    static IEnumerable<i32> Evens() {
        let mut i = 0;
    top:
        if (i > 6) yield break;
        if (i % 2 == 0) yield return i;
        i += 1;
        goto top;
    }
    static async Task<string> Found() {
        foreach (let x in 0..5) {
            await Task.Yield();
            if (x == 3) goto done;
        }
        return "none";
    done:
        return "three";
    }
    public static string Run() {
        let mut evens = "";
        foreach (let e in Evens()) {
            evens = evens + e;
        }
        return Retry().Result + " " + evens + " " + Found().Result;
    }
}
"#;

/// `goto` in an `async` method's and an iterator's machine: back across an
/// `await`, round a `yield`, forward out of a loop that awaits.
#[test]
fn gotos_in_machines() {
    let Some(out) = support::run(&[("A.ys", MACHINES)], "t.AScript.A.Run()") else {
        return;
    };
    assert_eq!(out, "3 0246 three");
}
