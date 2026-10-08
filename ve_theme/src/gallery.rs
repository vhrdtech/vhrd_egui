//! Every egui widget and every [`UiExt`] helper in one view (THM-5): the tool for tuning tokens, and the
//! content of the `gallery_dark.png` / `gallery_light.png` snapshots.
//!
//! `cargo run -p ve_theme --example gallery` shows dark and light side by side. Apps can also put
//! [`Gallery::ui`] in a debug window to check their own token overrides.

use egui::{Color32, Frame, Margin, RichText, Theme, Ui};

use crate::{Tokens, Tone, UiExt};

/// Widget state for the gallery (deterministic, so snapshots are stable).
#[derive(Clone, Debug)]
pub struct Gallery {
    checked: bool,
    radio: u8,
    slider: f32,
    drag: i32,
    text: String,
    combo: usize,
    toggle: bool,
    selected: usize,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            checked: true,
            radio: 1,
            slider: 63.0,
            drag: 42,
            text: "omarchy-m1".to_owned(),
            combo: 0,
            toggle: true,
            selected: 1,
        }
    }
}

const COMBO: [&str; 3] = ["CAN FD", "USB", "Analog"];

impl Gallery {
    /// Dark and light next to each other, each on its own background.
    pub fn side_by_side(&mut self, ui: &mut Ui) {
        ui.columns(2, |cols| {
            for (col, theme) in cols.iter_mut().zip([Theme::Dark, Theme::Light]) {
                col.push_id(theme, |ui| self.themed(ui, theme));
            }
        });
    }

    /// The gallery in one theme, on that theme's background, whatever the surrounding `Ui` uses.
    pub fn themed(&mut self, ui: &mut Ui, theme: Theme) {
        ui.set_style(ui.ctx().style_of(theme));
        let t = Tokens::of(ui.ctx(), theme);
        Frame::new()
            .fill(t.colors.bg)
            .inner_margin(Margin::same(t.space.l as i8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                self.ui(ui);
            });
    }

    /// Every widget in the `Ui`'s current theme.
    pub fn ui(&mut self, ui: &mut Ui) {
        let t = ui.tokens();
        let mode = if ui.visuals().dark_mode {
            "dark"
        } else {
            "light"
        };

        ui.panel_title_bar(
            format!("ve_theme · {mode}"),
            "Panel title bar: the panel's name on the left, its actions on the right",
            |ui| {
                ui.badge(
                    "v0.8",
                    Tone::Neutral,
                    "Badge, neutral: versions, variants, counts",
                );
            },
        );

        ui.section_header("Text", "Text styles of the type scale (THM-3)");
        ui.heading("Heading 20 / 26");
        ui.label(
            RichText::new("Title 15 / 20")
                .font(t.type_scale.title.font_id())
                .color(t.colors.title),
        );
        ui.label("Body 13 / 18: labels and running text");
        ui.horizontal(|ui| {
            ui.label(RichText::new("strong").strong());
            ui.label(RichText::new("weak").weak());
            ui.muted_label("muted label");
            ui.label(RichText::new("caption 11").font(t.type_scale.caption.font_id()));
            ui.label(RichText::new("small 10.5").small());
        });
        ui.horizontal(|ui| {
            ui.monospace("mono 0x1f · 12.5");
            ui.hyperlink_to("hyperlink", "https://vhrd.tech");
        });
        ui.horizontal(|ui| {
            ui.colored_label(ui.visuals().warn_fg_color, "warning text");
            ui.colored_label(ui.visuals().error_fg_color, "error text");
            ui.code("code");
        });

        ui.section_header(
            "Buttons",
            "Plain egui buttons and the UiExt buttons (THM-4)",
        );
        ui.horizontal(|ui| {
            ui.primary_button("Save", "Primary button: the one main action of a view");
            ui.button("Cancel").on_hover_text("Plain egui button");
            ui.danger_button("Delete", "Danger button: destroys something; say what");
            ui.add_enabled(false, egui::Button::new("Disabled"));
        });
        ui.horizontal(|ui| {
            for (i, name) in ["Overview", "Signals", "Log"].into_iter().enumerate() {
                if ui.selectable_label(self.selected == i, name).clicked() {
                    self.selected = i;
                }
            }
            ui.toggle_value(&mut self.toggle, "Toggle");
            ui.small_button("small").on_hover_text("Small button");
        });
        ui.toolbar(|ui| {
            ui.button("Run").on_hover_text("Toolbar button");
            ui.button("Stop").on_hover_text("Toolbar button");
            ui.separator();
            ui.button("Reset").on_hover_text("Toolbar button");
        });

        ui.section_header(
            "Inputs",
            "Check boxes, radio buttons, sliders, text edits, combo boxes",
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.checked, "Check box");
            ui.radio_value(&mut self.radio, 0, "Radio A");
            ui.radio_value(&mut self.radio, 1, "Radio B");
        });
        ui.add(egui::Slider::new(&mut self.slider, 0.0..=100.0).suffix(" %"));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.drag).suffix(" ms"));
            ui.add(egui::TextEdit::singleline(&mut self.text).desired_width(120.0));
            egui::ComboBox::from_id_salt("combo")
                .selected_text(COMBO[self.combo])
                .show_ui(ui, |ui| {
                    for (i, name) in COMBO.into_iter().enumerate() {
                        ui.selectable_value(&mut self.combo, i, name);
                    }
                });
        });
        ui.add(egui::ProgressBar::new(0.63).text("63 %"));

        ui.section_header("Containers", "Collapsing header, group frame, separator");
        egui::CollapsingHeader::new("Collapsing header")
            .default_open(true)
            .show(ui, |ui| {
                ui.group(|ui| ui.label("Group frame"));
            });
        ui.separator();

        ui.section_header(
            "Status",
            "Badges: the text says the state, the color backs it up",
        );
        ui.horizontal(|ui| {
            ui.badge("online", Tone::Good, "Badge, good");
            ui.badge("degraded", Tone::Warn, "Badge, warn");
            ui.badge("failed", Tone::Crit, "Badge, crit");
            ui.badge("offline", Tone::Off, "Badge, off");
            ui.badge("new", Tone::Accent, "Badge, accent");
        });

        ui.section_header(
            "Palette",
            "Color tokens; hover a swatch for its name and hex",
        );
        let c = t.colors;
        let swatches: [(&str, Color32); 19] = [
            ("bg", c.bg),
            ("surface", c.surface),
            ("surface_raised", c.surface_raised),
            ("line", c.line),
            ("line_strong", c.line_strong),
            ("title", c.title),
            ("text", c.text),
            ("text_muted", c.text_muted),
            ("accent", c.accent),
            ("accent_alt", c.accent_alt),
            ("primary", c.primary),
            ("good", c.good),
            ("warn", c.warn),
            ("crit", c.crit),
            ("off", c.off),
            ("red", c.red),
            ("selection", c.selection),
            ("hover", c.hover),
            ("focus", c.focus),
        ];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = t.space.xs;
            for (name, color) in swatches {
                let (rect, r) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                ui.painter().rect(
                    rect,
                    t.radius.s,
                    color,
                    egui::Stroke::new(t.stroke.thin, c.line_strong),
                    egui::StrokeKind::Inside,
                );
                r.on_hover_text(format!("{name} {}", color.to_hex()));
            }
        });
    }
}
