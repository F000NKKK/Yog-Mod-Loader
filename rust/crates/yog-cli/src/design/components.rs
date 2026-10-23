//! A mod's `.yui` components (`src/**/*.yui` with a `<Component>` root —
//! Yog-IDLE's `docs/yui.md`, "Components"), read straight from their
//! markup: no build needed to list them in the toolbox, to edit one in the
//! designer, or to draw an instance whose bindings only name its own
//! properties (`text="{x:Bind label}"`) — the instance is expanded in place,
//! its values substituted. Components whose bindings call code
//! (`{x:Bind caption()}`) are *compiled*: the designer host draws those with
//! the mod's library ([`custom`](super::custom)).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use yog_ids::Format;
use yog_ids::prop_kind;
use yog_ids::yui_component as ca;
use yog_sync::{Arc, Mutex, OnceLock};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use super::host::{ComponentInfo, PropInfo};
use super::markup::YuiNode;

/// One component of the mod.
#[derive(Debug, Clone)]
pub(crate) struct Def {
    /// Name, doc, properties and the `.yui` it's declared in.
    pub(crate) info: ComponentInfo,
    /// The body element (`None`: none yet — drawn empty).
    pub(crate) body: Option<YuiNode>,
    /// Some binding calls code — only the mod's library can draw it.
    pub(crate) compiled: bool,
}

impl Def {
    pub(crate) fn find<'a>(defs: &'a [Def], name: &str) -> Option<&'a Def> {
        defs.iter().find(|d| d.info.name == name)
    }
}

const DECLARATIONS: &[&str] = &[yog_ids::yui::PROPERTY, yog_ids::yui::EVENT];

/// Joins an expanded instance's uid and its inner element's path: never in
/// a document's own uids, so the designer can tell the inside of an
/// instance apart (and select the instance instead).
pub(crate) const INNER: char = '/';

// ── Parsing ────────────────────────────────────────────────────────────────────

fn element(e: &BytesStart, uid: String) -> Result<YuiNode, String> {
    let kind = e.name().into_inner().to_string();
    let mut attrs = HashMap::new();
    for a in e.attributes() {
        let a = a.map_err(|e| e.to_string())?;
        let value = a
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|e| e.to_string())?;
        attrs.insert(a.key.into_inner().to_string(), value.into_owned());
    }
    Ok(YuiNode {
        uid,
        kind,
        attrs,
        children: Vec::new(),
    })
}

/// A `.yui` document's element tree, uids `c0`, `c0/1`, ...
pub(crate) fn parse_tree(text: &str) -> Result<YuiNode, String> {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<YuiNode> = Vec::new();
    let mut root = None;
    let uid = |stack: &[YuiNode]| match stack.last() {
        Some(parent) => format!("{}{INNER}{}", parent.uid, parent.children.len()),
        None => "c0".to_string(),
    };
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let node = element(&e, uid(&stack))?;
                stack.push(node);
            }
            Event::Empty(e) => {
                let node = element(&e, uid(&stack))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::End(_) => {
                let node = stack.pop().ok_or("unbalanced end tag")?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::Eof => break,
            _ => {}
        }
        if root.is_some() && stack.is_empty() {
            break;
        }
    }
    root.ok_or_else(|| "no root element".to_string())
}

