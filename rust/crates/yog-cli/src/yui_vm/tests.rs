use super::expr::parse_expr;
use super::*;

fn codegen(body: &str) -> Result<String, String> {
    let src =
        format!(r#"<Screen id="t:s" width="200" height="100" x:ViewModel="Vm">{body}</Screen>"#);
    crate::yui::generate_for(&src, "s", None).map(|(c, _)| c)
}

#[test]
fn themes() {
    let src = r##"<Screen id="t:s" width="200" height="100" theme="minecraft:vanilla" x:ViewModel="Vm">
        <Screen.Resources><Style x:Key="Red" target="Button" bg="#FFFF0000"/></Screen.Resources>
        <Column>
            <Button x:Name="plain" command="{x:Bind go()}"/>
            <Button x:Name="own" bg="#FF00FF00" command="{x:Bind go()}"/>
            <Button x:Name="styled" style="@Red" command="{x:Bind go()}"/>
        </Column></Screen>"##;
    let code = crate::yui::generate_for(src, "s", None).unwrap().0;
    assert!(code.contains(r#".bg_image("sprite:panel")"#), "{code}");
    assert_eq!(
        code.matches(r#".bg_image("sprite:button")"#).count(),
        1,
        "{code}"
    );
    assert!(
        code.contains("bg(::yog_api::ui::Color(0xFF00FF00))")
            && code.contains("bg(::yog_api::ui::Color(0xFFFF0000))"),
        "{code}"
    );
    let bad = src.replace("minecraft:vanilla", "nope:nope");
    assert!(
        crate::yui::generate_for(&bad, "s", None)
            .unwrap_err()
            .contains("no such theme")
    );
}

#[test]
fn tooltips() {
    let code =
        codegen(r#"<Column><Label tooltip="Static"/><Label tooltip="{x:Bind hint()}"/></Column>"#)
            .unwrap();
    assert!(code.contains(r#".tooltip("Static")"#), "{code}");
    assert!(
        code.contains("::yog_kit::bind::text(&(vm.hint()))"),
        "{code}"
    );
    assert!(
        code.contains(".set_tooltip(::std::clone::Clone::clone("),
        "{code}"
    );
}

#[test]
fn image_state_backgrounds() {
    let code = codegen(r#"<Button bg="sprite:button" hover:bg="sprite:button_hover" disabled:bg="sprite:button_disabled" command="{x:Bind go()}"/>"#).unwrap();
    assert!(code.contains(r#".bg_image("sprite:button")"#), "{code}");
    assert!(code.contains(r#"StateFlags::HOVER, ::yog_api::ui::StylePatch { bg_image: ::std::option::Option::Some(::std::string::String::from("sprite:button_hover"))"#), "{code}");
    assert!(code.contains(r#"bg_image: ::std::option::Option::Some(::std::string::String::from("sprite:button_disabled"))"#), "{code}");
}

#[test]
fn expressions() {
    let e = |s: &str, scope| parse_expr(s).unwrap().emit(scope, false).unwrap();
    assert_eq!(e("count", Scope::Screen), "vm.count");
    assert_eq!(e("count_text()", Scope::Screen), "vm.count_text()");
    assert_eq!(
        e("filter == Filter::All", Scope::Screen),
        "vm.filter == Filter::All"
    );
    assert_eq!(
        e("!has(Filter::Yog)", Scope::Screen),
        "!vm.has(Filter::Yog)"
    );
    assert_eq!(
        e("Strings::count(n, 'x')", Scope::Screen),
        "Strings::count(&vm.n, \"x\")"
    );
    assert_eq!(
        e("Strings::v(version)", Scope::Template),
        "Strings::v(&item.version)"
    );
    assert_eq!(e("selected()?.name", Scope::Screen), "vm.selected()?.name");
    assert_eq!(e("vm.x.len()", Scope::Template), "vm.x.len()");
    assert_eq!(e("item", Scope::Template), "item");
    assert_eq!(e("1.5", Scope::Screen), "1.5");
    assert!(parse_expr("a < b").is_err());
    assert!(
        parse_expr("cx.close()")
            .unwrap()
            .emit(Scope::Screen, false)
            .is_err()
    );
    assert!(
        parse_expr("index")
            .unwrap()
            .emit(Scope::Screen, false)
            .is_err()
    );
}

#[test]
fn yres_colors_everywhere() {
    let mut refs = crate::yres::Refs::default();
    refs.colors.insert("Colors.accent".into(), 0xFF11_2233);
    refs.colors.insert("Colors.hot".into(), 0xFF44_5566);
    refs.colors.insert("Colors.tint".into(), 0x8077_8899);
    crate::yui::set_resource_refs(&refs);
    let code = codegen(
        r##"
        <Screen.Resources>
            <Color x:Key="Accent" value="@Colors.accent"/>
            <Style x:Key="Btn" target="Button" bg="@Colors.accent" hover:bg="@Colors.hot"/>
        </Screen.Resources>
        <Column w="100%">
            <Label text="a" color="@Accent" selected:color="@Accent" disabled:bg="@Colors.hot"/>
            <Button style="@Btn" text="b"/>
            <Image source="item:air" tint="@Colors.tint"/>
        </Column>"##,
    );
    let missing = codegen(r#"<Label text="a" hover:bg="@Colors.nope"/>"#);
    crate::yui::set_resource_refs(&crate::yres::Refs::default());
    let code = code.unwrap();
    for want in [
        ".color(::yog_api::ui::Color(0xFF112233))",
        "StateFlags::SELECTED, ::yog_api::ui::StylePatch { color: ::std::option::Option::Some(::yog_api::ui::Color(0xFF112233))",
        "StateFlags::DISABLED, ::yog_api::ui::StylePatch { bg: ::std::option::Option::Some(::yog_api::ui::Color(0xFF445566))",
        ".bg(::yog_api::ui::Color(0xFF112233))",
        "StateFlags::HOVER, ::yog_api::ui::StylePatch { bg: ::std::option::Option::Some(::yog_api::ui::Color(0xFF445566))",
        ".tint(::yog_api::ui::Color(0x80778899))",
    ] {
        assert!(code.contains(want), "missing {want}:\n{code}");
    }
    assert!(missing.unwrap_err().contains("no color `Colors.nope`"));
}

#[test]
fn bindings_commands_and_parts() {
    let code = codegen(r##"
        <Screen.Resources>
            <Color x:Key="Accent" value="#FFD700"/>
            <Style x:Key="Btn" target="Button" w="40" h="14" bg="#FF333333" hover:bg="#FF444444"/>
            <Style x:Key="Toggle" based_on="@Btn" checked:bg="#FF555555"/>
        </Screen.Resources>
        <Column w="100%">
            <Label x:Name="count" text="{x:Bind count_text()}" color="@Accent"/>
            <Label text="@lang:mymod.title"/>
            <Button style="@Btn" text="Close" command="{x:Bind cx.close()}"/>
            <ToggleButton style="@Toggle" text="Yog" checked="{x:Bind filter == Filter::Yog}"
                          enabled="{x:Bind has(Filter::Yog)}" command="{x:Bind set_filter(Filter::Yog)}"/>
            <TextBox w="100" h="14" text="{x:Bind search, Mode=TwoWay}" placeholder="@lang:mymod.search"
                     command="{x:Bind submit()}"/>
            <ScrollView h="50" scroll_key="{x:Bind search}" visible="{x:Bind show}"/>
        </Column>"##)
    .unwrap();
    for want in [
        "const _: () = ::yog_kit::__codegen::require(3);",
        "type Vm = Vm;",
        ".color(::yog_api::ui::Color(0xFFFFD700))",
        r#".id("count")"#,
        "pub const COUNT: &str = \"count\";",
        "if let ::std::option::Option::Some(__v) = self.b0.changed(::yog_kit::bind::text(&(vm.count_text()))) && let ::std::option::Option::Some(__w) = root.get_path_mut(&[0, 0]) { __w.set_text(",
        r#"::yog_kit::bind::text(&(::yog_api::tr("mymod.title")))"#,
        ".state_style(::yog_api::ui::StateFlags::HOVER, ::yog_api::ui::StylePatch { bg: ::std::option::Option::Some(::yog_api::ui::Color(0xFF444444)), ..::std::default::Default::default() })",
        ".state_style(::yog_api::ui::StateFlags::CHECKED",
        ".action(::yog_api::ui::Action { owner: 0, slot: 0, index: 0 })",
        ".action(::yog_api::ui::Action { owner: 0, slot: 1, index: 0 })",
        " && let ::std::option::Option::Some(__w) = root.get_path_mut(&[0, 3]) { ::yog_kit::bind::set_checked(__w, *__v)",
        "::yog_kit::bind::flag(vm.filter == Filter::Yog)",
        "slot: 0, .. }) => {\n                cx.close();",
        "slot: 1, .. }) => {\n                vm.set_filter(Filter::Yog);",
        "parts.focusable(&[0, 2]);",
        "parts.text_box(&[0, 4]);",
        "parts.scroller(&[0, 5]);",
        "u.set_text_box(0, __v)",
        "::yog_kit::ViewEvent::TextChanged { slot: 0, text } => {\n                vm.search = text;",
        "::yog_kit::ViewEvent::TextSubmitted { slot: 0, text } => {\n                let _ = text; vm.submit();",
        "u.reset_scroll(0);",
        ".visible = *__v",
        "::yog_kit::Screen::<View>::register(registry, vm)",
        "<Vm as ::std::default::Default>::default()",
    ] {
        assert!(code.contains(want), "missing {want}\n{code}");
    }
    assert!(
        !code.contains("text=") || code.contains("// @yui"),
        "{code}"
    );
}

#[test]
fn bound_sources() {
    let code = codegen(
        r##"
        <Column>
            <Surface w="10" h="10" source="{x:Bind grid_id()}"/>
            <Image w="10" h="10" source="{x:Bind icon}"/>
        </Column>"##,
    )
    .unwrap();
    for want in [
        r#"widget::surface("yog:none")"#,
        r#"widget::image("item:air")"#,
        " && let ::std::option::Option::Some(__w) = root.get_path_mut(&[0, 0]) { ::yog_kit::bind::set_source(__w, ",
        " && let ::std::option::Option::Some(__w) = root.get_path_mut(&[0, 1]) { ::yog_kit::bind::set_source(__w, ",
    ] {
        assert!(code.contains(want), "missing {want}\n{code}");
    }
}

#[test]
fn component_properties() {
    let code = codegen(r##"
        <Column>
            <StatBar id="hp" w="80" label="HP" value="{x:Bind hp}" fill="{x:Bind color()}" visible="{x:Bind alive}"/>
            <ListView h="40" items="{x:Bind rows}" key="{x:Bind id}">
                <ListView.ItemTemplate>
                    <DataTemplate>
                        <Pill text="{x:Bind name}"/>
                    </DataTemplate>
                </ListView.ItemTemplate>
            </ListView>
        </Column>"##)
    .unwrap();
    for want in [
        "__props |= self.b0.set(::yog_kit::bind::text(&(vm.hp)));",
        "__props |= self.b1.set(::yog_kit::bind::text(&(vm.color())));",
        r#"::yog_api::ui::component::build("StatBar", &[("label", "HP"), ("value", &**self.b0.get()), ("fill", &**self.b1.get())])"#,
        "u.relayout(); self.b2.reset();",
        r#"::yog_api::ui::component::build("Pill", &[("text", &**inst."#,
    ] {
        assert!(code.contains(want), "missing {want}\n{code}");
    }
    assert!(codegen(r#"<StatBar value="{x:Bind hp, Mode=TwoWay}"/>"#).is_err());
}

#[test]
fn lists() {
    let code = codegen(
        r##"
        <ListView w="100%" h="80" items="{x:Bind visible()}" key="{x:Bind id}"
                  selected="{x:Bind selected_id(), BindBack=select}" item_height="12">
            <ListView.ItemTemplate>
                <DataTemplate x:DataType="Entry">
                    <Row w="100%" selected:bg="#FF333366">
                        <Label text="{x:Bind name}"/>
                        <Button text="x" command="{x:Bind vm.remove(id)}"/>
                    </Row>
                </DataTemplate>
            </ListView.ItemTemplate>
            <ListView.Empty>
                <Label text="{x:Bind empty_text()}"/>
            </ListView.Empty>
        </ListView>
        <ContentPresenter content="{x:Bind selected()}">
            <DataTemplate><Label text="{x:Bind Strings::v(version)}"/></DataTemplate>
        </ContentPresenter>"##,
    )
    .unwrap();
    for want in [
        "struct __T0 {",
        "fn __E0() -> ::yog_api::ui::Widget",
        "l1: ::yog_kit::items::ItemsHost<__T0>,",
        "::yog_kit::items::ItemsHost::new(1, &[0], ::std::option::Option::Some(12.0), ::std::option::Option::Some(0), true)",
        "parts.list(1, &[0], ::std::option::Option::Some(0), ::std::option::Option::Some(12.0));",
        "let __sel = vm.selected_id();",
        "for (index, item) in ::std::iter::IntoIterator::into_iter(vm.visible()).enumerate() {",
        "__it.next(::yog_kit::bind::key(&(item.id)), ::yog_kit::bind::is_selected(&__sel, &(item.id)), __T0::build)",
        "if let ::std::option::Option::Some(__v) = inst.b0.changed(::yog_kit::bind::text(&(item.name))) && let ::std::option::Option::Some(__w) = w.get_path_mut(&[0]) { __w.set_text(",
        "if let ::std::option::Option::Some((w, u)) = __it.finish(::std::option::Option::Some(__E0)) {",
        "self.b0.changed(::yog_kit::bind::text(&(vm.empty_text())))",
        ".action(::yog_api::ui::Action { owner: 1, slot: 0, index: 0 })",
        "::yog_kit::ViewEvent::Action(::yog_api::ui::Action { owner: 1, slot: 0, index }) => {",
        "::std::clone::Clone::clone(&(item.id))",
        "if let ::std::option::Option::Some((__a0,)) = __args { vm.remove(__a0); }",
        "| ::yog_kit::ViewEvent::Select { list: 1, index } => {",
        "vm.select(__k);",
        "::yog_kit::items::ItemsHost::new(2, &[1], ::std::option::Option::None, ::std::option::Option::None, false)",
        "into_iter(vm.selected()).enumerate()",
        "Strings::v(&item.version)",
        "__it.finish(::std::option::Option::None);",
    ] {
        assert!(code.contains(want), "missing {want}\n{code}");
    }
}

#[test]
fn errors() {
    let e = |b: &str| codegen(b).unwrap_err();
    assert!(e(r#"<Button on_click="x"/>"#).contains("command="),);
    assert!(e(r#"<Label w="{x:Bind a}" hover:w="3"/>"#).contains("paint attributes"));
    assert!(e(r#"<TextBox text="{x:Bind name(), Mode=TwoWay}"/>"#).contains("TwoWay"));
    assert!(e(r#"<Label text="{x:Bind a &lt; b}"/>"#).contains("unexpected"));
    assert!(e(r#"<Label color="@Nope"/>"#).contains("no resource"));
    assert!(e(r#"<ListView items="{x:Bind xs}"/>"#).contains("item template"));
    assert!(e(r#"<Label text="{x:Bind index}"/>"#).contains("item templates"));
}

/// The yog-kit fixture's committed `.g.rs` matches the generator
/// (`UPDATE_GOLDEN=1` rewrites it); yog-kit compiles and runs it.
#[test]
fn kit_fixture() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../yog-kit/tests/fixtures");
    for name in ["list", "grid"] {
        let src = std::fs::read_to_string(dir.join(format!("{name}.yui"))).unwrap();
        let (code, warnings) = crate::yui::generate_for(&src, name, None).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let out = dir.join(format!("{name}.g.rs"));
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(&out, &code).unwrap();
        }
        let want = std::fs::read_to_string(&out).unwrap_or_default();
        assert!(
            want == code,
            "{} is stale; rerun with UPDATE_GOLDEN=1",
            out.display()
        );
    }
}

#[test]
fn legacy_screens_unchanged() {
    let src = r#"<Screen id="t:s"><Label text="@lang:t.k" on_click="go"/></Screen>"#;
    let (code, _) = crate::yui::generate_for(src, "s", None).unwrap();
    assert!(code.contains("pub fn render(gfx"), "{code}");
    assert!(!code.contains("yog_kit"), "{code}");
}
