//! Yog's domain over Substrate Data: tags, `.yres` entries, translation
//! keys, texts, images and colours — read from text, written back, and
//! resolved.

use yog_data::schema::{Resolve, Unresolved};
use yog_data::{
    Color, Id, Image, ItemIdOrTag, Kind, Path, Ref, RefKey, Res, Schema, TagId, Text, Value, from_text,
    from_value_with, kinds,
};

/// A recipe-like document: what each Yog type reads.
#[derive(Schema, Debug, PartialEq)]
struct Card {
    name: Text,
    icon: Image,
    tint: Option<Color>,
    ingredients: Vec<ItemIdOrTag>,
    #[data(default)]
    note: Option<Text>,
}

const CARD: &str = r##"{
    name: "@res:Strings.card.title",
    icon: "@image:item:minecraft:book",
    tint: "#8D6ACC",
    ingredients: ["@tag:item:minecraft:planks", "@item:minecraft:stick"],
    note: "@item:minecraft:stick",
}"##;

fn card() -> Card {
    from_text::<Card>(CARD).unwrap_or_else(|e| panic!("{e}")).value
}

#[test]
fn reads_yogs_typed_values() {
    let c = card();
    assert_eq!(
        c.name,
        Text::Res(Res {
            set: "Strings".into(),
            key: "card.title".into()
        })
    );
    assert_eq!(c.icon, Image::Source("item:minecraft:book".into()));
    assert_eq!(c.tint, Some(Color::Argb(0xFF8D_6ACC)), "#RRGGBB is opaque");
    assert_eq!(
        c.ingredients,
        [
            ItemIdOrTag::Tag(TagId::new("minecraft:planks")),
            ItemIdOrTag::One(Id::new("minecraft:stick")),
        ]
    );
    assert!(matches!(&c.note, Some(Text::NameOf(r)) if r.kind == kinds::Item::NAME));
}

#[test]
fn writes_back_what_it_read_in_every_form() {
    let c = card();
    let again: Card = from_text(&yog_data::to_text(&c)).unwrap_or_else(|e| panic!("{e}")).value;
    assert_eq!(again, c, "text");
    let bytes = yog_data::to_bytes(&c);
    assert_eq!(yog_data::read_bytes::<Card>(&bytes).ok(), Some(c), "binary");
}

#[test]
fn a_tag_of_another_kind_is_a_problem() {
    let wrong = r#"{ name: "x", icon: "@image:x", ingredients: ["@tag:block:minecraft:logs"] }"#;
    let errors = from_text::<Card>(wrong).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(errors.contains("tag of item"), "{errors}");
}

#[test]
fn plain_words_and_escaped_at_are_literal_texts() {
    let t: Text = yog_data::read_value(&Value::Str("@@everyone {b|hi}".into())).unwrap_or_default();
    assert_eq!(t, Text::Literal("@everyone {b|hi}".into()));
}

/// What a game resolves: names its texts by translation keys.
struct Game;

impl Resolve for Game {
    fn reference(&mut self, r: &Ref, _: &Path) -> Result<RefKey, Unresolved> {
        if r.id == "minecraft:nope" {
            return Err(Unresolved::Missing(r.clone()));
        }
        Ok(r.id.clone())
    }

    fn expand(&mut self, r: &Ref, _: &Path) -> Result<Option<Value>, Unresolved> {
        Ok(Text::of_ref(r.clone()).map(|t| {
            Value::Str(match t {
                Text::Res(res) => format!("gm.{}", res.key),
                Text::NameOf(r) => format!("{}.{}", r.kind, r.id.replace(':', ".")),
                Text::Lang(k) => k,
                Text::Literal(s) => s,
            })
        }))
    }
}

#[derive(Schema, Debug, PartialEq)]
struct Shown {
    #[data(with = "yog_data::TextOf")]
    title: String,
    #[data(with = "yog_data::TextOf")]
    name: String,
    #[data(with = "yog_data::TextOf")]
    done: String,
    #[data(with = "yog_data::TextOf")]
    plain: String,
}

#[test]
fn texts_resolve_through_the_resolvers_expand() {
    let doc: Value = from_text(
        r#"{ title: "@res:Strings.title", name: "@item:minecraft:stick", done: "@lang:gui.done", plain: "words" }"#,
    )
    .unwrap_or_else(|e| panic!("{e}"))
    .value;
    let shown = from_value_with::<Shown>(&doc, &mut Game).unwrap_or_else(|e| panic!("{e:?}")).value;
    assert_eq!(
        shown,
        Shown {
            title: "gm.title".into(),
            name: "item.minecraft.stick".into(),
            done: "gui.done".into(),
            plain: "words".into(),
        }
    );
    let kept = yog_data::read_value::<Shown>(&doc).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(kept.title, "@res:Strings.title", "no resolver: as written");
}

#[test]
fn what_a_read_met_splits_into_content_tags_and_entries() {
    let read = from_text::<Card>(CARD).unwrap_or_else(|e| panic!("{e}"));
    let tags = yog_data::found::tags(&read.refs);
    assert_eq!(tags.len(), 1);
    assert_eq!((tags[0].value.kind.as_str(), tags[0].value.id.as_str()), ("item", "minecraft:planks"));
    let entries = yog_data::found::resources(&read.refs);
    assert_eq!(entries.iter().map(|f| f.value.name()).collect::<Vec<_>>(), ["Strings.card.title"]);
    let content: Vec<_> = yog_data::found::content(&read.refs).map(|f| f.value.to_string()).collect();
    assert_eq!(content, ["@item:minecraft:stick", "@item:minecraft:stick"]);
}

#[test]
fn colours_read_both_ways_and_refuse_nonsense() {
    assert_eq!(Color::parse("#80FF0000"), Some(0x80FF_0000));
    assert_eq!(Color::parse("#FF0000"), Some(0xFFFF_0000));
    assert_eq!(Color::parse("#F00"), None);
    assert_eq!(Color::format(0x80FF_0000), "#80FF0000");
}
