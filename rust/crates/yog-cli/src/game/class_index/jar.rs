//! The class files of a game jar. A server jar since 1.18 is a bundler:
//! the server itself is a jar inside it, named by its `versions.list`.

use std::io::Read;

/// The bundler's list (`<hash>\t<id>\t<path>` lines) and where the paths are.
const VERSIONS_LIST: &str = "META-INF/versions.list";
const VERSIONS_DIR: &str = "META-INF/versions/";
/// What separates a list line's fields, and where the path is.
const FIELD_SEP: char = '\t';
const PATH_FIELD: usize = 2;
/// A class file's extension.
const CLASS_EXT: &str = ".class";

fn archive(bytes: Vec<u8>) -> Result<zip::ZipArchive<std::io::Cursor<Vec<u8>>>, String> {
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("a game jar: {e}"))
}

fn entry(zip: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>, name: &str) -> Option<Vec<u8>> {
    let mut file = zip.by_name(name).ok()?;
    let mut out = Vec::new();
    file.read_to_end(&mut out).ok()?;
    Some(out)
}

/// The class files of the jar `bytes` (the jar a bundler carries, if it's one).
pub(super) fn game_classes(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut zip = archive(bytes.to_vec())?;
    let inner = entry(&mut zip, VERSIONS_LIST).and_then(|list| {
        let list = String::from_utf8_lossy(&list).into_owned();
        let path = list
            .lines()
            .next()?
            .split(FIELD_SEP)
            .nth(PATH_FIELD)?
            .to_owned();
        entry(&mut zip, &format!("{VERSIONS_DIR}{path}"))
    });
    if let Some(inner) = inner {
        zip = archive(inner)?;
    }
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| Some(zip.by_index(i).ok()?.name().ok()?.into_owned()))
        .filter(|n| n.ends_with(CLASS_EXT))
        .collect();
    Ok(names.iter().filter_map(|n| entry(&mut zip, n)).collect())
}