/// `stat_bar` → `StatBar`.
fn pascal(stem: &str) -> String {
    stem.split(['_', '-'])
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// The component a `<Component>` tree declares (`None`: not a component —
/// a book's kind of page or a recipe layout isn't one to place on screens).
pub(crate) fn definition(root: &YuiNode, file: &Path) -> Option<Def> {
    if root.kind != yog_ids::yui::COMPONENT
        || root.attr("book_page").is_some()
        || root.attr(yog_ids::yui_attr::RECIPE_LAYOUT).is_some()
    {
        return None;
    }
    let name = root
        .attr(ca::NAME)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| pascal(&file.file_stem().unwrap_or_default().to_string_lossy()));
    let props: Vec<PropInfo> = root
        .children
        .iter()
        .filter(|c| DECLARATIONS.contains(&c.kind.as_str()))
        .map(|c| {
            let kind = if c.kind == yog_ids::yui::EVENT {
                prop_kind::EVENT.to_string()
            } else {
                c.attr(ca::TYPE)
                    .unwrap_or(prop_kind::TEXT)
                    .trim()
                    .to_string()
            };
            let options = crate::ui::component::options_of(c.attr(ca::OPTIONS));
            let default = c.attr(ca::DEFAULT).map_or_else(
                || crate::ui::component::default_of(&kind, &options),
                str::to_string,
            );
            PropInfo {
                name: c.attr(ca::NAME).unwrap_or_default().trim().to_string(),
                kind,
                default,
                doc: c.attr(ca::DOC).unwrap_or_default().to_string(),
                category: c.attr(ca::CATEGORY).unwrap_or_default().to_string(),
                options,
            }
        })
        .collect();
    let body = root
        .children
        .iter()
        .find(|c| !DECLARATIONS.contains(&c.kind.as_str()))
        .cloned();
    let names: Vec<&str> = props.iter().map(|p| p.name.as_str()).collect();
    let compiled = body.as_ref().is_some_and(|b| calls_code(b, &names));
    let text = |key: &str| root.attr(key).unwrap_or_default().trim().to_string();
    let size = |key: &str| {
        root.attr(key)
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|n| *n > 0.0)
            .unwrap_or(0.0)
    };
    let info = ComponentInfo {
        name,
        description: text(ca::DESCRIPTION),
        display_name: text(ca::DISPLAY_NAME),
        icon: text(ca::ICON),
        category: text(ca::CATEGORY),
        design_w: size(ca::DESIGN_W),
        design_h: size(ca::DESIGN_H),
        toolbox: root
            .attr(ca::TOOLBOX)
            .is_none_or(|v| v.trim() != yog_ids::yui_value::FALSE),
        props,
        file: Some(file.to_string_lossy().into_owned()),
    };
    Some(Def {
        info,
        body,
        compiled,
    })
}

// ── Bindings ───────────────────────────────────────────────────────────────────

/// The expression of a `{x:Bind expr[, options]}` value.
pub(crate) fn binding(value: &str) -> Option<&str> {
    let inner = value.trim().strip_prefix("{x:Bind")?.strip_suffix('}')?;
    let mut depth = 0i32;
    let mut quote = false;
    for (i, c) in inner.char_indices() {
        match c {
            '\'' => quote = !quote,
            '(' if !quote => depth += 1,
            ')' if !quote => depth -= 1,
            ',' if !quote && depth == 0 => return Some(inner[..i].trim()),
            _ => {}
        }
    }
    Some(inner.trim())
}

fn calls_code(node: &YuiNode, props: &[&str]) -> bool {
    node.attrs
        .values()
        .filter_map(|v| binding(v))
        .any(|e| !props.contains(&e))
        || node.children.iter().any(|c| calls_code(c, props))
}

/// `node` and its subtree with every binding to a property replaced by its
/// value in `values`, and design-time `d:attr` values applied. A binding
/// that calls code shows as `{expr}` in a text, and is dropped elsewhere.
pub(crate) fn bind_values(node: &YuiNode, values: &HashMap<String, String>) -> YuiNode {
    let mut attrs = HashMap::new();
    for (k, v) in &node.attrs {
        if k.starts_with("d:") {
            continue;
        }
        match binding(v) {
            None => {
                attrs.insert(k.clone(), v.clone());
            }
            Some(e) => {
                if let Some(value) = values.get(e) {
                    attrs.insert(k.clone(), value.clone());
                } else if matches!(k.as_str(), "text" | "placeholder") {
                    attrs.insert(k.clone(), format!("{{{e}}}"));
                }
            }
        }
    }
    for (k, v) in &node.attrs {
        if let Some(k) = k.strip_prefix("d:") {
            attrs.insert(k.to_string(), v.clone());
        }
    }
    YuiNode {
        uid: node.uid.clone(),
        kind: node.kind.clone(),
        attrs,
        children: node
            .children
            .iter()
            .map(|c| bind_values(c, values))
            .collect(),
    }
}

