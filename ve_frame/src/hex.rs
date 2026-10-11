//! Hexagon geometry and the items on its slanted faces (FRM-3).

use egui::{FontId, Pos2, Rangef, Rect, Sense, Ui, Vec2, pos2, vec2};
use ve_dash::{Status, Theme};

/// One of the four slanted faces of a flat-topped hexagon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HexFace {
    UpperLeft,
    UpperRight,
    LowerLeft,
    LowerRight,
}

impl HexFace {
    /// All four, clockwise from the upper left.
    pub const ALL: [Self; 4] = [
        Self::UpperLeft,
        Self::UpperRight,
        Self::LowerRight,
        Self::LowerLeft,
    ];
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Text(String),
    Led(Status),
    Meter { frac: f32, segments: u8 },
    Button(String),
    Toggle(String, bool),
}

/// A thing in the bar on a slanted face of a hexagon ([`crate::WidgetFrame::hex_side`]), drawn rotated with
/// the face: a text, an LED, a small segmented meter, or a simple control (button, toggle) that is hit-tested
/// through the rotation. Arbitrary egui UI can't be rotated, so a bar takes these items instead of a closure.
#[derive(Clone, Debug, PartialEq)]
pub struct SideItem {
    kind: Kind,
    hover: String,
}

impl SideItem {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            hover: String::new(),
        }
    }

    /// A short text or an icon glyph ("24 °C", "ch 2"). Cut with an ellipsis when the bar is too short.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(Kind::Text(text.into()))
    }

    /// A status dot. Give it a [`Self::hover`]: colour alone doesn't say what it means.
    pub fn led(status: Status) -> Self {
        Self::new(Kind::Led(status))
    }

    /// A small bar of `segments` blocks (1 to 10), `frac` of them lit, in the heat colours.
    pub fn meter(frac: f32, segments: u8) -> Self {
        Self::new(Kind::Meter {
            frac: if frac.is_finite() {
                frac.clamp(0.0, 1.0)
            } else {
                0.0
            },
            segments: segments.clamp(1, 10),
        })
    }

    /// A button. A click comes back in [`crate::FrameResponse::side_clicks`].
    pub fn button(text: impl Into<String>) -> Self {
        Self::new(Kind::Button(text.into()))
    }

    /// A switch with its label, drawn `on` or off. It keeps no state: a click comes back in
    /// [`crate::FrameResponse::side_clicks`] and the widget flips its own flag.
    pub fn toggle(text: impl Into<String>, on: bool) -> Self {
        Self::new(Kind::Toggle(text.into(), on))
    }

    /// The tooltip: what the item is, its unit, what clicking it does.
    pub fn hover(mut self, hover: impl Into<String>) -> Self {
        self.hover = hover.into();
        self
    }
}

/// How far the slanted faces reach in from the left and right edge of a hexagon `height` tall: the faces
/// stand at 60°, as in a regular hexagon, however wide the flat faces are.
pub fn slant(height: f32) -> f32 {
    height / (2.0 * 3f32.sqrt())
}

/// The corners of the hexagon filling `rect`, clockwise from the top left.
pub fn points(rect: Rect) -> [Pos2; 6] {
    let s = slant(rect.height());
    let cy = rect.center().y;
    [
        pos2(rect.left() + s, rect.top()),
        pos2(rect.right() - s, rect.top()),
        pos2(rect.right(), cy),
        pos2(rect.right() - s, rect.bottom()),
        pos2(rect.left() + s, rect.bottom()),
        pos2(rect.left(), cy),
    ]
}

