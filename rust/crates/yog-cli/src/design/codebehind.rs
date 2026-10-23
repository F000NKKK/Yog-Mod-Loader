//! Event handlers in a screen's code-behind — what double-clicking a button
//! in Visual Studio's designer does: `foo.yui`'s `on_click="play"` gets an
//! `events::PLAY => play(),` arm in the `match` over `click(event)` in
//! `foo.rs` and an empty `fn play()`, and the editor opens there. An
//! existing arm is just navigated to. A missing `foo.rs` is created with the
//! usual skeleton (`ui_include!`, `on_event`, `register`).

use std::path::{Path, PathBuf};

use syn::visit::Visit;

use crate::ui::yui::const_name;

/// Where the editor should go.
#[derive(Debug, PartialEq, yog_data::Schema)]
#[data(rename_all = "camelCase")]
pub(crate) struct HandlerLocation {
    pub(crate) path: String,
    /// 1-based.
    pub(crate) line: usize,
    /// 1-based.
    pub(crate) column: usize,
    /// The file was written (created or a handler added).
    pub(crate) changed: bool,
}

/// A Rust fn name for an event: `play`, `mymod:btn-play` → `mymod_btn_play`.
pub(crate) fn handler_name(event: &str) -> String {
    let name = const_name(event).to_lowercase();
    let name = name.trim_start_matches('_');
    if name.is_empty() {
        "on_click".to_string()
    } else {
        name.to_string()
    }
}

/// `foo.yui` → `foo.rs` beside it.
pub(crate) fn code_behind_path(yui: &Path) -> PathBuf {
    yui.with_extension("rs")
}

fn stem(yui: &Path) -> String {
    yui.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "screen".into())
}

fn skeleton(yui: &Path, constant: &str, handler: &str) -> String {
    let stem = stem(yui);
    format!(
        r#"//! Code-behind for `{stem}.yui` — the screen's widgets come from the
//! generated `build()`/`render()`/`click()`/`events`/`ids` (`yog build`).

yog_api::ui_include!("{stem}");

/// Handles one UI event of the screen; `true` = used.
fn on_event(event: &str) -> bool {{
    let Some(clicked) = click(event) else {{
        return false;
    }};
    match clicked.as_str() {{
        events::{constant} => {handler}(),
        _ => {{}}
    }}
    true
}}

fn {handler}() {{
}}

/// Registers the screen; open it with [`open`].
pub fn register(registry: &mut yog_api::Registry) {{
    registry.on_ui_render(ID, |gfx| render(gfx));
    registry.register_ui(ID, |_ui_id, event| {{
        on_event(event);
    }});
}}
"#
    )
}

/// The `match` over clicked events: one with an `events::` arm, else one
/// on `….as_str()` of something named like a click.
struct FindMatch {
    best: Option<(u8, MatchInfo)>,
    /// The enclosing top-level fn's last line while visiting.
    fn_end: usize,
}

#[derive(Clone)]
struct MatchInfo {
    /// Line/column (1-based / 0-based) of the first catch-all arm, if any.
    catch_all: Option<(usize, usize)>,
    /// Line of the closing `}` of the match, and its column.
    close: (usize, usize),
    /// Column of the first arm (indentation for a new one).
    arm_col: usize,
    fn_end: usize,
}

impl<'ast> Visit<'ast> for FindMatch {
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        let saved = self.fn_end;
        self.fn_end = f.block.brace_token.span.close().end().line;
        syn::visit::visit_item_fn(self, f);
        self.fn_end = saved;
    }

    fn visit_expr_match(&mut self, m: &'ast syn::ExprMatch) {
        let events_arm = m
            .arms
            .iter()
            .any(|a| pat_path(&a.pat).is_some_and(|p| p.starts_with("events::")));
        let scrutinee = quote_ish(&m.expr);
        let score = if events_arm {
            2
        } else if scrutinee.contains("click") || scrutinee.contains("event") {
            1
        } else {
            0
        };
        if score > 0 && self.best.as_ref().is_none_or(|(s, _)| *s < score) {
            let catch_all = m
                .arms
                .iter()
                .find(|a| matches!(&a.pat, syn::Pat::Wild(_) | syn::Pat::Ident(_)))
                .and_then(|a| pat_start(&a.pat));
            let arm_col = m
                .arms
                .first()
                .and_then(|a| pat_start(&a.pat))
                .map_or(m.match_token.span.start().column + 4, |(_, c)| c);
            let close = m.brace_token.span.close();
            self.best = Some((
                score,
                MatchInfo {
                    catch_all,
                    close: (close.start().line, close.start().column),
                    arm_col,
                    fn_end: self.fn_end,
                },
            ));
        }
        syn::visit::visit_expr_match(self, m);
    }
}