fn defaults(def: &Def) -> HashMap<String, String> {
    def.info
        .props
        .iter()
        .map(|p| (p.name.clone(), p.default.clone()))
        .collect()
}

fn with_uids(node: &mut YuiNode, uid: &str) {
    node.uid = uid.to_string();
    for (i, c) in node.children.iter_mut().enumerate() {
        with_uids(c, &format!("{uid}{INNER}{i}"));
    }
}

/// An instance of a markup-only component as the tree it stands for: the
/// body with the instance's property values, the instance's own attributes
/// (position, size, style — as `yog build` applies them to the built
/// widget) over the body root's, the instance's uid on the root and
/// `uid/i/j` inside.
pub(crate) fn expand(def: &Def, instance: &YuiNode) -> YuiNode {
    let mut values = defaults(def);
    let mut own = HashMap::new();
    for (k, v) in &instance.attrs {
        if def.info.props.iter().any(|p| &p.name == k) {
            values.insert(k.clone(), v.clone());
        } else {
            own.insert(k.clone(), v.clone());
        }
    }
    let Some(body) = &def.body else {
        return YuiNode {
            uid: instance.uid.clone(),
            kind: yog_ids::yui::SPACER.into(),
            attrs: own,
            children: Vec::new(),
        };
    };
    let mut tree = bind_values(body, &values);
    with_uids(&mut tree, &instance.uid);
    tree.attrs.extend(own);
    tree
}

/// The name a component being designed is checked under (it has no file of its own yet).
const COMPONENT_FILE: &str = "component";

/// A `<Component>` document as the designer draws it: a screen of its
/// design size holding the body, bound to the properties' defaults.
pub(crate) fn design_root(root: &YuiNode) -> YuiNode {
    let def = definition(root, Path::new(&Format::Ui.file(COMPONENT_FILE)));
    let values = def.as_ref().map(defaults).unwrap_or_default();
    // A recipe layout is its own size; a component's design size is a preview's.
    let layout = root.attr(yog_ids::yui_attr::RECIPE_LAYOUT).is_some();
    let size = |own: &str, design: &str, default: &'static str| {
        root.attr(if layout { own } else { design })
            .unwrap_or(default)
            .to_string()
    };
    let mut attrs = HashMap::from([
        ("width".to_string(), size("width", ca::DESIGN_W, "120")),
        ("height".to_string(), size("height", ca::DESIGN_H, "40")),
        ("anchor".to_string(), "center".to_string()),
    ]);
    if let Some(bg) = root.attr("d:bg") {
        attrs.insert(yog_ids::yui_attr::BG.into(), bg.to_string());
    }
    let children = root
        .children
        .iter()
        .filter(|c| !DECLARATIONS.contains(&c.kind.as_str()))
        .map(|c| bind_values(c, &values))
        .collect();
    YuiNode {
        uid: root.uid.clone(),
        kind: yog_ids::yui::SCREEN.into(),
        attrs,
        children,
    }
}

// ── The mod's components ─────────────────────────────────────────────────────────

fn yui_files(mod_root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = ignore::WalkBuilder::new(mod_root.join(yog_ids::project::SRC))
        .hidden(false)
        .build()
        .flatten()
        .map(|e| e.into_path())
        .filter(|p| p.is_file() && Format::Ui.is(p))
        .collect();
    out.sort();
    out
}

type Scanned = (u64, Arc<Vec<Def>>);

/// The mod's components, re-read when its `.yui` files change.
pub(crate) fn scan(mod_root: &Path) -> Arc<Vec<Def>> {
    scan_with_revision(mod_root).1
}