/// The hexagon's extent in x at height `y`.
pub fn span_at(rect: Rect, y: f32) -> Rangef {
    let half = rect.height() / 2.0;
    let off = if half > 0.0 {
        ((y - rect.center().y).abs() / half).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let inset = slant(rect.height()) * off;
    Rangef::new(rect.left() + inset, rect.right() - inset)
}

/// The default thickness of a side bar, in points.
pub const BAR_THICKNESS: f32 = 18.0;

/// How far a bar `thickness` thick reaches into the hexagon, measured along a row (its faces stand at 60°).
pub fn bar_reach(thickness: f32) -> f32 {
    thickness / (3f32.sqrt() / 2.0)
}

/// The bar of `face`: a band `thickness` thick along the slanted face, from the left or right vertex to the
/// header (upper faces, `top` is its lower edge) or the footer (lower faces, `bottom` is its upper edge). Its
/// ends are cut level, so the upper and the lower bar of a side meet in a seam at the vertex's height. The
/// points go around it: the face from the vertex to the flat edge, then back along the side toward the body.
pub fn bar(rect: Rect, face: HexFace, thickness: f32, top: f32, bottom: f32) -> [Pos2; 4] {
    let cy = rect.center().y;
    let (end_y, right) = match face {
        HexFace::UpperLeft => (top, false),
        HexFace::UpperRight => (top, true),
        HexFace::LowerLeft => (bottom, false),
        HexFace::LowerRight => (bottom, true),
    };
    let span = |y: f32| span_at(rect, y);
    let reach = bar_reach(thickness);
    let (vertex, end, inward) = if right {
        (span(cy).max, span(end_y).max, -reach)
    } else {
        (span(cy).min, span(end_y).min, reach)
    };
    [
        pos2(vertex, cy),
        pos2(end, end_y),
        pos2(end + inward, end_y),
        pos2(vertex + inward, cy),
    ]
}

/// One of the two regions of a hexagon beside its body: between the body's side and the two slanted faces that
/// meet at the left or right vertex. A body that asked for them ([`crate::WidgetFrame::hex_wings`]) lays
/// content into them; egui clips to rectangles only, so the content keeps inside the outline itself, with
/// [`Wing::span_at`], [`Wing::inscribed`] and [`Wing::contains`].
#[derive(Clone, Debug, PartialEq)]
pub struct Wing {
    /// The outline, clockwise: a triangle with its tip at the hexagon's vertex, cut off at the header and the
    /// footer, a few points in from the faces.
    pub polygon: Vec<Pos2>,
    /// The rectangle around [`Self::polygon`].
    pub rect: Rect,
    tip: Pos2,
    /// The side toward the body.
    inner_x: f32,
    /// How fast the wing narrows: points of width lost per point away from the tip's height.
    slope: f32,
}

impl Wing {
    /// The usable extent in x at height `y`; `None` above or below the wing.
    pub fn span_at(&self, y: f32) -> Option<Rangef> {
        if y < self.rect.top() || y > self.rect.bottom() {
            return None;
        }
        let reach = (y - self.tip.y).abs() * self.slope;
        let (a, b) = if self.tip.x < self.inner_x {
            (self.tip.x + reach, self.inner_x)
        } else {
            (self.inner_x, self.tip.x - reach)
        };
        (b > a).then(|| Rangef::new(a, b))
    }

    /// The largest upright rectangle that fits into the wing, for a label or a small control.
    pub fn inscribed(&self) -> Rect {
        let half_x = (self.tip.x + self.inner_x) / 2.0;
        let dy = if self.slope > 0.0 {
            (half_x - self.tip.x).abs() / self.slope
        } else {
            0.0
        };
        let rows = Rangef::new(
            (self.tip.y - dy).max(self.rect.top()),
            (self.tip.y + dy).min(self.rect.bottom()),
        );
        let cols = Rangef::new(half_x.min(self.inner_x), half_x.max(self.inner_x));
        Rect::from_x_y_ranges(cols, rows)
    }

    /// Whether `p` lies in the wing.
    pub fn contains(&self, p: Pos2) -> bool {
        self.span_at(p.y).is_some_and(|span| span.contains(p.x))
    }
}

/// Distance of a wing from the slanted faces.
const WING_MARGIN: f32 = 5.0;

/// The left and the right wing of the hexagon in `rect`, between the heights `top` and `bottom` (under the
/// header, above the footer); `body` is the column the body takes.
pub fn wings(rect: Rect, top: f32, bottom: f32, body: Rangef) -> [Wing; 2] {
    let half = rect.height() / 2.0;
    let slope = if half > 0.0 {
        slant(rect.height()) / half
    } else {
        0.0
    };
    let cy = rect.center().y;
    let make = |tip_x: f32, inner_x: f32| {
        let dir = (inner_x - tip_x).signum();
        let edge = |y: f32| tip_x + dir * (y - cy).abs() * slope;
        let tip = pos2(tip_x, cy);
        let mut polygon = vec![
            pos2(edge(top), top),
            pos2(inner_x, top),
            pos2(inner_x, bottom),
            pos2(edge(bottom), bottom),
            tip,
        ];
        if dir < 0.0 {
            polygon.reverse();
        }
        let rect = Rect::from_points(&polygon);
        Wing {
            polygon,
            rect,
            tip,
            inner_x,
            slope,
        }
    };
    [
        make(rect.left() + WING_MARGIN, body.min),
        make(rect.right() - WING_MARGIN, body.max),
    ]
}

/// Gap between items along a bar, and the free length at its ends.
const ITEM_GAP: f32 = 7.0;
const BAR_END: f32 = 4.0;
/// Diameter of an LED item.
const DOT: f32 = 7.0;
/// A meter's blocks: width, gap and (at most) height.
const SEG: (f32, f32, f32) = (3.0, 2.0, 10.0);

/// A frame of reference turned with a face: `along` it (left to right) and `down` (where a line of text grows
/// to), around an origin on the bar's centre line.
#[derive(Clone, Copy)]
struct Turned {
    origin: Pos2,
    along: Vec2,
    down: Vec2,
}

impl Turned {
    /// The screen point `x` along the bar and `y` below its centre line.
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + self.along * x + self.down * y
    }

    /// The four corners of the box from `x` to `x + w` along, `h` high around the centre line.
    fn quad(&self, x: f32, w: f32, h: f32) -> Vec<Pos2> {
        vec![
            self.at(x, -h / 2.0),
            self.at(x + w, -h / 2.0),
            self.at(x + w, h / 2.0),
            self.at(x, h / 2.0),
        ]
    }

    /// Where the screen point `p` is in the bar's frame: along, and below the centre line. This is the
    /// inverse rotation the controls are hit-tested through.
    fn local(&self, p: Pos2) -> Vec2 {
        let d = p - self.origin;
        vec2(d.dot(self.along), d.dot(self.down))
    }

    /// Whether `p` lies in the box from `x` to `x + w` along, `h` high.
    fn holds(&self, p: Pos2, x: f32, w: f32, h: f32) -> bool {
        let l = self.local(p);
        l.x >= x && l.x <= x + w && l.y.abs() <= h / 2.0
    }
}

