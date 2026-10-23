use super::*;

#[test]
fn places_problems_at_their_lines() {
    let src = "first\nsecond word";
    let mut r = Report::new("a.ys", src);
    let word = Span::new(13, 17);
    assert_eq!(r.text(word), "word");
    assert_eq!(r.line_of(word), 2);
    r.error("YS0001", word, "bad");
    let d = &r.out[0];
    assert_eq!((d.line, d.end_line), (Some(2), Some(2)));
    assert!(d.is_error());
}
