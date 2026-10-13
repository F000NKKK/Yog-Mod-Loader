//! Synthetic inputs for the benches, generated at a given size so every
//! bench covers the same code paths at 10/100/1000-ish scale. Generic on
//! purpose — no real mod's data.

#![allow(dead_code)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The mod id of every synthetic project.
pub const MOD_ID: &str = "example";

/// The sizes benches are run at.
pub const SIZES: [usize; 3] = [10, 100, 1000];

/// A `.yres` set of `n` entries — texts (some with a comment or an explicit
/// key), images and colors — and its `ru_ru` variant translating the texts.
pub fn yres_set(n: usize) -> (String, String) {
    let (mut neutral, mut ru) = (String::from("{\n"), String::from("{\n"));
    for i in 0..n {
        let sep = if i + 1 == n { "" } else { "," };
        match i % 5 {
            0 => {
                let _ = writeln!(neutral, "  \"entry_{i}\": {{ \"type\": \"image\", \"value\": \"img/entry_{i}.png\" }}{sep}");
            }
            1 => {
                let _ = writeln!(neutral, "  \"entry_{i}\": {{ \"type\": \"color\", \"value\": \"#FF{:06X}\", \"comment\": \"Color {i}\" }}{sep}", i * 2654435 % 0xFFFFFF);
            }
            2 => {
                let _ = writeln!(neutral, "  \"entry_{i}\": {{ \"value\": \"Entry {i}: {{count}} things\", \"comment\": \"Text with a placeholder\" }}{sep}");
            }
            _ => {
                let _ = writeln!(neutral, "  \"entry_{i}\": \"Entry number {i}\"{sep}");
            }
        }
    }
    let texts: Vec<usize> = (0..n).filter(|i| i % 5 >= 2).collect();
    for (k, i) in texts.iter().enumerate() {
        let sep = if k + 1 == texts.len() { "" } else { "," };
        let _ = writeln!(ru, "  \"entry_{i}\": \"Запись номер {i}\"{sep}");
    }
    neutral.push('}');
    ru.push('}');
    (neutral, ru)
}

/// A `.yblock` — alternating a template model with a `.png` from
/// `resources/` and an element model with per-face UVs.
pub fn yblock(i: usize) -> String {
    let model = if i.is_multiple_of(2) {
        format!("{{ \"parent\": \"cube_all\", \"textures\": {{ \"all\": \"block/b{i}.png\" }} }}")
    } else {
        let face = |uv: &str| format!("{{ \"texture\": \"#0\", \"uv\": [{uv}] }}");
        format!(
            "{{ \"textures\": {{ \"0\": \"minecraft:block/stone\" }}, \"elements\": [{{ \"from\": [0, 0, 0], \"to\": [16, {h}, 16], \"faces\": {{ \"down\": {f}, \"up\": {f}, \"north\": {f}, \"south\": {f}, \"west\": {f}, \"east\": {f} }} }}] }}",
            h = 1 + i % 15,
            f = face("0, 0, 16, 16"),
        )
    };
    format!(
        "{{\n  \"name\": \"@Blocks.block_{i}\",\n  \"tooltip\": \"Block {i}\",\n  \"block\": {{ \"hardness\": {}, \"resistance\": 4, \"sound\": \"stone\", \"tool\": \"pickaxe\", \"requires_tool\": {} }},\n  \"model\": {model}\n}}\n",
        1 + i % 5,
        i.is_multiple_of(3)
    )
}

/// A `.yitem` — a sprite item, every third one food.
pub fn yitem(i: usize) -> String {
    let props = if i.is_multiple_of(3) {
        "\"food\": { \"nutrition\": 4, \"saturation\": 0.3 }".to_string()
    } else {
        format!("\"stack_size\": {}", 1 + i % 64)
    };
    format!(
        "{{\n  \"name\": \"@Blocks.item_{i}\",\n  \"item\": {{ {props} }},\n  \"model\": {{ \"parent\": \"generated\", \"textures\": {{ \"layer0\": \"item/i{i}.png\" }} }}\n}}\n"
    )
}

/// A mod project on disk (in a temp dir) with `n` blocks, `n` items, their
/// names in a `Blocks` set (+ `ru_ru`), an `n`-entry `Misc` set and every
/// texture they refer to (empty files — only their presence is checked).
pub fn mod_root(n: usize) -> PathBuf {
    let root = std::env::temp_dir().join(format!("yog-cli-bench-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let write = |rel: &str, text: &[u8]| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write("yog.toml", format!("[mod]\nid = \"{MOD_ID}\"\n").as_bytes());
    let (mut names, mut ru) = (String::from("{\n"), String::from("{\n"));
    for i in 0..n {
        let sep = if i + 1 == n { "" } else { "," };
        let _ = writeln!(
            names,
            "  \"block_{i}\": \"Block {i}\",\n  \"item_{i}\": \"Item {i}\"{sep}"
        );
        let _ = writeln!(
            ru,
            "  \"block_{i}\": \"Блок {i}\",\n  \"item_{i}\": \"Предмет {i}\"{sep}"
        );
        write(&format!("content/blocks/b{i}.yblock"), yblock(i).as_bytes());
        write(&format!("content/items/i{i}.yitem"), yitem(i).as_bytes());
        write(&format!("resources/block/b{i}.png"), b"");
        write(&format!("resources/item/i{i}.png"), b"");
        write(&format!("resources/img/entry_{i}.png"), b"");
    }
    names.push('}');
    ru.push('}');
    write("resources/Blocks.yres", names.as_bytes());
    write("resources/Blocks.ru_ru.yres", ru.as_bytes());
    let (misc, misc_ru) = yres_set(n);
    write("resources/Misc.yres", misc.as_bytes());
    write("resources/Misc.ru_ru.yres", misc_ru.as_bytes());
    root
}