/// Line (1-based) and column (0-based) where an arm's pattern starts.
fn pat_start(p: &syn::Pat) -> Option<(usize, usize)> {
    let span = match p {
        syn::Pat::Path(p) => p.path.segments.first()?.ident.span(),
        syn::Pat::Ident(i) => i.ident.span(),
        syn::Pat::Wild(w) => w.underscore_token.span,
        syn::Pat::Lit(l) => match &l.lit {
            syn::Lit::Str(s) => s.span(),
            _ => return None,
        },
        _ => return None,
    };
    Some((span.start().line, span.start().column))
}

fn pat_path(p: &syn::Pat) -> Option<String> {
    match p {
        syn::Pat::Path(p) => Some(
            p.path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        ),
        _ => None,
    }
}

/// A rough source form of an expression — enough to look for names in it.
fn quote_ish(e: &syn::Expr) -> String {
    struct Names(String);
    impl<'ast> Visit<'ast> for Names {
        fn visit_ident(&mut self, i: &'ast proc_macro2::Ident) {
            self.0.push_str(&i.to_string());
            self.0.push(' ');
        }
    }
    let mut n = Names(String::new());
    n.visit_expr(e);
    n.0.to_lowercase()
}

/// 1-based line/column of the first `needle` in `src`.
fn find(src: &str, needle: &str) -> Option<(usize, usize)> {
    let at = src.find(needle)?;
    let line = src[..at].matches('\n').count() + 1;
    let col = at - src[..at].rfind('\n').map_or(0, |i| i + 1) + 1;
    Some((line, col))
}

/// Byte offset of the start of 1-based `line`.
fn line_start(src: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    src.match_indices('\n')
        .nth(line - 2)
        .map_or(src.len(), |(i, _)| i + 1)
}