/// [`scan`] and a fingerprint of the `.yui` files it read.
pub(crate) fn scan_with_revision(mod_root: &Path) -> (u64, Arc<Vec<Def>>) {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Scanned>>> = OnceLock::new();
    let files = yui_files(mod_root);
    let fp = crate::files::fingerprint(&files);
    let cache = CACHE.get_or_init(Default::default);
    if let Some((seen, defs)) = cache.lock().get(mod_root)
        && *seen == fp
    {
        return (fp, defs.clone());
    }
    let defs: Vec<Def> = files
        .iter()
        .filter_map(|f| {
            let text = std::fs::read_to_string(f).ok()?;
            // Cheap check first: most .yui files are screens.
            if !text.contains("<Component") {
                return None;
            }
            definition(&parse_tree(&text).ok()?, f)
        })
        .collect();
    let defs = Arc::new(defs);
    cache
        .lock()
        .insert(mod_root.to_path_buf(), (fp, defs.clone()));
    (fp, defs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PILL: &str = r##"<?xml version="1.0"?>
<!-- a tag -->
<Component name="Pill" description="@Strings.pill" icon="item:name_tag" design_w="40" design_h="12" toolbox="false">
    <Property name="label" type="text" default="Yog" doc="Text" category="Data"/>
    <Property name="fill" type="color" default="#FF2D4A8A"/>
    <Event name="on_pick"/>
    <Row bg="{x:Bind fill}" padding="1" on_click="{x:Bind on_pick}" d:min_w="30">
        <Label text="{x:Bind label}" color="#FFFFFF"/>
    </Row>
</Component>"##;

    #[test]
    fn reads_a_component() {
        let tree = parse_tree(PILL).unwrap();
        let def = definition(&tree, Path::new("src/pill.yui")).unwrap();
        assert_eq!(def.info.name, "Pill");
        assert_eq!(def.info.props.len(), 3);
        assert_eq!(def.info.props[2].kind, "event");
        assert_eq!(def.info.props[0].category, "Data");
        assert_eq!(
            (def.info.description.as_str(), def.info.icon.as_str()),
            ("@Strings.pill", "item:name_tag")
        );
        assert_eq!(
            (def.info.design_w, def.info.design_h, def.info.toolbox),
            (40.0, 12.0, false)
        );
        assert!(!def.compiled);
        assert_eq!(def.body.as_ref().unwrap().children[0].uid, "c0/3/0");

        let inst = YuiNode {
            uid: "n7".into(),
            kind: "Pill".into(),
            attrs: HashMap::from([
                ("x".into(), "4".into()),
                ("label".into(), "Hi".into()),
                ("bg".into(), "#FF000000".into()),
            ]),
            children: vec![],
        };
        let t = expand(&def, &inst);
        assert_eq!(t.uid, "n7");
        assert_eq!(t.kind, "Row");
        assert_eq!(
            t.attr("bg"),
            Some("#FF000000"),
            "the instance's own attributes win"
        );
        assert_eq!(t.attr("x"), Some("4"));
        assert_eq!(t.attr("on_click"), Some(""));
        assert_eq!(t.attr("min_w"), Some("30"));
        assert_eq!(t.children[0].uid, "n7/0");
        assert_eq!(t.children[0].attr("text"), Some("Hi"));

        let screen = design_root(&tree);
        assert_eq!(
            (
                screen.kind.as_str(),
                screen.attr("width"),
                screen.children.len()
            ),
            ("Screen", Some("40"), 1)
        );
        assert_eq!(screen.children[0].attr("bg"), Some("#FF2D4A8A"));
        assert_eq!(screen.children[0].uid, "c0/3", "the document's uids stay");
    }

    #[test]
    fn code_bindings_make_a_component_compiled() {
        let tree = parse_tree(r##"<Component><Property name="v" type="number"/><Label text="{x:Bind caption(), Mode=OneTime}" d:color="#FF0000"/></Component>"##).unwrap();
        let def = definition(&tree, Path::new("src/stat_bar.yui")).unwrap();
        assert_eq!(def.info.name, "StatBar");
        assert!(def.compiled);
        let screen = design_root(&tree);
        assert_eq!(screen.children[0].attr("text"), Some("{caption()}"));
        assert_eq!(screen.children[0].attr("color"), Some("#FF0000"));
        assert!(
            definition(
                &parse_tree("<Screen id='a:b'/>").unwrap(),
                Path::new("a.yui")
            )
            .is_none()
        );
    }
}
