use super::DoctorReport;

#[test]
fn doctor_json_is_valid() {
    let report = DoctorReport {
        cli_version: "1.2.3",
        rust_toolchain: "1.99",
        cargo: Some("cargo 1.99.0 (abc \"q\")".into()),
        rustup: None,
        cargo_zigbuild: true,
        zig: None,
        targets: vec![("linux-x86_64", "x86_64-unknown-linux-gnu", true)],
    };
    let v: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(v["cli_version"], "1.2.3");
    assert_eq!(v["cargo"], "cargo 1.99.0 (abc \"q\")");
    assert!(v["rustup"].is_null());
    assert_eq!(v["targets"][0]["installed"], true);
}
