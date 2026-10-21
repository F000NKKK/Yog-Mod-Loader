use super::filter_benign_warnings;

#[test]
fn drops_zig_linker_optimization_warning() {
    let stderr = "\
warning: linker stderr: ignoring deprecated linker optimization setting '1'
  |
  = note: `#[warn(linker_messages)]` on by default

warning: `example` (lib) generated 1 warning
    Finished `release` profile [optimized] target(s) in 0.03s
";
    assert_eq!(
        filter_benign_warnings(stderr),
        "    Finished `release` profile [optimized] target(s) in 0.03s\n"
    );
}

#[test]
fn drops_its_summary_when_it_came_twice() {
    let stderr = "\
warning: linker stderr: ignoring deprecated linker optimization setting '1'
  |
  = note: `#[warn(linker_messages)]` on by default

warning: `example` (lib) generated 1 warning (1 duplicate)
    Finished `release` profile [optimized] target(s) in 0.03s
";
    assert_eq!(
        filter_benign_warnings(stderr),
        "    Finished `release` profile [optimized] target(s) in 0.03s\n"
    );
}

#[test]
fn keeps_real_warnings() {
    let stderr = "\
warning: unused variable: `x`
  --> src/lib.rs:1:5
warning: `my-mod` (lib) generated 1 warning
";
    assert_eq!(filter_benign_warnings(stderr), stderr);
}
