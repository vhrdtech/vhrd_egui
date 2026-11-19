//! The top bar's gallery page: the bar in dark and light with every setting at hand, a width to walk the
//! collapse ladder, the debug badge, the app's slots and the density.

use egui::{Theme, Ui, vec2};
use ve_basics::BuildInfo;
use ve_basics::actions::{Action, Actions};

use super::{
    BadgePlace, Brand, Desktop, FieldMode, Logo, MenuButton, MenuStyle, TopBar, TopBarSettings,
    WindowControls,
};
use crate::{Chrome, Density, Tokens, UiExt};

/// The build the gallery's bar shows.
const INFO: BuildInfo = BuildInfo {
    name: "eventfull",
    version: "0.12.11",
    git_sha: "dfd23ca",
    build_time: "18 Nov 2026 10:00",
    debug: false,
};

/// The narrowest and widest bar the width slider gives.
const WIDTH_MIN: f32 = 240.0;
const WIDTH_MAX: f32 = 1600.0;

/// State of the top bar page.
pub struct TopBarGallery {
    settings: TopBarSettings,
    desktop: Desktop,
    width: f32,
    debug: bool,
    slots: bool,
    logo: bool,
    density: Density,
    vertical: bool,
    actions: [Actions; 2],
    last: String,
}

impl Default for TopBarGallery {
    fn default() -> Self {
        Self {
            settings: TopBarSettings::default(),
            desktop: Desktop::Tiling,
            width: WIDTH_MAX,
            debug: false,
            slots: true,
            logo: true,
            density: Density::Comfortable,
            vertical: false,
            actions: [demo_actions(), demo_actions()],
            last: String::new(),
        }
    }
}

/// The width egui's buttons with these labels take side by side.
fn buttons_width(ui: &Ui, labels: &[&str]) -> f32 {
    let s = ui.spacing();
    let font = egui::TextStyle::Button.resolve(ui.style());
    labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).to_owned(), font.clone(), egui::Color32::WHITE)
                .size()
                .x
                + 2.0 * s.button_padding.x
        })
        .sum::<f32>()
        + s.item_spacing.x * labels.len() as f32
}

/// A few menus and two global actions, as an app declares them.
fn demo_actions() -> Actions {
    let list = [
        Action::new("file.open", "Open")
            .group("File")
            .keys(&["Ctrl+O"]),
        Action::new("file.save", "Save")
            .group("File")
            .keys(&["Ctrl+S"]),
        Action::new("edit.undo", "Undo")
            .group("Edit")
            .keys(&["Ctrl+Z"]),
        Action::new("edit.redo", "Redo")
            .group("Edit")
            .keys(&["Ctrl+Shift+Z"]),
        Action::new("view.tracks", "Tracks panel")
            .group("View")
            .keys(&["Ctrl+B"])
            .toggle(),
        Action::new("help.about", "About").group("Help"),
        Action::new("view.theme", "Theme")
            .icon("circle-half")
            .help("Switch dark and light"),
        Action::new("app.settings", "Settings").icon("gear-six"),
        super::palette_action(),
    ];
    let mut a = Actions::from_list(list);
    // spirit-level: allow(ui-disabled-without-reason) the Err is the reason the menu shows under the entry
    a.set_enabled(
        "edit.redo",
        Err("Nothing to redo: the last step was undone already".into()),
    );
    a.set_checked("view.tracks", true);
    a
}

/// One row of choices: `label`, then a selectable per value with its tooltip.
fn choice<T: PartialEq + Copy>(ui: &mut Ui, label: &str, value: &mut T, items: &[(T, &str, &str)]) {
    ui.horizontal(|ui| {
        ui.muted_label(label);
        for &(v, text, tip) in items {
            ui.selectable_value(value, v, text).on_hover_text(tip);
        }
    });
}

