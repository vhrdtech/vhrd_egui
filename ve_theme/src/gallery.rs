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
            text: "dev-pc-a".to_owned(),
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

        ui.section_header(
            "Containers",
            "Collapsing header, group frame, the rest of a cut list under a fold (BAS-5), separator",
        );
        egui::CollapsingHeader::new("Collapsing header")
            .default_open(true)
            .show(ui, |ui| {
                ui.group(|ui| ui.label("Group frame"));
            });
        ui.label("busy-1 · working");
        let muted = ui.tokens().colors.text_muted;
        let (header, _) = ve_basics::fold_rest(
            ui,
            "gallery-fold-rest",
            RichText::new("2 more subagents, not busy").color(muted),
            |ui| {
                ui.label("idle-a · done");
                ui.label("idle-b · failed");
            },
        );
        header.on_hover_text(
            "Fold rest: the rest of a cut list, collapsed by default; click to show them",
        );
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
        let swatches: [(&str, Color32); 21] = [
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
            ("slug", c.slug),
            ("task", c.task),
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

/// The property-panel widgets ([`crate::props`], THM-17) in one pane: the gallery's second page, and the content
/// of the `props_dark.png` / `props_light.png` snapshots.
#[derive(Clone, Debug)]
pub struct PropsGallery {
    width: u8,
    scalar: f64,
    angle: f32,
    vec3: [f64; 3],
    blend: u8,
    layers: [bool; 3],
    selected: usize,
    filter: String,
}

impl Default for PropsGallery {
    fn default() -> Self {
        Self {
            width: 2,
            scalar: 7.0,
            angle: 90.0,
            vec3: [10.1, 7.124, 100.0],
            blend: 0,
            layers: [true, true, false],
            selected: 1,
            filter: String::new(),
        }
    }
}

const ROWS: [(&str, &str, Tone); 7] = [
    ("First World", "reviewed", Tone::Good),
    ("Second Nature", "unreviewed", Tone::Warn),
    ("Third Degree", "stale", Tone::Crit),
    ("Fourth Wall", "no footprint", Tone::Off),
    ("Fifth Column", "reviewed", Tone::Good),
    ("Sixth Sense", "partial", Tone::Warn),
    ("Seventh Seal", "reviewed", Tone::Good),
];

impl PropsGallery {
    /// Dark and light next to each other, each on its own background.
    pub fn side_by_side(&mut self, ui: &mut Ui) {
        ui.columns(2, |cols| {
            for (col, theme) in cols.iter_mut().zip([Theme::Dark, Theme::Light]) {
                col.push_id(theme, |ui| self.themed(ui, theme));
            }
        });
    }

    /// The pane in one theme, on that theme's background.
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

    /// The pane in the `Ui`'s current theme.
    pub fn ui(&mut self, ui: &mut Ui) {
        use crate::props::*;
        let width = &mut self.width;
        Pane::new()
            .bar(|ui| {
                ui.primary_button("▶ Run", "Primary button: the one main action of the pane");
                ui.separator();
                Segmented::new(width)
                    .item(1, "¼", "A quarter of the window")
                    .item(2, "½", "Half the window")
                    .item(3, "¾", "Three quarters of the window")
                    .show(ui);
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.button("×").on_hover_text("Close the pane");
                    ui.button("▾").on_hover_text("More: the pane's menu");
                });
            })
            .title(
                "A standard editor pane",
                "Pane: the panel's box, a bar on top, a quiet title",
            )
            .show(ui, |ui| {
                SubPane::new("Sub-pane")
                    .center("Center")
                    .summary("Right")
                    .tooltip(
                        "Sub-pane: a section that folds; folded, its summary says what is inside",
                    )
                    .show(ui, |ui| {
                        Group::new("A standard group")
                            .tooltip("Group: properties that belong together")
                            .show(ui, |ui| {
                                PropertyRow::new("Scalar property")
                                    .tooltip("Property row: the name above its input")
                                    .show(ui, |ui| {
                                        ui.add(
                                            ScalarField::new(&mut self.scalar)
                                                .unit("mm")
                                                .tooltip("A length"),
                                        )
                                    });
                                PropertyRow::new("Angle").show(ui, |ui| {
                                    ui.add(
                                        ScalarField::new(&mut self.angle)
                                            .unit("°")
                                            .decimals(1)
                                            .range(-360.0..=360.0)
                                            .tooltip("Counter-clockwise"),
                                    )
                                });
                                PropertyRow::new("Vec3 property").show(ui, |ui| {
                                    let [x, y, z] = &mut self.vec3;
                                    ui.add(VecField::new([x, y, z]).decimals(3).tooltip("A point"))
                                });
                                PropertyRow::new("Disabled").show(ui, |ui| {
                                    let mut v = 0.5;
                                    ui.add(
                                        ScalarField::new(&mut v).disabled_because(Some(
                                            "not clickable: open a file first",
                                        )),
                                    )
                                });
                            });
                        Group::new("Choices").show(ui, |ui| {
                            Segmented::new(&mut self.blend)
                                .item(0, "Normal", "Segmented: one choice of a few")
                                .item(1, "Multiply", "Dark where both are dark")
                                .item_if(2, "Screen", "", Some("needs the wgpu renderer"))
                                .fill_width()
                                .show(ui);
                            ui.horizontal_wrapped(|ui| {
                                for (on, name) in
                                    self.layers.iter_mut().zip(["Pads", "Courtyard", "Silk"])
                                {
                                    ui.add(
                                        ToggleChip::new(on, name)
                                            .tooltip("Toggle chip: shown or not"),
                                    );
                                }
                                let mut off = false;
                                ui.add(
                                    ToggleChip::new(&mut off, "Fab")
                                        .disabled_because(Some("the footprint has no fab layer")),
                                );
                            });
                        });
                        Facts::new("gallery facts").show(ui, |f| {
                            f.fact("Footprint", "Package_DFN_QFN:WQFN-20-1EP_3x3mm_P0.4mm", "");
                            f.fact_mono("Hash", "fp1:f460b103cd5189b2", "The review hash");
                            f.fact_tone("Status", "unreviewed", Tone::Warn, "No review line yet");
                        });
                        notice(ui, Tone::Warn, "Notice: a warning beside what it is about");
                        hint(ui, "Hint: why something is off and what turns it on");
                    });
                SubPane::new("List")
                    .summary(format!("{} items", ROWS.len()))
                    .show(ui, |ui| {
                        filter_field(ui, &mut self.filter, "filter: name or status");
                        let f = self.filter.to_lowercase();
                        ListView::new("gallery list")
                            .max_height(150.0)
                            .show(ui, |list| {
                                for (i, (name, word, tone)) in ROWS.into_iter().enumerate() {
                                    if !f.is_empty()
                                        && !name.to_lowercase().contains(&f)
                                        && !word.contains(&f)
                                    {
                                        continue;
                                    }
                                    if list
                                        .row(
                                            ListRow::new(name)
                                                .lead(word, tone)
                                                .selected(self.selected == i),
                                        )
                                        .clicked()
                                    {
                                        self.selected = i;
                                    }
                                }
                                if list.is_empty() {
                                    list.empty("nothing matches the filter");
                                }
                            });
                    });
                SubPane::new("Folded")
                    .default_open(false)
                    .summary("3 settings inside")
                    .show(ui, |ui| {
                        hint(ui, "The body of a folded sub-pane");
                    });
            });
    }
}
