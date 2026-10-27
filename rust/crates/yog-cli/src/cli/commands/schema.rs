//! `yog schema <format>`: the format's shape (`yog_data::Shape::describe`)
//! as JSON — what editors and docs are generated from.

pub(super) const WORDS: &[&str] = &["schema"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let all = || yog_formats::names().collect::<Vec<_>>().join(", ");
    let name = rest
        .first()
        .ok_or_else(|| format!("usage: yog schema <format> — {}", all()))?;
    let shape =
        yog_formats::shape_of(name).ok_or_else(|| format!("no format `{name}` — {}", all()))?;
    println!(
        "{}",
        yog_data::text::to_string_pretty(&yog_data::Schema::write(&shape.describe()))
    );
    Ok(())
}