impl TopBarGallery {
    /// The settings, then the bar in dark and in light.
    pub fn ui(&mut self, ui: &mut Ui) {
        self.settings_ui(ui);
        let gap = ui.tokens().space.l;
        ui.add_space(gap);
        for (i, theme) in [Theme::Dark, Theme::Light].into_iter().enumerate() {
            ui.push_id(theme, |ui| {
                ui.set_style(ui.ctx().style_of(theme));
                let t = Tokens::of(ui.ctx(), theme);
                egui::Frame::new().fill(t.colors.bg).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let w = self.width.min(ui.available_width());
                    ui.allocate_ui(vec2(w, t.chrome.top_bar), |ui| {
                        ui.set_max_width(w);
                        self.bar(ui, i);
                    });
                    // Room for an open menu's first rows under the bar.
                    ui.add_space(t.chrome.row * 4.0);
                });
            });
        }
        if !self.last.is_empty() {
            ui.muted_label(format!("Last: {}", self.last));
        }
    }

    fn bar(&mut self, ui: &mut Ui, i: usize) {
        let info = BuildInfo {
            debug: self.debug,
            ..INFO
        };
        let logo = if self.logo {
            Logo::Placeholder("ef")
        } else {
            Logo::None
        };
        let brand = Brand::new("eventfull", &info)
            .logo(logo)
            .description("The gallery's sample app");
        let view_width = buttons_width(ui, &["Horizontal", "Vertical"]);
        let record_width = buttons_width(ui, &["Record"]);
        let vertical = &mut self.vertical;
        let mut bar = TopBar::new(&brand, &self.settings)
            .desktop(self.desktop)
            .menus(&mut self.actions[i])
            .document("bench-18nov2026.efp", true)
            .document_path("~/captures/bench")
            .globals(&["view.theme", "app.settings"]);
        if self.slots {
            bar = bar
                .left("View", view_width, |ui| {
                    ui.selectable_value(vertical, false, "Horizontal")
                        .on_hover_text("Tracks run left to right");
                    ui.selectable_value(vertical, true, "Vertical")
                        .on_hover_text("Tracks run top to bottom");
                })
                .right("Record", record_width, |ui| {
                    ui.primary_button("Record", "A slot item: the app's own button");
                });
        }
        bar.show(ui);
        for f in self.actions[i].take_fired() {
            self.last = format!("{} from {:?}", f.id, f.source);
        }
    }

    fn settings_ui(&mut self, ui: &mut Ui) {
        let s = &mut self.settings;
        choice(
            ui,
            "Menus (S1)",
            &mut s.menu_style,
            &[
                (
                    MenuStyle::RowAndField,
                    "Row and field",
                    "The menu row and the command field (the pick)",
                ),
                (
                    MenuStyle::Classic,
                    "Classic",
                    "A menu row, the document as a title, search as a button",
                ),
                (
                    MenuStyle::Button,
                    "Button",
                    "One menu button, the document as a breadcrumb",
                ),
            ],
        );
        choice(
            ui,
            "Collapsed menu (S23)",
            &mut s.menu_button,
            &[
                (
                    MenuButton::Name,
                    "Name",
                    "The app's name is the button (the pick)",
                ),
                (MenuButton::Mark, "Mark", "The logo is the button"),
                (MenuButton::Dots, "Dots", "A grid of dots"),
                (MenuButton::Word, "Menu", "The word Menu"),
                (MenuButton::Lines, "Lines", "Three lines"),
            ],
        );
        choice(
            ui,
            "Field (S13, S24)",
            &mut s.field,
            &[
                (
                    FieldMode::Document,
                    "Document",
                    "The document's name until clicked (the pick)",
                ),
                (
                    FieldMode::Search,
                    "Search",
                    "Always \"Search or run a command\"",
                ),
                (
                    FieldMode::None,
                    "None",
                    "No field; Ctrl K still opens the palette",
                ),
            ],
        );
        choice(
            ui,
            "Badge (S5)",
            &mut s.badge,
            &[
                (BadgePlace::Right, "Right", "At the right end (the pick)"),
                (
                    BadgePlace::ByName,
                    "By the name",
                    "Right after the app's name",
                ),
            ],
        );
        choice(
            ui,
            "Window controls (S4)",
            &mut s.window_controls,
            &[
                (WindowControls::Auto, "Auto", "From the desktop below"),
                (WindowControls::Ours, "Ours", "Ours, on the desktop's side"),
                (WindowControls::System, "System", "The system's title bar"),
                (WindowControls::None, "None", "No controls"),
            ],
        );
        choice(
            ui,
            "Desktop",
            &mut self.desktop,
            &[
                (
                    Desktop::Tiling,
                    "Tiling",
                    "Hyprland and other tiling window managers: no controls",
                ),
                (
                    Desktop::Stacking,
                    "Windows",
                    "Windows, GNOME, KDE: controls at the right",
                ),
                (Desktop::MacOs, "macOS", "Controls at the left, close first"),
            ],
        );
        let before = self.density;
        choice(
            ui,
            "Density (S3)",
            &mut self.density,
            &[
                (
                    Density::Comfortable,
                    "Comfortable",
                    "40 point bar (the pick)",
                ),
                (Density::Compact, "Compact", "32 point bar"),
            ],
        );
        if before != self.density {
            for theme in [Theme::Dark, Theme::Light] {
                let mut t = (*Tokens::of(ui.ctx(), theme)).clone();
                t.chrome = Chrome::for_density(self.density);
                t.apply(ui.ctx(), theme);
            }
        }
        ui.horizontal(|ui| {
            ui.muted_label("Width");
            ui.add(egui::Slider::new(&mut self.width, WIDTH_MIN..=WIDTH_MAX).suffix(" pt"))
                .on_hover_text("Narrow the bar to walk the collapse ladder (A2)");
            ui.checkbox(&mut self.debug, "Debug build")
                .on_hover_text("The badge as an orange pill");
            ui.checkbox(&mut self.slots, "Slots")
                .on_hover_text("The app's left and right slot items; they fold into the menu");
            ui.checkbox(&mut self.logo, "Logo")
                .on_hover_text("Without a logo the wordmark starts at the edge and never folds");
        });
    }
}