/// `text` cut to `room` points with an ellipsis; `None` when not even one letter and the ellipsis fit.
fn cut_to(ui: &Ui, text: &str, font: &FontId, room: f32) -> Option<(String, f32)> {
    let width = |t: &str| {
        ui.painter()
            .layout_no_wrap(t.to_owned(), font.clone(), egui::Color32::WHITE)
            .size()
            .x
    };
    let full = width(text);
    if full <= room {
        return Some((text.to_owned(), full));
    }
    let letters: Vec<char> = text.chars().collect();
    (1..letters.len()).rev().find_map(|keep| {
        let cut: String = letters[..keep].iter().collect();
        let cut = format!("{}…", cut.trim_end());
        let w = width(&cut);
        (w <= room).then_some((cut, w))
    })
}

/// Draw the bar of `face` with its `items`, in the hexagon in `rect`: the band (quiet fill, a rule toward the
/// body), then the items rotated with the face. An item that doesn't fit is cut with an ellipsis (a text), or
/// an ellipsis stands for it and everything after it, with a tooltip naming what is missing. Returns the
/// indices of the controls that were clicked.
pub(crate) fn bar_ui(
    ui: &mut Ui,
    theme: &Theme,
    rect: Rect,
    (face, thickness): (HexFace, f32),
    (top, bottom): (f32, f32),
    (fill, line): (egui::Color32, egui::Stroke),
    items: &[SideItem],
) -> Vec<usize> {
    let quad = bar(rect, face, thickness, top, bottom);
    let [vertex, end, end_in, vertex_in] = quad;
    let painter = ui.painter().clone();
    painter.add(egui::Shape::convex_polygon(
        quad.to_vec(),
        fill,
        egui::Stroke::NONE,
    ));
    painter.line_segment([end_in, vertex_in], line);
    painter.line_segment([vertex, vertex_in], line);

    // the face from left to right, so text reads that way
    let (from, to, from_in) = if vertex.x < end.x {
        (vertex, end, vertex_in)
    } else {
        (end, vertex, end_in)
    };
    let length = (to - from).length();
    if length <= 0.0 {
        return Vec::new();
    }
    let along = (to - from) / length;
    let down = vec2(-along.y, along.x);
    let angle = along.y.atan2(along.x);
    // the centre line: half the thickness in from the face. The level cuts at the ends take a slice off the
    // inner side, at the vertex end; the upright box that fits starts after it.
    let inward = (from_in - from) - along * (from_in - from).dot(along);
    let turned = Turned {
        origin: from + inward * 0.5,
        along,
        down,
    };
    let slice = thickness / 3f32.sqrt();
    let (start, stop) = if vertex.x < end.x {
        (slice + BAR_END, length - BAR_END)
    } else {
        (BAR_END, length - slice - BAR_END)
    };

    let font = FontId::monospace(10.0);
    let text_h = ui.fonts_mut(|f| f.row_height(&font));
    let box_h = (thickness - 4.0).clamp(8.0, 16.0);
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let text_at = |x: f32, text: &str, color: egui::Color32| {
        let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
        let pos = turned.at(x, -galley.size().y / 2.0);
        painter.add(egui::epaint::TextShape::new(pos, galley, color).with_angle(angle));
    };
    let aabb = |x: f32, w: f32, h: f32| Rect::from_points(&turned.quad(x, w, h));

    let mut clicked = Vec::new();
    let mut at = start;
    for (i, item) in items.iter().enumerate() {
        let room = stop - at;
        let id = ui.id().with((face, i));
        // what the item needs along the bar; a text may be cut to the room there is
        let fits = match &item.kind {
            Kind::Text(text) => cut_to(ui, text, &font, room).map(|(shown, w)| (w, Some(shown))),
            Kind::Led(_) => Some((DOT, None)),
            Kind::Meter { segments, .. } => {
                let n = f32::from(*segments);
                Some((n * SEG.0 + (n - 1.0) * SEG.1, None))
            }
            Kind::Button(text) => {
                cut_to(ui, text, &font, room - 8.0).map(|(s, w)| (w + 8.0, Some(s)))
            }
            Kind::Toggle(text, _) => cut_to(ui, text, &font, room - TOGGLE_W - 4.0)
                .map(|(s, w)| (TOGGLE_W + 4.0 + w, Some(s))),
        };
        let Some((width, shown)) = fits.filter(|(w, _)| *w <= room) else {
            // no room: an ellipsis stands for this item and the ones after it
            let left = items.len() - i;
            if let Some((dots, w)) = cut_to(ui, "…", &font, room.max(0.0) + BAR_END) {
                text_at(at, &dots, theme.text_muted);
                let r = ui.interact(aabb(at, w, text_h), id, Sense::hover());
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Label, true, format!("{left} more"))
                });
                r.on_hover_text(format!(
                    "{left} more item{} on this side: no room for {}; make the widget larger",
                    if left == 1 { "" } else { "s" },
                    if left == 1 { "it" } else { "them" },
                ));
            }
            break;
        };
        let shown = shown.unwrap_or_default();
        let inside = |h: f32| pointer.is_some_and(|p| turned.holds(p, at, width, h));
        match &item.kind {
            Kind::Text(full) => {
                text_at(at, &shown, theme.text_muted);
                let r = ui.interact(aabb(at, width, text_h), id, Sense::hover());
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Label, true, full.clone())
                });
                let cut = (shown != *full).then(|| full.clone());
                tooltip(r, inside(text_h), cut, &item.hover);
            }
            Kind::Led(status) => {
                let c = turned.at(at + DOT / 2.0, 0.0);
                let color = status.color(theme);
                if *status != Status::Off {
                    painter.circle_filled(c, DOT * 0.5 + 2.5, color.gamma_multiply(0.2));
                }
                painter.circle_filled(c, DOT * 0.5, color);
                let r = ui.interact(aabb(at, DOT, DOT), id, Sense::hover());
                let label = format!("status: {}", item.hover);
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.clone())
                });
                tooltip(r, inside(DOT + 4.0), None, &item.hover);
            }
            Kind::Meter { frac, segments } => {
                let lit = (frac * f32::from(*segments)).ceil() as u8;
                let h = SEG.2.min(box_h);
                for n in 0..*segments {
                    let x = at + f32::from(n) * (SEG.0 + SEG.1);
                    let t = f32::from(n) / f32::from((*segments).max(2) - 1);
                    let color = if n < lit {
                        ve_dash::heat(theme, t)
                    } else {
                        theme.border
                    };
                    painter.add(egui::Shape::convex_polygon(
                        turned.quad(x, SEG.0, h),
                        color,
                        egui::Stroke::NONE,
                    ));
                }
                let r = ui.interact(aabb(at, width, h), id, Sense::hover());
                let label = format!("{:.0} %", frac * 100.0);
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.clone())
                });
                let percent = format!("{:.0} % ({lit} of {segments} segments)", frac * 100.0);
                tooltip(r, inside(h + 4.0), Some(percent), &item.hover);
            }
            Kind::Button(full) => {
                let r = ui.interact(aabb(at, width, box_h), id, Sense::click());
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, full.clone())
                });
                let over = r.hovered() && inside(box_h);
                let fill = if over && r.is_pointer_button_down_on() {
                    theme.border
                } else if over {
                    theme.border.gamma_multiply(0.7)
                } else {
                    theme.border.gamma_multiply(0.35)
                };
                painter.add(egui::Shape::convex_polygon(
                    turned.quad(at, width, box_h),
                    fill,
                    egui::Stroke::new(1.0, theme.border),
                ));
                text_at(at + 4.0, &shown, theme.text);
                if hit(&r, &turned, (at, width, box_h)) {
                    clicked.push(i);
                }
                let cut = (shown != *full).then(|| full.clone());
                tooltip(r, over, cut, &item.hover);
            }
            Kind::Toggle(full, on) => {
                let r = ui.interact(aabb(at, width, box_h), id, Sense::click());
                r.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, full.clone())
                });
                let over = r.hovered() && inside(box_h);
                let track_h = 9.0_f32.min(box_h);
                let track = if *on { theme.accent } else { theme.border };
                painter.add(egui::Shape::convex_polygon(
                    turned.quad(at, TOGGLE_W, track_h),
                    track,
                    egui::Stroke::new(1.0, if over { theme.text } else { theme.border }),
                ));
                let knob_x = if *on { TOGGLE_W - 4.5 } else { 4.5 };
                painter.circle_filled(turned.at(at + knob_x, 0.0), 3.0, theme.text);
                let color = if *on { theme.text } else { theme.text_muted };
                text_at(at + TOGGLE_W + 4.0, &shown, color);
                if hit(&r, &turned, (at, width, box_h)) {
                    clicked.push(i);
                }
                let state = format!("{full}: {}", if *on { "on" } else { "off" });
                tooltip(r, over, Some(state), &item.hover);
            }
        }
        at += width + ITEM_GAP;
    }
    clicked
}

