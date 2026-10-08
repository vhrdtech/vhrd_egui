//! `compare`: per-file report between the app as it is and a fresh generation, optionally three-way with a
//! generation from the template version the app was made from.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use similar::TextDiff;
use walkdir::WalkDir;

/// Folders never compared.
const SKIP: &[&str] = &["target", ".git", ".idea"];
/// Files whose differences are expected.
const NOISE: &[&str] = &["Cargo.lock", "ve_app.json"];

pub fn run(current: &Path, new: &Path, base: Option<&Path>, diff: bool) -> Result<(), String> {
    for d in [Some(current), Some(new), base].into_iter().flatten() {
        if !d.is_dir() {
            return Err(format!("{}: not a directory", d.display()));
        }
    }
    let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
    paths.extend(list(new)?);
    if let Some(b) = base {
        paths.extend(list(b)?);
    }
    // Only the template's files: app files that no template version has are the app's own business.
    let mut counts = (0usize, 0usize, 0usize, 0usize);
    for rel in &paths {
        let cur = read(&current.join(rel));
        let nw = read(&new.join(rel));
        let bs = base.map(|b| read(&b.join(rel)));
        let name = rel.display();
        let noise = rel
            .file_name()
            .is_some_and(|f| NOISE.iter().any(|n| f == *n));
        let verdict = match (&cur, &nw, &bs) {
            (_, _, _) if noise => "noise   ",
            (None, Some(_), _) => "add     ",
            (Some(_), None, Some(Some(_))) => "dropped ",
            (Some(_), None, _) => "app-only",
            (Some(c), Some(n), _) if c == n => "same    ",
            (Some(c), Some(_), Some(Some(b))) if c == b => "update  ",
            (Some(_), Some(n), Some(Some(b))) if n == b => "local   ",
            (Some(_), Some(_), _) => "merge   ",
            (None, None, _) => continue,
        };
        match verdict.trim() {
            "same" | "noise" => counts.0 += 1,
            "update" | "add" => counts.1 += 1,
            "local" | "app-only" => counts.2 += 1,
            _ => counts.3 += 1,
        }
        println!("{verdict} {name}");
        if diff
            && matches!(verdict.trim(), "update" | "merge")
            && let (Some(c), Some(n)) = (&cur, &nw)
        {
            let text = TextDiff::from_lines(c, n);
            print!(
                "{}",
                text.unified_diff()
                    .context_radius(3)
                    .header(&format!("current/{name}"), &format!("new/{name}"))
            );
        }
    }
    println!();
    println!(
        "{} same, {} to take (update/add), {} to keep (local/app-only), {} to merge by meaning",
        counts.0, counts.1, counts.2, counts.3
    );
    println!(
        "update = the app never changed it, take the new file · local = only the app changed it, keep · merge = both changed, read the diff · dropped = the template no longer ships it, decide"
    );
    Ok(())
}

fn list(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_str().is_some_and(|n| SKIP.contains(&n)))
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_path_buf();
            out.push(rel);
        }
    }
    Ok(out)
}

/// File content as text; binary files compare through a lossy conversion, which is enough for same/changed.
fn read(p: &Path) -> Option<String> {
    std::fs::read(p)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_files_without_skipped_dirs() {
        let dir = std::env::temp_dir().join(format!("ve_template-cmp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).expect("mkdir");
        std::fs::create_dir_all(dir.join("target")).expect("mkdir");
        std::fs::write(dir.join("src/main.rs"), "fn main() {}").expect("write");
        std::fs::write(dir.join("target/x"), "junk").expect("write");
        let files = list(&dir).expect("list");
        assert_eq!(files, vec![PathBuf::from("src/main.rs")]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