/// Adds (or finds) the handler for `event` in `src`; `None` when there's no
/// event `match` to add an arm to. Returns the new text and the 1-based
/// line/column of the handler's body (or the existing arm).
pub(crate) fn add_handler(src: &str, event: &str) -> Result<(String, usize, usize, bool), String> {
    let constant = const_name(event);
    let arm_ref = format!("events::{constant}");
    let handler = handler_name(event);
    // Already handled: go to the handler fn if there is one, else the arm.
    if let Some(at) = src.find(&arm_ref).filter(|&at| {
        !src[at + arm_ref.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
    }) {
        let fn_def = format!("fn {handler}(");
        if let Some((line, col)) = find(src, &fn_def) {
            return Ok((src.to_string(), line + 1, col + 4, false));
        }
        let line = src[..at].matches('\n').count() + 1;
        let col = at - src[..at].rfind('\n').map_or(0, |i| i + 1) + 1;
        return Ok((src.to_string(), line, col, false));
    }
    let file = syn::parse_file(src).map_err(|e| format!("the code-behind doesn't parse: {e}"))?;
    let mut finder = FindMatch {
        best: None,
        fn_end: 0,
    };
    finder.visit_file(&file);
    let Some((_, m)) = finder.best else {
        return Err("no `match` over the clicked event (`match clicked.as_str() { … }`) to add the handler to".into());
    };
    let indent = " ".repeat(m.arm_col);
    let arm = format!("{indent}{arm_ref} => {handler}(),\n");
    let has_fn = src.contains(&format!("fn {handler}("));
    // The arm: before the catch-all, else just before the match's `}`.
    let arm_at = match m.catch_all {
        Some((line, _)) => line_start(src, line),
        None => line_start(src, m.close.0),
    };
    let mut edits = vec![(arm_at, arm)];
    if !has_fn {
        // The fn: after the fn holding the match (or at the end).
        let fn_at = if m.fn_end > 0 {
            line_start(src, m.fn_end + 1)
        } else {
            src.len()
        };
        let lead = if fn_at == src.len() && !src.ends_with('\n') {
            "\n\n"
        } else {
            "\n"
        };
        edits.push((fn_at, format!("{lead}fn {handler}() {{\n    \n}}\n")));
    }
    edits.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    let mut out = src.to_string();
    for (at, text) in edits {
        out.insert_str(at, &text);
    }
    let (line, _) = find(&out, &format!("fn {handler}(")).unwrap_or((0, 1));
    Ok((out, line + 1, 5, true))
}

/// Makes sure `yui`'s code-behind handles `event`, writing the file when
/// needed; where to put the caret.
pub(crate) fn ensure_handler(yui: &Path, event: &str) -> Result<HandlerLocation, String> {
    let rs = code_behind_path(yui);
    let path = rs.to_string_lossy().into_owned();
    if !rs.exists() {
        let handler = handler_name(event);
        let text = skeleton(yui, &const_name(event), &handler);
        std::fs::write(&rs, &text).map_err(|e| format!("{}: {e}", rs.display()))?;
        let (line, _) = find(&text, &format!("fn {handler}() {{")).unwrap_or((1, 1));
        return Ok(HandlerLocation {
            path,
            line: line + 1,
            column: 5,
            changed: true,
        });
    }
    let src = std::fs::read_to_string(&rs).map_err(|e| format!("{}: {e}", rs.display()))?;
    let (text, line, column, changed) = add_handler(&src, event)?;
    if changed {
        std::fs::write(&rs, &text).map_err(|e| format!("{}: {e}", rs.display()))?;
    }
    Ok(HandlerLocation {
        path,
        line,
        column,
        changed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "yog_api::ui_include!(\"menu\");

fn on_event(event: &str) -> bool {
    let Some(clicked) = click(event) else {
        return false;
    };
    match clicked.as_str() {
        events::DONE => close(),
        other => {
            let _ = other;
        }
    }
    true
}

fn close() {}
";

    #[test]
    fn names_follow_the_generator() {
        assert_eq!(const_name("filter_all"), "FILTER_ALL");
        assert_eq!(const_name("mymod:btn-play"), "MYMOD_BTN_PLAY");
        assert_eq!(handler_name("filterAll"), "filter_all");
        assert_eq!(handler_name("play_click"), "play_click");
    }

    #[test]
    fn adds_an_arm_before_the_catch_all_and_a_fn_after() {
        let (out, line, col, changed) = add_handler(SRC, "play_click").unwrap();
        assert!(changed);
        assert!(out.contains("        events::DONE => close(),\n        events::PLAY_CLICK => play_click(),\n        other => {"), "{out}");
        assert!(
            out.contains("    true\n}\n\nfn play_click() {\n    \n}\n\nfn close() {}"),
            "{out}"
        );
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[line - 2], "fn play_click() {");
        assert_eq!(col, 5);
        syn::parse_file(&out).unwrap();
    }

    #[test]
    fn an_existing_handler_is_found_not_duplicated() {
        let (out, line, _, changed) = add_handler(SRC, "done").unwrap();
        assert!(!changed);
        assert_eq!(out, SRC);
        assert_eq!(line, 8);
    }

    #[test]
    fn a_missing_code_behind_gets_the_skeleton() {
        let dir = std::env::temp_dir().join(format!("yog-idle-cb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let yui = dir.join("menu.yui");
        let loc = ensure_handler(&yui, "play").unwrap();
        let text = std::fs::read_to_string(dir.join("menu.rs")).unwrap();
        assert!(text.contains("events::PLAY => play(),"));
        assert_eq!(text.lines().nth(loc.line - 2), Some("fn play() {"));
        syn::parse_file(&text).unwrap();
        // Again: just found.
        assert!(!ensure_handler(&yui, "play").unwrap().changed);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