/// A plain screen: `n` rows of a label and a button.
pub fn screen(n: usize) -> String {
    let mut s = String::from(
        "<Screen id=\"example:plain\" width=\"400\" height=\"300\">\n  <Column w=\"100%\" gap=\"2\">\n",
    );
    for i in 0..n {
        let _ = write!(
            s,
            "    <Row gap=\"4\" align=\"center\">\n      <Label text=\"Row {i}\" color=\"#FFCCCCCC\"/>\n      <Button id=\"b{i}\" text=\"Go {i}\" w=\"40\" h=\"14\"/>\n    </Row>\n"
        );
    }
    s.push_str("  </Column>\n</Screen>\n");
    s
}

/// A view-model screen: resources and styles, a `<Grid>`, `n` bound
/// controls (one-way, two-way, commands) and a list with an item template.
pub fn vm_screen(n: usize) -> String {
    let mut s = String::from(
        "<Screen id=\"example:vm\" width=\"400\" height=\"224\" anchor=\"center\" bg=\"#E0101010\" x:ViewModel=\"ExampleVm\">\n\
         \x20 <Screen.Resources>\n\
         \x20   <Color x:Key=\"Accent\" value=\"#FFD700\" />\n\
         \x20   <Style x:Key=\"Toggle\" target=\"ToggleButton\" w=\"58\" h=\"14\" bg=\"#FF333333\" hover:bg=\"#FF444444\" checked:bg=\"#FF555555\" disabled:color=\"#FF777777\" />\n\
         \x20 </Screen.Resources>\n\
         \x20 <Grid w=\"100%\" h=\"100%\" rows=\"20,*,Auto\" columns=\"*,*\">\n\
         \x20   <Label Grid.ColumnSpan=\"2\" text=\"{x:Bind title()}\" color=\"@Accent\" />\n\
         \x20   <ScrollView Grid.Row=\"1\" scroll_key=\"{x:Bind scope()}\">\n\
         \x20     <Column w=\"100%\" gap=\"2\">\n",
    );
    for i in 0..n {
        let _ = write!(
            s,
            "        <Row gap=\"3\">\n\
             \x20         <ToggleButton x:Name=\"toggle_{i}\" style=\"@Toggle\" text=\"Option {i}\" checked=\"{{x:Bind mode == {i}}}\" enabled=\"{{x:Bind enabled({i})}}\" command=\"{{x:Bind set_mode({i})}}\" />\n\
             \x20         <TextBox x:Name=\"field_{i}\" max_length=\"32\" text=\"{{x:Bind field_{i}, Mode=TwoWay}}\" />\n\
             \x20         <Label text=\"{{x:Bind label({i})}}\" color=\"{{x:Bind color({i})}}\" />\n\
             \x20       </Row>\n"
        );
    }
    s.push_str(
        "      </Column>\n\
         \x20   </ScrollView>\n\
         \x20   <ListView x:Name=\"list\" Grid.Row=\"1\" Grid.Column=\"1\" items=\"{x:Bind visible()}\" key=\"{x:Bind id}\" selected=\"{x:Bind selected_id(), BindBack=select}\" item_height=\"24\">\n\
         \x20     <ListView.ItemTemplate>\n\
         \x20       <DataTemplate x:DataType=\"Entry\">\n\
         \x20         <Column w=\"100%\" h=\"22\" hover:bg=\"#66333344\" selected:bg=\"#883A3A5A\">\n\
         \x20           <Label text=\"{x:Bind name}\" selected:color=\"#FFD700\" ellipsis=\"true\" />\n\
         \x20           <Label text=\"{x:Bind subtitle()}\" color=\"#AAAAAA\" />\n\
         \x20         </Column>\n\
         \x20       </DataTemplate>\n\
         \x20     </ListView.ItemTemplate>\n\
         \x20   </ListView>\n\
         \x20   <Button x:Name=\"done\" Grid.Row=\"2\" w=\"40\" h=\"14\" halign=\"right\" text=\"Done\" command=\"{x:Bind cx.close()}\" />\n\
         \x20 </Grid>\n\
         </Screen>\n",
    );
    s
}

/// A component with `n` properties of every type, bound in its markup.
pub fn component(n: usize) -> String {
    let mut s = String::from(
        "<Component name=\"Example\" description=\"An example\" icon=\"item:name_tag\" design_w=\"120\" design_h=\"16\">\n",
    );
    let types = [
        ("text", "\"x\""),
        ("number", "\"0.5\""),
        ("color", "\"#FF55C055\""),
        ("bool", "\"true\""),
    ];
    for i in 0..n {
        let (ty, default) = types[i % types.len()];
        let _ = writeln!(
            s,
            "  <Property name=\"p{i}\" type=\"{ty}\" default={default} doc=\"Property {i}\"/>"
        );
    }
    s.push_str("  <Column min_w=\"60\" gap=\"2\">\n");
    for i in 0..n {
        let attr = match i % 4 {
            0 => "text",
            1 => "w",
            2 => "color",
            _ => "visible",
        };
        let el = if i % 4 == 1 {
            "Spacer h=\"2\""
        } else {
            "Label text=\"x\""
        };
        let el = if attr == "text" {
            "Label".to_string()
        } else {
            el.to_string()
        };
        let _ = writeln!(s, "    <{el} {attr}=\"{{x:Bind p{i}}}\"/>");
    }
    s.push_str("  </Column>\n</Component>\n");
    s
}

pub fn remove(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
}
