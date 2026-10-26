//! Where `.yui` files and their generated code are: finding them, naming
//! them, generating every one of a project.

use std::path::{Path, PathBuf};

use super::{generate_with, parse_xml};
use yog_data::Diagnostic;

/// Where generated screen code goes, under the project root.
pub const GENERATED_DIR: &str = yog_ids::project::GENERATED;

/// `path` from the current folder when it's relative.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

/// The project root (the folder with `yog.toml`) a `.yui` file belongs to.
pub fn project_root(yui: &Path) -> Option<PathBuf> {
    absolute(yui)
        .ancestors()
        .skip(1)
        .find(|d| d.join(crate::project::manifest::MANIFEST).is_file())
        .map(Path::to_path_buf)
}

/// `foo.yui`'s name for `ui_include!`: its path under `src/` without the
/// extension, `/`-separated (`mod_list`, `screens/options`).
pub fn include_name(root: &Path, yui: &Path) -> String {
    let abs = absolute(yui);
    let rel = abs
        .strip_prefix(root.join(yog_ids::project::SRC))
        .or_else(|_| abs.strip_prefix(root))
        .unwrap_or(&abs)
        .with_extension("");
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(yog_ids::sep::PATH)
}

/// Where `src/<name>.yui`'s generated code goes:
/// `obj/generated/<name>.g.rs`.
pub fn generated_path(root: &Path, name: &str) -> PathBuf {
    root.join(GENERATED_DIR)
        .join(format!("{name}{GENERATED_EXT}"))
}

/// Rust files, and what a generated one's name ends with.
const RS: &str = "rs";
const GENERATED_EXT: &str = ".g.rs";

/// Generates `obj/generated/<name>.g.rs` for one `.yui` file of the
/// project at `root` — only rewritten when the output changed, so cargo
/// doesn't rebuild for nothing. Its path, and what to warn of.
pub fn generate_file(root: &Path, yui: &Path) -> Result<(PathBuf, Vec<Diagnostic>), Diagnostic> {
    let shown = crate::shown_path(root, yui);
    let screen = |d: Diagnostic| d.in_file(&shown).or_code(yog_ids::build_problem::SCREEN);
    let src = std::fs::read_to_string(yui).map_err(|e| screen(e.to_string().into()))?;
    let name = include_name(root, yui);
    let mod_id = std::fs::read_to_string(root.join(crate::project::manifest::MANIFEST))
        .ok()
        .and_then(|t| crate::project::manifest::parse_yog_toml(&t).ok())
        .map(|t| t.id);
    // The mod's recipe layouts, for the screen's <RecipeArea>s.
    let layouts: Vec<yog_formats::book::layout::RecipeLayout> = if src.contains(RECIPE_AREA_TAG) {
        crate::ui::book_page::find_layouts(
            root,
            mod_id.as_deref().unwrap_or(yog_ids::ns::MINECRAFT),
            &mut Vec::new(),
        )
        .into_iter()
        .map(|(_, l)| l)
        .collect()
    } else {
        Vec::new()
    };
    let (code, warnings) =
        generate_with(&src, &name, mod_id.as_deref(), &layouts).map_err(screen)?;
    let out = generated_path(root, &name);
    crate::write_if_changed(&out, &code)?;
    Ok((out, warnings.into_iter().map(screen).collect()))
}

/// What opens a `<RecipeArea>`: a screen with one needs the mod's layouts.
const RECIPE_AREA_TAG: &str = yog_ids::concatcp!("<", yog_ids::yui::RECIPE_AREA);

/// Whether `path`'s root is a `<Component book_page="…">`.
pub fn is_book_page_file(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| parse_xml(&s).ok())
        .is_some_and(|r| crate::ui::book_page::is_book_ui(&r))
}

/// Every `.yui` file under `dir`, recursively.
pub fn find_yui_files(dir: &Path) -> Vec<PathBuf> {
    yog_analysis::files::with_ext(dir, &[yog_ids::Format::Ui.ext()])
}

/// What [`generate_all`] did: how many screens it read, what to warn of.
pub struct Generated {
    /// How many screens it read.
    pub screens: usize,
    /// What to warn of.
    pub warnings: Vec<Diagnostic>,
}

/// `yog build`'s step: regenerates every `src/**/*.yui`'s `.g.rs` and drops
/// generated files whose `.yui` is gone.
pub fn generate_all(root: &Path) -> Result<Generated, Diagnostic> {
    let files = find_yui_files(&root.join(yog_ids::project::SRC));
    let mut wanted = Vec::new();
    let mut warnings = Vec::new();
    for file in &files {
        // A kind of book page is the book's (`crate::ui::book_page`), not a widget.
        if is_book_page_file(file) {
            continue;
        }
        let (out, w) = generate_file(root, file)?;
        wanted.push(out);
        warnings.extend(w);
    }
    // Components without a code-behind, for `ui_components!()`.
    let (index, warning) = crate::ui::component::write_index(root)?;
    wanted.push(index);
    warnings.extend(warning.map(|w| Diagnostic::warning(yog_ids::build_problem::SCREEN, w)));
    // `.yres` accessors live there too (`crate::yres` cleans those up).
    let res_index = root.join(crate::yres::GENERATED_FILE);
    let res_dir = root.join(crate::yres::GENERATED_DIR);
    // So do `content/` blocks and items (`crate::content`).
    let content_file = root.join(crate::content::GENERATED_FILE);
    let content_parts = content_file
        .parent()
        .map(|d| d.join(crate::content::GENERATED_PARTS_DIR))
        .unwrap_or_default();
    for stale in yog_analysis::files::with_ext(&root.join(GENERATED_DIR), &[RS]) {
        if stale.to_string_lossy().ends_with(GENERATED_EXT)
            && !wanted.contains(&stale)
            && stale != res_index
            && stale != content_file
            && stale != crate::project::script_lib::root_file(root)
            && !stale.starts_with(&content_parts)
            && !stale.starts_with(&res_dir)
        {
            crate::files::remove(&stale);
        }
    }
    Ok(Generated {
        screens: files.len(),
        warnings,
    })
}
