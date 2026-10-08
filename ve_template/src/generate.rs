//! `new`: render the embedded templates into an app folder and write its `ve_app.json`.

use std::path::{Path, PathBuf};

use include_dir::{Dir, include_dir};
use minijinja::{Environment, Value, context};

use crate::answers::{Answers, Layout, Record};

/// The template files, embedded at build time. `*.j2` files are rendered, everything else is copied as is;
/// `__name__` in a path becomes the crate name.
static TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../egui_app_skill/templates");

/// Generate `out/<name>` from the answers.
pub fn new(answers: &Answers, out: &Path, force: bool, dry_run: bool) -> Result<(), String> {
    validate(answers)?;
    let root = out.join(&answers.name);
    if root.exists() && !force && !dry_run {
        return Err(format!(
            "{} exists; --force overwrites the template's files in it",
            root.display()
        ));
    }
    let env = environment(answers)?;
    let ctx = context_of(answers)?;
    let mut files = Vec::new();
    collect(&TEMPLATES, &mut files);
    files.sort_by(|a, b| a.path().cmp(b.path()));
    for file in &files {
        let rel = published_path(file.path(), &answers.name);
        let target = root.join(&rel);
        let content: Vec<u8> = if is_template(file.path()) {
            let src = std::str::from_utf8(file.contents())
                .map_err(|e| format!("{}: not UTF-8: {e}", file.path().display()))?;
            env.render_str(src, &ctx)
                .map_err(|e| format!("{}: {e}", file.path().display()))?
                .into_bytes()
        } else {
            file.contents().to_vec()
        };
        println!("{}", rel.display());
        if dry_run {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&target, content).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    let record = Record::fresh(answers.clone());
    let json = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())? + "\n";
    println!("ve_app.json");
    if !dry_run {
        std::fs::write(root.join("ve_app.json"), json).map_err(|e| e.to_string())?;
        println!();
        println!(
            "{} generated with template {} ({}).",
            answers.name,
            crate::TEMPLATE_VERSION,
            crate::TEMPLATE_COMMIT
        );
        println!(
            "Next: cd {} && git init && just test && UPDATE_SNAPSHOTS=1 cargo test   # first image snapshot",
            root.display()
        );
        println!(
            "Replace assets/icon.svg with the app's icon (`just icon` renders the PNG), commit ve_app.json with the app."
        );
    }
    Ok(())
}

fn validate(a: &Answers) -> Result<(), String> {
    let ok = !a.name.is_empty()
        && a.name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !a.name.starts_with(|c: char| c.is_ascii_digit());
    if !ok {
        return Err(format!(
            "name {:?}: use snake_case (lowercase letters, digits, underscores)",
            a.name
        ));
    }
    if a.layout == Layout::Shell {
        return Err(
            "--layout shell needs the ve_app crate (vhrd_egui WID-3), which has not landed yet; use plain"
                .into(),
        );
    }
    if a.vhrd_egui != "git" && !Path::new(&a.vhrd_egui).join("Cargo.toml").exists() {
        return Err(format!(
            "--vhrd-egui {}: not a vhrd_egui checkout (no Cargo.toml there); use `git` or a path",
            a.vhrd_egui
        ));
    }
    Ok(())
}

/// The Jinja environment: the `dep(crate)` function renders a Cargo dependency source for a vhrd_egui crate.
fn environment(answers: &Answers) -> Result<Environment<'static>, String> {
    let mut env = Environment::new();
    env.set_keep_trailing_newline(true);
    let source = answers.vhrd_egui.clone();
    env.add_function("dep", move |name: String| -> Value {
        if source == "git" {
            Value::from(format!("git = \"{VHRD_EGUI_GIT}\", package = \"{name}\""))
        } else {
            let abs = std::path::absolute(&source).unwrap_or_else(|_| PathBuf::from(&source));
            Value::from(format!("path = \"{}\"", abs.join(&name).display()))
        }
    });
    Ok(env)
}

/// Where the generated apps take the vhrd_egui crates from by default.
const VHRD_EGUI_GIT: &str = "ssh://git@github.com/vhrdtech/vhrd_egui.git";

