//! [`UiExt`]: the styles egui's `Style` cannot express (THM-4).
//!
//! Every interactive helper takes its tooltip as an argument, so the AGENTS.md UI guide rule "generous
//! tooltips" holds by construction: say what it is, its unit, what clicking it does.

use egui::{
    Align2, Button, Color32, CornerRadius, Frame, InnerResponse, Margin, Response, RichText, Sense,
    Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetText, WidgetType, vec2,
};
use std::sync::Arc;

use crate::Tokens;
use crate::typography::{Face, TypeStep};

/// What a [`UiExt::badge`] says, which picks its color. Status tones are never the only carrier of meaning:
/// the badge text is the label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Versions, variants, counts: hairline box, muted text.
    Neutral,
    /// Something to notice, not a state: the focus / interaction teal.
    Accent,
    /// Healthy / online.
    Good,
    /// Degraded / needs attention.
    Warn,
    /// Failing / critical.
    Crit,
    /// Offline / disabled.
    Off,
}

impl Tone {
    /// The tone's base color in `tokens`.
    pub fn color(self, tokens: &Tokens) -> Color32 {
        let c = &tokens.colors;
        match self {
            Self::Neutral => c.line_strong,
            Self::Accent => c.focus,
            Self::Good => c.good,
            Self::Warn => c.warn,
            Self::Crit => c.crit,
            Self::Off => c.off,
        }
    }
}

/// VHRD styles on top of egui. `use ve_theme::UiExt as _;` then `ui.primary_button(..)`.
pub trait UiExt {
    /// The `Ui` the helpers draw into.
    fn ui_mut(&mut self) -> &mut Ui;

    /// The tokens for this `Ui`'s current theme (dark or light), as installed by [`crate::setup`].
    fn tokens(&mut self) -> Arc<Tokens> {
        Tokens::of_ui(self.ui_mut())
    }

    /// The one main action of a view: filled with `primary`, text in the matching on-color.
    fn primary_button(
        &mut self,
        text: impl Into<String>,
        tooltip: impl Into<WidgetText>,
    ) -> Response {
        let ui = self.ui_mut();
        let fill = Tokens::of_ui(ui).colors.primary;
        filled_button(ui, text.into(), fill, tooltip.into())
    }

    /// A destructive action (delete, reset, disconnect): filled with `crit`. Say in the tooltip what is lost.
    fn danger_button(
        &mut self,
        text: impl Into<String>,
        tooltip: impl Into<WidgetText>,
    ) -> Response {
        let ui = self.ui_mut();
        let fill = Tokens::of_ui(ui).colors.crit;
        filled_button(ui, text.into(), fill, tooltip.into())
    }

    /// A section title in the `title` style with a hairline running to the right edge. Returns the title's
    /// response (with `tooltip`), e.g. for `ve_basics::hover_link`.
    fn section_header(
        &mut self,
        title: impl Into<String>,
        tooltip: impl Into<WidgetText>,
    ) -> Response {
        let ui = self.ui_mut();
        let t = Tokens::of_ui(ui);
        ui.add_space(t.space.s);
        let response = ui
            .horizontal(|ui| {
                let r = ui.label(
                    RichText::new(title.into())
                        .font(t.type_scale.title.font_id())
                        .color(t.colors.title),
                );
                let y = r.rect.center().y;
                let x0 = r.rect.right() + t.space.m;
                let x1 = ui.max_rect().right();
                if x1 > x0 {
                    ui.painter()
                        .hline(x0..=x1, y, Stroke::new(t.stroke.thin, t.colors.line));
                }
                r
            })
            .inner;
        ui.add_space(t.space.xs);
        response.on_hover_text(tooltip)
    }

