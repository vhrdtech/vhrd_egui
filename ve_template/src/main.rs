//! `ve_template`: the vhrd egui app template as a command (PLT-2, PLT-5).
//!
//! - `new NAME` generates an app from `egui_app_skill/templates/` (embedded at build time) and records the
//!   template version, commit and every answer in the app's `ve_app.json`, so an agent can upgrade it later.
//! - `new --answers APP/ve_app.json` regenerates with recorded answers (the upgrade flow).
//! - `check-answers APP/ve_app.json` lists options added or removed since the app was generated.
//! - `compare --current APP --new DIR [--base DIR]` reports per file what the template changed.
//! - `version` prints the template version and commit.
//!
//! How agents use it: `egui_app_skill/SKILL.md`.

mod answers;
mod compare;
mod generate;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use answers::{Answers, Layout, Renderer};

/// Template version: the vhrd_egui workspace version, bumped at landing.
pub const TEMPLATE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Short git SHA of the vhrd_egui checkout the binary was built from, `-dirty` with local changes.
pub const TEMPLATE_COMMIT: &str = match option_env!("GIT_SHA") {
    Some(s) => s,
    None => "unknown",
};
/// Name of the skill the template belongs to (`egui_app_skill/SKILL.md`).
pub const TEMPLATE_NAME: &str = "egui-app";

#[derive(Parser)]
#[command(
    version,
    about = "vhrd egui app template: generate, check and compare apps (egui_app_skill)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a new app in OUT/NAME, or regenerate one from recorded answers (--answers)
    New {
        /// Crate name, snake_case (also the binary name and the default app id)
        name: Option<String>,
        /// Directory the app folder is created in
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// Regenerate with the answers recorded in an app's ve_app.json (the upgrade flow); NAME is then ignored
        #[arg(long, conflicts_with = "name")]
        answers: Option<PathBuf>,
        /// Window class and storage id (matches the .desktop StartupWMClass) [default: NAME]
        #[arg(long)]
        app_id: Option<String>,
        /// Name shown in the title bar and the launcher [default: NAME with spaces and capitals]
        #[arg(long)]
        display_name: Option<String>,
        /// Author for Cargo.toml, "Name <mail>"
        #[arg(long, default_value = "Roman Isaikin <romix.lab@gmail.com>")]
        author: String,
        /// plain: one eframe::App with a top bar and a central panel; shell: the ve_app widget host (WID-3)
        #[arg(long, value_enum, default_value_t = Layout::Plain)]
        layout: Layout,
        /// eframe renderer feature
        #[arg(long, value_enum, default_value_t = Renderer::Wgpu)]
        renderer: Renderer,
        /// Leave ve_theme out: the app keeps egui's default look (rare; the theme is the stack's look)
        #[arg(long)]
        no_theme: bool,
        /// Where the vhrd_egui crates come from: `git` (the GitHub repo) or a path to a checkout
        #[arg(long, default_value = "git")]
        vhrd_egui: String,
        /// Overwrite files that already exist in OUT/NAME
        #[arg(long)]
        force: bool,
        /// List the files without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Options added or removed since an app was generated (reads its ve_app.json)
    CheckAnswers {
        /// The app's ve_app.json
        file: PathBuf,
    },
    /// Per-file report of what changed between an app and a freshly generated one
    Compare {
        /// The app as it is now
        #[arg(long)]
        current: PathBuf,
        /// A fresh generation with the current template (`new --answers`)
        #[arg(long)]
        new: PathBuf,
        /// A generation with the template version the app was made from (3-way: tells local changes apart)
        #[arg(long)]
        base: Option<PathBuf>,
        /// Print unified diffs for the changed files
        #[arg(long)]
        diff: bool,
    },
    /// Template version and commit
    Version,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::New {
            name,
            out,
            answers,
            app_id,
            display_name,
            author,
            layout,
            renderer,
            no_theme,
            vhrd_egui,
            force,
            dry_run,
        } => {
            let answers = match answers {
                Some(file) => answers::read(&file).map(|r| r.answers),
                None => match name {
                    Some(name) => Ok(Answers::new(
                        name,
                        app_id,
                        display_name,
                        author,
                        layout,
                        renderer,
                        !no_theme,
                        vhrd_egui,
                    )),
                    None => Err("give a NAME or --answers FILE".to_string()),
                },
            };
            answers.and_then(|a| generate::new(&a, &out, force, dry_run))
        }
        Cmd::CheckAnswers { file } => answers::check(&file),
        Cmd::Compare {
            current,
            new,
            base,
            diff,
        } => compare::run(&current, &new, base.as_deref(), diff),
        Cmd::Version => {
            println!(
                "ve_template {TEMPLATE_VERSION} ({TEMPLATE_COMMIT}, template {TEMPLATE_NAME})"
            );
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
