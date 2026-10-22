use super::*;

fn codegen(src: &str) -> Result<String, String> {
    generate(&yui::parse_xml(src)?, "ui/stat_bar", None)
        .map(|(c, _)| c)
        .map_err(String::from)
}

#[test]
fn generates_a_component() {
    let code = codegen(r##"<Component description="A bar." icon="@Icons.bar" category="Bars" design_w="120" design_h="10" d:bg="#FF000000">
        <Property name="label" default="HP" doc="Left text" category="Text"/>
        <Property name="value" type="number" default="0.5"/>
        <Property name="fill" type="color" default="#FF55FF55"/>
        <Property name="size" type="choice" options="small, big"/>
        <Property name="wide" type="bool" default="true"/>
        <Event name="on_select"/>
        <Row w="100%" gap="4" on_click="{x:Bind on_select}">
            <Label text="{x:Bind label}" d:text="Sample" visible="{x:Bind wide}"/>
            <Spacer w="{x:Bind bar_w()}" h="6" bg="{x:Bind fill}"/>
            <Pill text="{x:Bind label}" tone="red"/>
        </Row>
    </Component>"##)
    .unwrap();
    for want in [
        "pub struct StatBar {",
        "    /// Left text\n    pub label: String,",
        "pub value: f32,",
        "pub fill: u32,",
        "label: p.text(\"label\"),",
        "size: p.word(\"size\").to_string(),",
        "Prop::text(\"label\", \"HP\").doc(\"Left text\").category(\"Text\")",
        "const DOC: &'static str = \"A bar.\";",
        "const ICON: &'static str = \"@Icons.bar\";",
        "const CATEGORY: &'static str = \"Bars\";",
        "const DESIGN_SIZE: (f32, f32) = (120.0, 10.0);",
        "const TOOLBOX: bool = true;",
        "Prop::choice(\"size\", &[\"small\", \"big\"], \"small\")",
        "Prop::bool(\"wide\", true)",
        "Prop::event(\"on_select\")",
        "const NAME: &'static str = \"StatBar\";",
        ".with_text(::yog_api::ui::component::bind::text(&(vm.label)))",
        ".visible(::yog_api::ui::component::bind::flag(vm.wide))",
        ".w(::yog_api::ui::component::bind::units(&(vm.bar_w())))",
        ".bg(::yog_api::ui::component::bind::color(&(vm.fill)))",
        ".on_click(::yog_api::ui::component::bind::text(&(vm.on_select)))",
        "component::build(\"Pill\", &[(\"text\", &*::yog_api::ui::component::bind::text(&(vm.label))), (\"tone\", \"red\")])",
        "::yog_api::ui_component!(StatBar);",
    ] {
        assert!(code.contains(want), "missing {want:?} in\n{code}");
    }
}

#[test]
fn color_properties_take_yres_colors() {
    let mut refs = crate::yres::Refs::default();
    refs.colors.insert("Colors.fill".into(), 0xFF01_0203);
    crate::yui::set_resource_refs(&refs);
    let code = codegen(
        r#"<Component><Property name="fill" type="color" default="@Colors.fill"/><Row bg="@Colors.fill"/></Component>"#,
    );
    let screen = crate::yui::generate_for(
        r#"<Screen id="m:s" width="10" height="10"><Example fill="@Colors.fill"/></Screen>"#,
        "s",
        None,
    );
    crate::yui::set_resource_refs(&crate::yres::Refs::default());
    let code = code.unwrap();
    assert!(
        code.contains(r##"Prop::color("fill", "#FF010203")"##),
        "{code}"
    );
    assert!(
        code.contains(".bg(::yog_api::ui::Color(0xFF010203))"),
        "{code}"
    );
    let (screen, _) = screen.unwrap();
    assert!(screen.contains(r##"("fill", "#FF010203")"##), "{screen}");
}

#[test]
fn rejects_bad_components() {
    for (src, msg) in [
        (r#"<Component name="bad"><Row/></Component>"#, "PascalCase"),
        (
            r#"<Component><Property name="w"/><Row/></Component>"#,
            "every element has",
        ),
        (
            r#"<Component><Property name="halign"/><Row/></Component>"#,
            "every element has",
        ),
        (
            r#"<Component><Property name="margin"/><Row/></Component>"#,
            "every element has",
        ),
        (
            r#"<Component><Property name="a" type="float"/><Row/></Component>"#,
            "expected one of",
        ),
        (
            r#"<Component><Property name="a" type="number" default="x"/><Row/></Component>"#,
            "expected a number",
        ),
        (r#"<Component><Row/><Row/></Component>"#, "one body element"),
        (
            r#"<Component doc="x"><Row/></Component>"#,
            "no attribute `doc`",
        ),
        (
            r#"<Component design_w="wide"><Row/></Component>"#,
            "expected a size",
        ),
        (
            r#"<Component toolbox="no"><Row/></Component>"#,
            "expected true or false",
        ),
        (
            r#"<Component icon="apple"><Row/></Component>"#,
            "image source",
        ),
        (
            r#"<Component><Event name="on_x" type="text"/><Row/></Component>"#,
            "no attribute `type`",
        ),
        (
            r#"<Component><Property name="a"/></Component>"#,
            "has no body",
        ),
        (
            r#"<Component><Row padding="{x:Bind a}"/></Component>"#,
            "can't be bound",
        ),
        (
            r#"<Component><Row bg="{x:Bind cx.close()}"/></Component>"#,
            "cx",
        ),
    ] {
        let e = codegen(src).unwrap_err();
        assert!(e.contains(msg), "{src}: {e}");
    }
}

#[test]
fn index_lists_components_without_code_behind() {
    let dir = std::env::temp_dir().join(format!("yog-components-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src/ui")).unwrap();
    std::fs::write(dir.join("yog.toml"), "").unwrap();
    std::fs::write(
        dir.join("src/ui/pill.yui"),
        "<Component><Label/></Component>",
    )
    .unwrap();
    std::fs::write(dir.join("src/bar.yui"), "<Component><Label/></Component>").unwrap();
    std::fs::write(dir.join("src/bar.rs"), "").unwrap();
    std::fs::write(dir.join("src/main.yui"), r#"<Screen id="a:b"/>"#).unwrap();
    let index = std::fs::read_to_string(write_index(&dir).unwrap().0).unwrap();
    assert!(
        index.contains("pub mod ui_pill {") && index.contains("/generated/ui/pill.g.rs"),
        "{index}"
    );
    assert!(!index.contains("bar") && !index.contains("main"), "{index}");
    let _ = std::fs::remove_dir_all(&dir);
}