    /// A full-width title bar at the top of a panel: `title` on the left (in the `title` style, with
    /// `tooltip`), whatever `add_right` adds (buttons, a badge) on the right, a hairline below.
    /// The returned response is the title's.
    fn panel_title_bar<R>(
        &mut self,
        title: impl Into<String>,
        tooltip: impl Into<WidgetText>,
        add_right: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let ui = self.ui_mut();
        let t = Tokens::of_ui(ui);
        let title = title.into();
        let tooltip = tooltip.into();
        let frame = Frame::new()
            .fill(t.colors.surface_raised)
            .inner_margin(Margin::symmetric(t.space.m as i8, t.space.s as i8));
        let inner = frame.show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let height = t
                .type_scale
                .title
                .line_height
                .max(ui.spacing().interact_size.y);
            egui::Sides::new().height(height).show(
                ui,
                |ui| {
                    ui.label(
                        RichText::new(title)
                            .font(t.type_scale.title.font_id())
                            .color(t.colors.title),
                    )
                    .on_hover_text(tooltip)
                },
                add_right,
            )
        });
        let rect = inner.response.rect;
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            Stroke::new(t.stroke.thin, t.colors.line),
        );
        let (title_response, right) = inner.inner;
        InnerResponse::new(right, title_response)
    }

    /// A compact horizontal strip for tool buttons on a `surface` fill. Give each button its tooltip.
    fn toolbar<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let ui = self.ui_mut();
        let t = Tokens::of_ui(ui);
        Frame::new()
            .fill(t.colors.surface)
            .stroke(Stroke::new(t.stroke.thin, t.colors.line))
            .corner_radius(CornerRadius::same(t.radius.m))
            .inner_margin(Margin::same(t.space.s as i8))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = t.space.s;
                ui.spacing_mut().button_padding = vec2(t.space.s + t.space.xs, t.space.xs);
                ui.horizontal(add_contents).inner
            })
    }

    /// Secondary text (hints, units, explanations) in `text_muted`. Still AA on every background.
    fn muted_label(&mut self, text: impl Into<RichText>) -> Response {
        let ui = self.ui_mut();
        let color = Tokens::of_ui(ui).colors.text_muted;
        ui.label(text.into().color(color))
    }

    /// A small mono badge: a version, a variant, or a state. `tone` picks the color; the text is always the
    /// label (status is never color alone), `tooltip` says what it means.
    fn badge(
        &mut self,
        text: impl Into<String>,
        tone: Tone,
        tooltip: impl Into<WidgetText>,
    ) -> Response {
        let ui = self.ui_mut();
        let t = Tokens::of_ui(ui);
        let text = text.into();
        let base = tone.color(&t);
        let bg = t.colors.bg;
        let (fill, text_color) = match tone {
            Tone::Neutral => (Color32::TRANSPARENT, t.colors.text_muted),
            _ => (
                base.gamma_multiply(0.14),
                t.legible(base, bg.blend(base.gamma_multiply(0.14))),
            ),
        };
        let step = TypeStep {
            face: Face::Mono,
            ..t.type_scale.caption
        };
        let galley = ui
            .painter()
            .layout_no_wrap(text.clone(), step.font_id(), text_color);
        let pad = vec2(t.space.s + t.space.xs, t.space.xs);
        let size = galley.size() + 2.0 * pad;
        let (rect, response) = ui.allocate_exact_size(size.max(Vec2::ZERO), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let radius = CornerRadius::same(t.radius.s);
            painter.rect(
                rect,
                radius,
                fill,
                Stroke::new(t.stroke.thin, base),
                StrokeKind::Inside,
            );
            painter.galley(
                Align2::CENTER_CENTER
                    .anchor_size(rect.center(), galley.size())
                    .min,
                galley,
                text_color,
            );
        }
        response.on_hover_text(tooltip)
    }
}

impl UiExt for Ui {
    fn ui_mut(&mut self) -> &mut Ui {
        self
    }
}

fn filled_button(ui: &mut Ui, text: String, fill: Color32, tooltip: WidgetText) -> Response {
    let t = Tokens::of_ui(ui);
    let on = t.on(fill);
    let hovered = fill.lerp_to_gamma(t.colors.text, 0.15);
    let pressed = fill.lerp_to_gamma(t.colors.text, 0.28);
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        for (state, f) in [
            (&mut w.inactive, fill),
            (&mut w.hovered, hovered),
            (&mut w.active, pressed),
        ] {
            state.weak_bg_fill = f;
            state.bg_fill = f;
            state.bg_stroke = Stroke::new(t.stroke.thin, f);
            state.fg_stroke.color = on;
        }
        ui.add(Button::new(text))
    })
    .inner
    .on_hover_text(tooltip)
}
