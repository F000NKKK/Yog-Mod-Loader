//! The names hold together, and the generated copies are current.

use super::*;

#[test]
fn ids_are_composed_from_their_namespace() {
    assert_eq!(yog::STATE, "yog:state");
    assert_eq!(game::OVERWORLD, "minecraft:overworld");
    assert_eq!(image_scheme::ITEM, kind::ITEM);
    assert_eq!(split("stone"), ("minecraft", "stone"));
    assert_eq!(qualify("mymod:x"), "mymod:x");
    assert_eq!(Format::of(".yblock"), Some(Format::Block));
    assert_eq!(registry_of(kind::BIOME), "worldgen/biome");
    assert_eq!(Format::Anim.file("walk"), "walk.yanim");
}

/// The hosts' copy (`java-common`), when this crate is in its repository.
#[test]
fn the_java_names_are_current() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../java-common/src/main/java/dev/yog/YogNames.java");
    let Ok(java) = std::fs::read_to_string(&file) else {
        return;
    };
    assert!(
        java == export::java(export::JAVA_PACKAGE, export::JAVA_CLASS),
        "{} is stale: cargo run -p yog-ids --bin yog-ids-export -- --java ../java-common/src/main/java/dev/yog dev.yog YogNames",
        file.display()
    );
}

#[test]
fn the_exports_name_every_group() {
    let ts = export::typescript();
    assert!(
        ts.contains("export const imageScheme = {") && ts.contains("  YOG: \"yog\","),
        "{ts}"
    );
    assert!(
        ts.contains("  block: { ext: \"yblock\", noun: \"block\", isData: true },"),
        "{ts}"
    );
    let java = export::java("dev.yog.generated", "YogNames");
    assert!(
        java.contains("public static final class ImageScheme {")
            && java.contains("public static final String STATE = \"yog:state\";"),
        "{java}"
    );
}

#[test]
fn a_font_alias_stands_for_the_games_font() {
    assert_eq!(
        game_font_of(&font_alias(game_font::UNIFORM)).as_deref(),
        Some("minecraft:uniform")
    );
    assert_eq!(
        game_font_of(yog::DEFAULT_FONT).as_deref(),
        Some(game::DEFAULT_FONT)
    );
    assert_eq!(game_font_of("minecraft:uniform"), None);
    assert_eq!(game_font_of(yog::FONT_ALIAS), None);
}
