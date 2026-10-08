//! Titled panel, btop style: a rounded border with the title set into the
//! top border line.

use egui::{Align2, CornerRadius, FontId, Frame, Id, InnerResponse, Margin, Stroke, Ui, pos2};

use crate::theme::Theme;

/// How much room a panel asks for. The layout decides what it does with it: `Auto` leaves the choice to the
/// layout (packing, later physics), `Compact` keeps the panel in one narrow column, `Wide` gives it the whole
/// row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PanelSize {
    #[default]
    Auto,
    Compact,
    Wide,
}

impl PanelSize {
    /// The next one in the cycle auto → compact → wide → auto (what a click on the switch does).
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Compact,
            Self::Compact => Self::Wide,
            Self::Wide => Self::Auto,
        }
    }

    /// The word on the switch.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Compact => "compact",
            Self::Wide => "wide",
        }
    }

    /// What the panel with `key` is set to right now ([`Panel::size_switch`]); `Auto` until it was clicked.
    pub fn of(ctx: &egui::Context, key: impl std::hash::Hash + std::fmt::Debug) -> Self {
        let id = Id::new(("ve_dash::panel_size", key));
        ctx.data(|d| d.get_temp::<Self>(id)).unwrap_or_default()
    }
}

/// A bordered panel whose header line carries the title at the left and, at the right, an ⓘ button (its
/// tooltip holds what the panel has no room for) and a size switch — all on the border line, so neither takes a
/// row of its own.
///
/// ```ignore
/// Panel::new(&theme, "model usage")
///     .info(|ui| { ui.label("plan, PCs, ..."); })
///     .size_switch("usage")
///     .show(ui, |ui| { /* widgets */ });
/// // the layout reads what was chosen:
/// let size = PanelSize::of(ctx, "usage");
/// ```
#[must_use = "call .show(ui, ..)"]
pub struct Panel<'a> {
    theme: &'a Theme,
    title: String,
    info: Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    size_key: Option<Id>,
}

impl<'a> Panel<'a> {
    pub fn new(theme: &'a Theme, title: impl Into<String>) -> Self {
        Self {
            theme,
            title: title.into(),
            info: None,
            size_key: None,
        }
    }

    /// The ⓘ in the header: `add_info` fills its tooltip.
    pub fn info(mut self, add_info: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.info = Some(Box::new(add_info));
        self
    }

    /// The auto / compact / wide switch in the header, remembered under `key` (read it with
    /// [`PanelSize::of`] with the same key).
    pub fn size_switch(mut self, key: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.size_key = Some(Id::new(("ve_dash::panel_size", key)));
        self
    }

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let theme = self.theme;
        let frame = Frame::new()
            .fill(theme.panel_bg)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 14,
                bottom: 10,
            })
            .outer_margin(Margin {
                left: 0,
                right: 0,
                top: 6, // room for the title text above the border line
                bottom: 0,
            });
        let inner = frame.show(ui, add_contents);

        let frame_rect = inner.response.rect;
        if !ui.is_rect_visible(frame_rect) {
            return inner;
        }
        if !self.title.is_empty() {
            let painter = ui.painter();
            let font = FontId::proportional(12.0);
            let text_pos = pos2(frame_rect.left() + 12.0, frame_rect.top());
            let text_size = painter
                .layout_no_wrap(self.title.clone(), font.clone(), theme.title)
                .size();
            // Blank the border segment behind the title, then draw it.
            let bg = Align2::LEFT_CENTER
                .anchor_size(text_pos, text_size)
                .expand(3.0);
            painter.rect_filled(bg, 0.0, theme.bg);
            painter.text(
                text_pos,
                Align2::LEFT_CENTER,
                &self.title,
                font,
                theme.title,
            );
        }
        self.header_controls(ui, frame_rect);
        inner
    }

    /// The ⓘ and the size switch on the right of the border line.
    fn header_controls(self, ui: &mut Ui, frame_rect: egui::Rect) {
        let theme = self.theme;
        if self.info.is_none() && self.size_key.is_none() {
            return;
        }
        let icon = 13.0;
        let font = FontId::monospace(10.0);
        // The widest word of the switch, so the header doesn't change width as it cycles.
        let word_w = [PanelSize::Auto, PanelSize::Compact, PanelSize::Wide]
            .iter()
            .map(|s| {
                ui.painter()
                    .layout_no_wrap(s.label().to_owned(), font.clone(), theme.text)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let chip_w = word_w + 10.0;
        let mut width = 6.0;
        if self.size_key.is_some() {
            width += chip_w + 4.0;
        }
        if self.info.is_some() {
            width += icon + 4.0;
        }
        let right = frame_rect.right() - 10.0;
        let rect = egui::Rect::from_min_max(
            pos2(right - width, frame_rect.top() - 9.0),
            pos2(right, frame_rect.top() + 9.0),
        );
        ui.painter().rect_filled(rect, 0.0, theme.bg);
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        inner.spacing_mut().item_spacing.x = 4.0;
        if let Some(add_info) = self.info {
            crate::info_icon(&mut inner, theme, icon).on_hover_ui(add_info);
        }
        if let Some(id) = self.size_key {
            let size = inner
                .data(|d| d.get_temp::<PanelSize>(id))
                .unwrap_or_default();
            let chip = crate::ActionChip::new(
                egui::RichText::new(size.label())
                    .font(font)
                    .color(theme.text_muted),
                theme,
            )
            .min_width(chip_w)
            .active(size != PanelSize::Auto)
            .hover(format!(
                "Panel size: {} (click for {}). Auto lets the layout decide, compact keeps it in one narrow column, wide gives it the whole row.",
                size.label(),
                size.next().label()
            ))
            .show(&mut inner);
            if chip.clicked() {
                inner.data_mut(|d| d.insert_temp(id, size.next()));
            }
        }
    }
}

/// Draw a bordered panel with `title` sitting on the top border (a [`Panel`] without controls).
///
/// ```ignore
/// panel(ui, &theme, "omarchy-m1", |ui| { /* widgets */ });
/// ```
pub fn panel<R>(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    Panel::new(theme, title).show(ui, add_contents)
}
