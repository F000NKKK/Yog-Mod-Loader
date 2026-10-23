//! What generating a view-model screen keeps: the view's fields, its
//! parts, its event dispatch, its item templates; resolving resources and
//! styles into plain attributes.

use std::collections::HashMap;

use super::expr::Scope;
use crate::ui::resources::{Entry, Resources, local_name};
use crate::ui::yui::{self, Element, err};
use yog_data::Diagnostic;

/// Where generated update code writes: the view or a template instance.
pub(super) struct Ctx {
    pub(super) scope: Scope,
    /// `self.` or `inst.`
    pub(super) fields: &'static str,
    /// The tree variable: `root` or `w`.
    pub(super) tree: &'static str,
    /// List owner of actions (0 = screen).
    pub(super) owner: u16,
    /// Slot fields declared (name, type) — for templates.
    pub(super) slots: Vec<(String, String)>,
    pub(super) update: String,
}

/// What generated code calls the view's fields, a template instance's,
/// the tree in each.
pub(super) const VIEW_FIELDS: &str = "self.";
pub(super) const INSTANCE_FIELDS: &str = "inst.";
pub(super) const VIEW_TREE: &str = "root";
pub(super) const ITEM_TREE: &str = "w";
/// The screen's own actions' owner (a list's is its number).
pub(super) const SCREEN_OWNER: u16 = 0;

impl Ctx {
    pub(super) fn new(scope: Scope, fields: &'static str, tree: &'static str, owner: u16) -> Self {
        Ctx {
            scope,
            fields,
            tree,
            owner,
            slots: Vec::new(),
            update: String::new(),
        }
    }
}

/// Everything a view-model screen's code is made of so far.
pub(super) struct VmGen<'a> {
    pub(super) file: String,
    pub(super) mod_id: Option<&'a str>,
    /// `<Screen theme>`: element defaults under its own attributes and styles.
    pub(super) theme: Option<yog_ui::theme::Theme>,
    pub(super) resources: HashMap<String, String>,
    pub(super) styles: HashMap<String, Vec<(String, String)>>,
    pub(super) ids: yui::Names,
    /// View fields (name, type, init).
    pub(super) view_fields: Vec<(String, String, String)>,
    pub(super) parts: String,
    pub(super) dispatch: Vec<(String, String)>,
    pub(super) templates: String,
    pub(super) n_slots: usize,
    pub(super) n_lists: u16,
    pub(super) n_templates: usize,
    pub(super) n_scrollers: u16,
    pub(super) n_text_boxes: u16,
    /// Next command slot per owner.
    pub(super) cmd_slots: HashMap<u16, u16>,
}

/// `&[0, 2, 1]`: a widget's path in the tree.
pub(super) fn path_lit(path: &[u16]) -> String {
    format!(
        "&[{}]",
        path.iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

impl<'a> VmGen<'a> {
    pub(super) fn new(
        file: String,
        mod_id: Option<&'a str>,
        theme: Option<yog_ui::theme::Theme>,
    ) -> Self {
        VmGen {
            file,
            mod_id,
            theme,
            resources: HashMap::new(),
            styles: HashMap::new(),
            ids: yui::Names::default(),
            view_fields: Vec::new(),
            parts: String::new(),
            dispatch: Vec::new(),
            templates: String::new(),
            n_slots: 0,
            n_lists: 0,
            n_templates: 0,
            n_scrollers: 0,
            n_text_boxes: 0,
            cmd_slots: HashMap::new(),
        }
    }

    /// A new binding slot of type `ty`: a field of the view, or of the
    /// template instance being generated.
    pub(super) fn slot(&mut self, ctx: &mut Ctx, ty: &str) -> String {
        let name = format!("b{}", self.n_slots);
        self.n_slots += 1;
        if ctx.scope == Scope::Template && ctx.fields == INSTANCE_FIELDS {
            ctx.slots.push((name.clone(), ty.to_string()));
        } else {
            self.view_fields.push((
                name.clone(),
                format!("::yog_kit::bind::Slot<{ty}>"),
                "::yog_kit::bind::Slot::new()".into(),
            ));
        }
        name
    }

    /// The next command slot of `owner`'s actions.
    pub(super) fn next_cmd(&mut self, owner: u16) -> u16 {
        let n = self.cmd_slots.entry(owner).or_insert(0);
        let s = *n;
        *n += 1;
        s
    }

    /// An attribute's value with a `@local` resource resolved.
    fn resolved(&self, el: &Element, k: &str, v: &str) -> Result<String, Diagnostic> {
        let Some(name) = local_name(v) else {
            return Ok(v.to_string());
        };
        match self.resources.get(name) {
            Some(r) => Ok(r.clone()),
            None if k == yog_ids::yui_attr::STYLE => Ok(v.to_string()),
            None => err(
                el,
                format!("{k}=\"{v}\": no resource `{name}` in <Screen.Resources>"),
            ),
        }
    }

    /// Resolves `@local` resources, `x:Name` and `style` into plain attributes.
    pub(super) fn resolve(&self, el: &Element) -> Result<Element, Diagnostic> {
        let mut out = el.clone();
        out.attrs.clear();
        let mut style: Vec<(String, String)> = Vec::new();
        for (k, v) in &el.attrs {
            let v = self.resolved(el, k, v)?;
            match k.as_str() {
                yog_ids::yui_attr::X_NAME => out.attrs.push((yog_ids::yui_attr::ID.into(), v)),
                yog_ids::yui_attr::STYLE => {
                    let name = v.trim().trim_start_matches(yog_ids::sep::TYPED);
                    let Some(s) = self.styles.get(name) else {
                        return err(
                            el,
                            format!(
                                "style=\"{v}\": no <Style x:Key=\"{name}\"> in <Screen.Resources>"
                            ),
                        );
                    };
                    style = s.clone();
                }
                _ => out.attrs.push((k.clone(), v)),
            }
        }
        for (k, v) in style {
            if !out.attrs.iter().any(|(ek, _)| *ek == k) {
                out.attrs.push((k, v));
            }
        }
        // Theme defaults come last: own attributes and styles win.
        if let Some(theme) = &self.theme {
            theme.apply(&el.name, &mut out.attrs);
        }
        Ok(out)
    }

    /// The screen's `<Screen.Resources>`: its values and styles.
    pub(super) fn collect_resources(&mut self, res: &Element) -> Result<(), Diagnostic> {
        let entries: Vec<Entry> = res
            .children
            .iter()
            .map(|r| Entry {
                kind: &r.name,
                line: r.line,
                attrs: r
                    .attrs
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect(),
            })
            .collect();
        let read = Resources::read(&entries);
        if let Some(found) = read.problems.into_iter().next() {
            return Err(found);
        }
        self.resources = read.values;
        self.styles = read.styles;
        Ok(())
    }

    /// The ids of a tree generated apart (a template's), as the screen's.
    pub(super) fn absorb(&mut self, ids: Vec<String>) {
        for id in &ids {
            self.ids.push(id);
        }
    }
}
