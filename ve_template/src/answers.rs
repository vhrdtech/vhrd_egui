//! The answers an app was generated with and the `ve_app.json` record that keeps them (PLT-2).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Window layout of the generated app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// One `eframe::App`: top bar with the build info, central panel.
    Plain,
    /// The `ve_app` shell: menu bar, egui_tiles widget host, about / settings windows (WID-3).
    Shell,
}

/// eframe rendering backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Renderer {
    Wgpu,
    Glow,
}

/// Everything the templates are rendered with. Every field is an answer recorded in `ve_app.json`;
/// a new field needs a `#[serde(default)]` so older records still load, and `check-answers` then
/// shows it as new.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Answers {
    /// Crate and binary name, snake_case.
    pub name: String,
    /// Window class and storage id.
    pub app_id: String,
    /// Human name in the title bar and launcher.
    pub display_name: String,
    /// Cargo.toml author.
    pub author: String,
    #[serde(default = "default_layout")]
    pub layout: Layout,
    #[serde(default = "default_renderer")]
    pub renderer: Renderer,
    /// Use ve_theme (THM-1).
    #[serde(default = "default_true")]
    pub theme: bool,
    /// `git` or a path to a vhrd_egui checkout.
    #[serde(default = "default_vhrd_egui")]
    pub vhrd_egui: String,
}

fn default_layout() -> Layout {
    Layout::Plain
}
fn default_renderer() -> Renderer {
    Renderer::Wgpu
}
fn default_true() -> bool {
    true
}
fn default_vhrd_egui() -> String {
    "git".into()
}

impl Answers {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        app_id: Option<String>,
        display_name: Option<String>,
        author: String,
        layout: Layout,
        renderer: Renderer,
        theme: bool,
        vhrd_egui: String,
    ) -> Self {
        Self {
            app_id: app_id.unwrap_or_else(|| name.clone()),
            display_name: display_name.unwrap_or_else(|| display_name_of(&name)),
            name,
            author,
            layout,
            renderer,
            theme,
            vhrd_egui,
        }
    }

    /// Defaults for a hypothetical app, used to learn which answers exist today.
    fn reference() -> Self {
        Self::new(
            "example".into(),
            None,
            None,
            String::new(),
            Layout::Plain,
            Renderer::Wgpu,
            true,
            "git".into(),
        )
    }

    /// `my_tool_ui` -> `My Tool Ui`.
    fn as_map(&self) -> BTreeMap<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(m)) => m.into_iter().collect(),
            _ => BTreeMap::new(),
        }
    }
}

/// `my_tool_ui` -> `My Tool Ui`.
pub fn display_name_of(name: &str) -> String {
    name.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Which template an app came from.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateRef {
    pub name: String,
    pub version: String,
    pub commit: String,
    #[serde(default)]
    pub dirty: bool,
}

/// One applied upgrade, written by the agent that did it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Upgrade {
    pub from: String,
    pub to: String,
    pub date: String,
    #[serde(default)]
    pub notes: String,
}

/// `ve_app.json`: the template origin of an app and everything an upgrade needs to know.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub template: TemplateRef,
    pub answers: Answers,
    /// The app was not generated but brought onto the template by hand.
    #[serde(default)]
    pub adopted: bool,
    #[serde(default)]
    pub upgrades: Vec<Upgrade>,
    /// Template changes the owner does not want (free text per item), skipped by future upgrades.
    #[serde(default)]
    pub rejected: Vec<String>,
    /// Deliberate deviations from the template an upgrading agent must keep.
    #[serde(default)]
    pub nuances: Vec<String>,
}

impl Record {
    pub fn fresh(answers: Answers) -> Self {
        let commit = crate::TEMPLATE_COMMIT;
        Self {
            template: TemplateRef {
                name: crate::TEMPLATE_NAME.into(),
                version: crate::TEMPLATE_VERSION.into(),
                commit: commit.trim_end_matches("-dirty").into(),
                dirty: commit.ends_with("-dirty"),
            },
            answers,
            adopted: false,
            upgrades: Vec::new(),
            rejected: Vec::new(),
            nuances: Vec::new(),
        }
    }
}

/// Read an app's `ve_app.json`.
pub fn read(file: &Path) -> Result<Record, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))
}

/// `check-answers`: options the template has today that the record lacks (new since generation,
/// shown with their default) and recorded keys the template no longer knows.
pub fn check(file: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let raw: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let recorded: BTreeMap<String, Value> = raw
        .get("answers")
        .and_then(Value::as_object)
        .map(|m| m.clone().into_iter().collect())
        .ok_or("no \"answers\" object in the record")?;
    let template = raw.get("template");
    let version = template
        .and_then(|t| t.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("?");
    let commit = template
        .and_then(|t| t.get("commit"))
        .and_then(Value::as_str)
        .unwrap_or("?");
    println!(
        "generated with template {version} ({commit}); this ve_template is {} ({})",
        crate::TEMPLATE_VERSION,
        crate::TEMPLATE_COMMIT
    );
    let current = Answers::reference().as_map();
    let mut quiet = true;
    for (key, default) in &current {
        if !recorded.contains_key(key) {
            println!("new option: {key} (default {default}); ask the user, then add it to answers");
            quiet = false;
        }
    }
    for key in recorded.keys() {
        if !current.contains_key(key) {
            println!("unknown recorded option: {key} (removed or renamed; see the CHANGELOG)");
            quiet = false;
        }
    }
    if quiet {
        println!("answers are complete, no new options");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_from_crate_name() {
        assert_eq!(display_name_of("my_tool_ui"), "My Tool Ui");
        assert_eq!(display_name_of("dash"), "Dash");
        assert_eq!(display_name_of("a-b__c"), "A B C");
    }

    #[test]
    fn old_record_without_new_fields_loads() {
        let json = r#"{"template":{"name":"egui-app","version":"0.8.0","commit":"abc"},
            "answers":{"name":"x","app_id":"x","display_name":"X","author":"me"}}"#;
        let r: Record = serde_json::from_str(json).expect("loads");
        assert_eq!(r.answers.layout, Layout::Plain);
        assert!(r.answers.theme);
        assert_eq!(r.answers.vhrd_egui, "git");
        assert!(!r.adopted);
    }

    #[test]
    fn record_round_trips() {
        let r = Record::fresh(Answers::reference());
        let json = serde_json::to_string_pretty(&r).expect("serializes");
        let back: Record = serde_json::from_str(&json).expect("parses");
        assert_eq!(back.answers.name, "example");
        assert_eq!(back.template.name, "egui-app");
    }
}
