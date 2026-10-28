//! The models' vocabulary for the IDE's TypeScript (`yog-formats-export`),
//! so the templates and render layers are written once.

use yog_codegen::{Header, Literals, Regenerate};
use yog_codegen_typescript::TypeScript;

use crate::content::vocab::{RENDER_LAYERS, TEMPLATES};

const HEADER: Header = Header {
    tool: "yog-formats",
    source: "Yog-Mod-Loader/rust/crates/yog-formats",
    regenerate: Regenerate::Command("cargo run -p yog-formats --bin yog-formats-export"),
};

/// A TypeScript module: the header, the render layers, the templates.
pub fn typescript() -> String {
    let mut out = format!(
        "{}\n/** How a block's model is drawn. */\nexport const RENDER_LAYERS = [{}] as const;\n\n\
         /** The model templates (`parent: \"cube_all\"`) and the texture slots each takes. */\n\
         export const TEMPLATES = [\n",
        HEADER.comment::<TypeScript>(),
        strings(&RENDER_LAYERS),
    );
    for t in TEMPLATES {
        out.push_str(&format!(
            "  {{ name: {}, slots: [{}], sprite: {} }},\n",
            TypeScript::string(t.name),
            strings(t.slots),
            t.sprite
        ));
    }
    out.push_str("] as const;\n");
    out
}

/// `items` as TypeScript string literals, comma-separated.
fn strings(items: &[&str]) -> String {
    items
        .iter()
        .map(|s| TypeScript::string(s))
        .collect::<Vec<_>>()
        .join(", ")
}
