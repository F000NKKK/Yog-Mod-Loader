use super::*;
use yog_script::index::access;

/// A class file assembled by hand: `public class a/B extends
/// java/lang/Object implements a/I { int x; boolean f(float) }`, a long
/// constant (two slots) in its pool and an attribute on its method.
fn class_bytes() -> Vec<u8> {
    let mut b = MAGIC.to_be_bytes().to_vec();
    b.extend([0, 0, 0, 65]);
    let utf8 = |s: &str| {
        let mut e = vec![1];
        e.extend((s.len() as u16).to_be_bytes());
        e.extend(s.as_bytes());
        e
    };
    let mut pool: Vec<Vec<u8>> = vec![
        utf8("a/B"),                      // 1
        vec![7, 0, 1],                    // 2: class a/B
        utf8("java/lang/Object"),         // 3
        vec![7, 0, 3],                    // 4
        utf8("a/I"),                      // 5
        vec![7, 0, 5],                    // 6
        vec![5, 0, 0, 0, 0, 0, 0, 0, 42], // 7 (and 8): a long
        utf8("x"),                        // 9
        utf8("I"),                        // 10
        utf8("f"),                        // 11
        utf8("(F)Z"),                     // 12
        utf8("Code"),                     // 13
    ];
    let count: u16 = 14;
    b.extend(count.to_be_bytes());
    pool.iter_mut().for_each(|e| b.append(e));
    b.extend((access::PUBLIC).to_be_bytes());
    b.extend([0, 2, 0, 4]);
    b.extend([0, 1, 0, 6]);
    // A field: access, name, descriptor, no attributes.
    b.extend([0, 1, 0, 0, 0, 9, 0, 10, 0, 0]);
    // A method with one attribute of three bytes.
    b.extend([0, 1, 0, 1, 0, 11, 0, 12, 0, 1, 0, 13, 0, 0, 0, 3, 1, 2, 3]);
    // No attributes of the class's own.
    b.extend([0, 0]);
    b
}

#[test]
fn a_class_reads() {
    let c = read(&class_bytes()).unwrap();
    assert_eq!(c.name, "a/B");
    assert_eq!(c.extends.as_deref(), Some("java/lang/Object"));
    assert_eq!(c.implements, ["a/I"]);
    assert_eq!(c.access, access::PUBLIC);
    assert_eq!(
        (c.fields[0].name.as_str(), c.fields[0].desc.as_str()),
        ("x", "I")
    );
    let f = &c.methods[0];
    assert_eq!(
        (f.name.as_str(), f.desc.as_str(), f.access),
        ("f", "(F)Z", access::PUBLIC)
    );
}

#[test]
fn what_isnt_a_class_doesnt_read() {
    assert_eq!(read(b"not a class"), None);
    let mut cut = class_bytes();
    cut.truncate(cut.len() - 5);
    assert_eq!(read(&cut), None);
}
