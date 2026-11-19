//! The icons page of the gallery (CHR-7), laid out as the design's frame I1: Phosphor and the crate's SVG icons at
//! 16, 20, 24 and 32 points, then both in a list at the chrome's icon size; its settings above.

use egui::{Frame, Margin, RichText, Theme, Ui};

use super::{Icon, IconImage, Slot, SvgIcon, own, ph};
use crate::{Density, Tokens, UiExt};

/// The Phosphor icons of frame I1.
pub const PHOSPHOR_I1: [&str; 4] = [
    ph::FOLDER_OPEN,
    ph::MAGNIFYING_GLASS,
    ph::GEAR_SIX,
    ph::WAVEFORM,
];

/// The sizes of frame I1, points.
pub const SIZES_I1: [f32; 4] = [16.0, 20.0, 24.0, 32.0];

/// Width of the name column in the list, points.
const LIST_NAME_WIDTH: f32 = 96.0;

/// Settings of the icons page.
#[derive(Clone, Debug, PartialEq)]
pub struct IconsGallery {
    /// Density of the list (its row height and icon size come from it).
    pub density: Density,
    /// Draw every slot of the SVG icons in the ink colour (a one-colour look).
    pub one_colour: bool,
    /// The slot drawn where the SVG icons use the accent.
    pub accent: Slot,
}

impl Default for IconsGallery {
    fn default() -> Self {
        Self {
            density: Density::Comfortable,
            one_colour: false,
            accent: Slot::Accent,
        }
    }
}

impl IconsGallery {
    /// Dark and light next to each other, each on its own background.
    pub fn side_by_side(&mut self, ui: &mut Ui) {
        self.settings(ui);
        ui.columns(2, |cols| {
            for (col, theme) in cols.iter_mut().zip([Theme::Dark, Theme::Light]) {
                col.push_id(theme, |ui| self.themed(ui, theme));
            }
        });
    }

    /// The page's settings: density, one colour, the accent slot.
    pub fn settings(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (d, name) in [
                (Density::Comfortable, "Comfortable"),
                (Density::Compact, "Compact"),
            ] {
                ui.selectable_value(&mut self.density, d, name)
                    .on_hover_text("Density: the list's row height and icon size");
            }
            ui.separator();
            ui.checkbox(&mut self.one_colour, "One colour")
                .on_hover_text("Every slot of the SVG icons in the ink colour");
            ui.separator();
            ui.label("Accent slot").on_hover_text(
                "The colour drawn where the SVG icons use the accent (the desktop accent by default)",
            );
            for slot in [Slot::Accent, Slot::Good, Slot::Warn, Slot::Crit] {
                ui.selectable_value(&mut self.accent, slot, slot.name())
                    .on_hover_text(format!("The accent shapes in the {} token", slot.name()));
            }
        });
    }

    /// The page in one theme, on that theme's background.
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

    fn own(&self, icon: &'static SvgIcon, tokens: &Tokens) -> IconImage {
        let ink = Slot::Ink.color(tokens);
        let img = IconImage::new(icon).color(Slot::Accent, self.accent.color(tokens));
        if self.one_colour {
            Slot::ALL.into_iter().fold(img, |img, s| img.color(s, ink))
        } else {
            img
        }
    }

    /// The page in the `Ui`'s current theme.
    pub fn ui(&mut self, ui: &mut Ui) {
        let t = ui.tokens();
        let chrome = crate::tokens::Chrome::for_density(self.density);
        ui.section_header(
            "Phosphor regular",
            "The base set: a font, tinted like text (ve_theme::icons::ph)",
        );
        Self::strip(
            ui,
            &t,
            PHOSPHOR_I1.map(|g| (Icon::from(g).name(), IconImage::new(g))),
        );
        ui.add_space(t.space.l);
        ui.section_header(
            "SVG icons (slots: ink, muted, accent)",
            "SVG files in ve_theme/icons with colour slots bound to tokens (ve_theme::icons::own)",
        );
        Self::strip(
            ui,
            &t,
            own::ALL.map(|i| (i.name.to_owned(), self.own(i, &t))),
        );
        ui.add_space(t.space.l);
        ui.section_header(
            "In a list",
            "The chrome's icon size and row height at the chosen density",
        );
        Frame::new()
            .fill(t.colors.surface)
            .corner_radius(t.radius.l)
            .inner_margin(Margin::symmetric(t.space.l as i8, t.space.s as i8))
            .show(ui, |ui| {
                for (glyph, svg) in PHOSPHOR_I1.into_iter().zip(own::ALL) {
                    ui.horizontal(|ui| {
                        ui.set_height(chrome.row);
                        let name = svg.name;
                        ui.add(self.own(svg, &t).size(chrome.icon))
                            .on_hover_text(format!("{name}: an SVG icon with colour slots"));
                        ui.allocate_ui_with_layout(
                            egui::vec2(LIST_NAME_WIDTH, chrome.row),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(LIST_NAME_WIDTH);
                                ui.label(RichText::new(name).size(chrome.text_size))
                            },
                        );
                        let icon = Icon::from(glyph);
                        ui.add(IconImage::new(glyph).size(chrome.icon))
                            .on_hover_text(format!("{}: Phosphor regular", icon.name()));
                        ui.label(
                            RichText::new(icon.name())
                                .size(chrome.text_size)
                                .color(t.colors.text_muted),
                        );
                    });
                }
            });
    }

    fn strip(ui: &mut Ui, t: &Tokens, items: impl IntoIterator<Item = (String, IconImage)>) {
        Frame::new()
            .fill(t.colors.surface)
            .corner_radius(t.radius.l)
            .inner_margin(Margin::same(t.space.l as i8))
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    for (name, icon) in items {
                        let gap = ui.spacing().item_spacing.x;
                        let width =
                            SIZES_I1.iter().sum::<f32>() + gap * (SIZES_I1.len() - 1) as f32;
                        let column = egui::Layout::top_down(egui::Align::Center);
                        ui.allocate_ui_with_layout(egui::vec2(width, 0.0), column, |ui| {
                            ui.set_width(width);
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_height(SIZES_I1[SIZES_I1.len() - 1]);
                                    for size in SIZES_I1 {
                                        ui.add(icon.size(size))
                                            .on_hover_text(format!("{name} at {size} points"));
                                    }
                                },
                            );
                            ui.label(
                                RichText::new(name)
                                    .monospace()
                                    .small()
                                    .color(t.colors.text_muted),
                            );
                        });
                        ui.add_space(t.space.l);
                    }
                });
            });
    }
}
