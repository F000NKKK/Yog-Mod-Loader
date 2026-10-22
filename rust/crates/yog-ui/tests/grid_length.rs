//! A grid size as `.yui` writes it.

use yog_ui::GridLength;

#[test]
fn sizes_read_as_yui_writes_them() {
    assert_eq!(GridLength::parse(" Auto "), Some(GridLength::Auto));
    assert_eq!(GridLength::parse("AUTO"), Some(GridLength::Auto));
    assert_eq!(GridLength::parse("*"), Some(GridLength::STAR));
    assert_eq!(GridLength::parse("2.5 *"), Some(GridLength::Star(2.5)));
    assert_eq!(GridLength::parse("26"), Some(GridLength::Px(26.0)));
    for bad in ["", "x*", "abc", "2**"] {
        assert_eq!(GridLength::parse(bad), None, "{bad}");
    }
}
