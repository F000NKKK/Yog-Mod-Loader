use super::*;

#[test]
fn every_case_from_every_case() {
    for name in [
        "stat_bar", "stat-bar", "statBar", "StatBar", "stat.bar", "stat/bar",
    ] {
        assert_eq!(pascal(name), "StatBar", "{name}");
        assert_eq!(camel(name), "statBar", "{name}");
        assert_eq!(snake(name), "stat_bar", "{name}");
        assert_eq!(screaming(name), "STAT_BAR", "{name}");
    }
}

#[test]
fn each_capital_starts_a_word() {
    assert_eq!(snake("PipeFlow"), "pipe_flow");
    assert_eq!(snake("My--Mod"), "my_mod");
    assert_eq!(pascal("item_2d"), "Item2d");
    assert_eq!(camel(""), "");
}