/// Length of a toggle's track along the bar.
const TOGGLE_W: f32 = 16.0;

/// Whether a control was clicked inside its turned box `(x, width, height)`: egui reports a click anywhere in
/// the upright rectangle around it, the inverse rotation says whether the pointer was on the control. A click
/// without a pointer (keyboard, assistive technology) counts.
fn hit(r: &egui::Response, turned: &Turned, (x, w, h): (f32, f32, f32)) -> bool {
    r.clicked()
        && r.interact_pointer_pos()
            .is_none_or(|p| turned.holds(p, x, w, h))
}

/// The tooltip of an item, shown only while the pointer is on the turned item itself: `first` (the full text
/// of a cut one, a meter's value) above the item's own `hover`.
fn tooltip(r: egui::Response, over: bool, first: Option<String>, hover: &str) {
    let r = r.on_hover_cursor(egui::CursorIcon::Default);
    if !over || (first.is_none() && hover.is_empty()) {
        return;
    }
    r.on_hover_ui(|ui| {
        if let Some(first) = &first {
            ui.label(first);
        }
        if !hover.is_empty() {
            ui.label(hover);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_stand_at_sixty_degrees() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(300.0, 120.0));
        let [tl, tr, r, br, bl, l] = points(rect);
        let angle = |a: Pos2, b: Pos2| ((b.y - a.y).abs()).atan2((b.x - a.x).abs()).to_degrees();
        for (a, b) in [(l, tl), (tr, r), (r, br), (bl, l)] {
            assert!((angle(a, b) - 60.0).abs() < 0.01);
        }
        assert_eq!((tl.y, br.y), (rect.top(), rect.bottom()));
    }

    #[test]
    fn span_narrows_toward_top_and_bottom() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 120.0));
        let s = slant(120.0);
        assert_eq!(span_at(rect, 60.0), Rangef::new(0.0, 300.0));
        let top = span_at(rect, 0.0);
        assert!((top.min - s).abs() < 1e-3 && (top.max - (300.0 - s)).abs() < 1e-3);
        let quarter = span_at(rect, 90.0);
        assert!((quarter.min - s / 2.0).abs() < 1e-3);
    }

    #[test]
    fn bars_run_along_the_faces_and_meet_at_the_vertex() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 160.0));
        let reach = bar_reach(18.0);
        assert!((reach * (3f32.sqrt() / 2.0) - 18.0).abs() < 1e-3);
        for face in HexFace::ALL {
            let [vertex, end, end_in, vertex_in] = bar(rect, face, 18.0, 26.0, 136.0);
            // the outer side is the face itself
            assert!(
                (span_at(rect, vertex.y).min - vertex.x).abs() < 1e-3
                    || (span_at(rect, vertex.y).max - vertex.x).abs() < 1e-3
            );
            assert_eq!(vertex.y, 80.0, "{face:?} starts at the vertex's height");
            assert!(
                end.y == 26.0 || end.y == 136.0,
                "{face:?} ends at the header or the footer"
            );
            // level ends, the inner side parallel to the face, inside the hexagon
            assert_eq!((end_in.y, vertex_in.y), (end.y, vertex.y));
            assert!(((end_in.x - end.x).abs() - reach).abs() < 1e-3);
            assert!(((vertex_in.x - vertex.x).abs() - reach).abs() < 1e-3);
            for p in [end_in, vertex_in] {
                assert!(span_at(rect, p.y).contains(p.x), "{face:?} {p:?}");
            }
        }
    }

    /// The inverse rotation: a point of the turned box is found where it was put, a point beside it is not.
    #[test]
    fn a_turned_box_is_hit_through_the_rotation() {
        let along = vec2(0.5, -3f32.sqrt() / 2.0);
        let turned = Turned {
            origin: pos2(100.0, 200.0),
            along,
            down: vec2(-along.y, along.x),
        };
        let p = turned.at(30.0, 3.0);
        let l = turned.local(p);
        assert!((l.x - 30.0).abs() < 1e-3 && (l.y - 3.0).abs() < 1e-3);
        assert!(turned.holds(p, 20.0, 20.0, 12.0));
        assert!(
            !turned.holds(turned.at(30.0, 9.0), 20.0, 20.0, 12.0),
            "beside it"
        );
        assert!(
            !turned.holds(turned.at(45.0, 0.0), 20.0, 20.0, 12.0),
            "past its end"
        );
        // a corner of the upright rectangle around the box is not on the box
        let around = Rect::from_points(&turned.quad(20.0, 20.0, 12.0));
        assert!(!turned.holds(around.left_top(), 20.0, 20.0, 12.0));
    }

    #[test]
    fn wings_narrow_to_the_vertex_and_hold_their_rectangle() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 160.0));
        let s = slant(160.0);
        let [left, right] = wings(rect, 26.0, 136.0, Rangef::new(s, 300.0 - s));
        for wing in [&left, &right] {
            assert_eq!(wing.polygon.len(), 5);
            assert!(wing.span_at(10.0).is_none() && wing.span_at(150.0).is_none());
            let mid = wing.span_at(80.0).expect("widest at the vertex").span();
            let high = wing.span_at(40.0).expect("inside").span();
            assert!(mid > high && high > 0.0, "{mid} {high}");
            let inner = wing.inscribed();
            assert!(inner.width() > 10.0 && inner.height() > 20.0, "{inner:?}");
            for corner in [
                inner.left_top(),
                inner.right_top(),
                inner.left_bottom(),
                inner.right_bottom(),
            ] {
                assert!(wing.contains(corner), "{corner:?} in {:?}", wing.polygon);
            }
            // inside the hexagon: never past the slanted faces
            for p in &wing.polygon {
                assert!(span_at(rect, p.y).contains(p.x), "{p:?}");
            }
        }
        assert!(left.rect.right() <= s + 0.01 && right.rect.left() >= 300.0 - s - 0.01);
        assert!(!left.contains(pos2(150.0, 80.0)));
    }
}