fn context_of(a: &Answers) -> Result<Value, String> {
    let answers = serde_json::to_value(a).map_err(|e| e.to_string())?;
    let v = Value::from_serialize(&answers);
    Ok(context! {
        ..v,
        ..context! {
            template_version => crate::TEMPLATE_VERSION,
            template_commit => crate::TEMPLATE_COMMIT,
            renderer_feature => match a.renderer {
                crate::answers::Renderer::Wgpu => "wgpu",
                crate::answers::Renderer::Glow => "glow",
            },
            initial => a.display_name.chars().next().unwrap_or('A').to_uppercase().to_string(),
        }
    })
}

fn collect<'a>(dir: &'a Dir<'a>, out: &mut Vec<&'a include_dir::File<'a>>) {
    for f in dir.files() {
        out.push(f);
    }
    for d in dir.dirs() {
        collect(d, out);
    }
}

fn is_template(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "j2")
}

/// `deploy/__name__.desktop.j2` -> `deploy/<name>.desktop`.
fn published_path(p: &Path, name: &str) -> PathBuf {
    let s = p.to_string_lossy().replace("__name__", name);
    let s = s.strip_suffix(".j2").map(str::to_owned).unwrap_or(s);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_published_without_the_template_suffix() {
        assert_eq!(
            published_path(Path::new("deploy/__name__.desktop.j2"), "dash"),
            PathBuf::from("deploy/dash.desktop")
        );
        assert_eq!(
            published_path(Path::new("build.rs"), "dash"),
            PathBuf::from("build.rs")
        );
        assert_eq!(
            published_path(Path::new("src/main.rs.j2"), "dash"),
            PathBuf::from("src/main.rs")
        );
    }

    #[test]
    fn every_template_renders() {
        let a = Answers::new(
            "demo_app".into(),
            None,
            None,
            "Tester <t@example.com>".into(),
            Layout::Plain,
            crate::answers::Renderer::Wgpu,
            true,
            "git".into(),
        );
        let env = environment(&a).expect("env");
        let ctx = context_of(&a).expect("ctx");
        let mut files = Vec::new();
        collect(&TEMPLATES, &mut files);
        assert!(files.len() > 10, "templates embedded");
        for f in files {
            if is_template(f.path()) {
                let src = std::str::from_utf8(f.contents()).expect("utf8");
                let out = env
                    .render_str(src, &ctx)
                    .unwrap_or_else(|e| panic!("{}: {e}", f.path().display()));
                assert!(
                    !out.contains("{{"),
                    "{}: unrendered placeholder",
                    f.path().display()
                );
                assert!(
                    !out.contains("{%"),
                    "{}: unrendered block",
                    f.path().display()
                );
            }
        }
    }

    #[test]
    fn git_and_path_dependency_sources() {
        let a = Answers::new(
            "x".into(),
            None,
            None,
            String::new(),
            Layout::Plain,
            crate::answers::Renderer::Wgpu,
            true,
            "git".into(),
        );
        let env = environment(&a).expect("env");
        let s = env
            .render_str("{{ dep('ve_basics') }}", context! {})
            .expect("renders");
        assert_eq!(
            s,
            format!("git = \"{VHRD_EGUI_GIT}\", package = \"ve_basics\"")
        );
        let here = env!("CARGO_MANIFEST_DIR").to_string() + "/..";
        let a = Answers::new(
            "x".into(),
            None,
            None,
            String::new(),
            Layout::Plain,
            crate::answers::Renderer::Wgpu,
            true,
            here,
        );
        let env = environment(&a).expect("env");
        let s = env
            .render_str("{{ dep('ve_basics') }}", context! {})
            .expect("renders");
        assert!(
            s.starts_with("path = \"/") && s.ends_with("/ve_basics\""),
            "{s}"
        );
    }

    #[test]
    fn names_are_validated() {
        let mk = |n: &str| {
            Answers::new(
                n.into(),
                None,
                None,
                String::new(),
                Layout::Plain,
                crate::answers::Renderer::Wgpu,
                true,
                "git".into(),
            )
        };
        assert!(validate(&mk("good_name2")).is_ok());
        assert!(validate(&mk("Bad-Name")).is_err());
        assert!(validate(&mk("2start")).is_err());
        assert!(validate(&mk("")).is_err());
    }
}
