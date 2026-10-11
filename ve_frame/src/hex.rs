//! Hexagon geometry and the items on its slanted faces (FRM-3).

use egui::{Align2, FontId, Pos2, Rangef, Rect, Sense, Ui, Vec2, pos2, vec2};
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
}

/// A small thing on a slanted face of a hexagon, drawn rotated along the face: a short text or an LED.
#[derive(Clone, Debug, PartialEq)]
pub struct SideItem {
    kind: Kind,
    hover: String,
}

impl SideItem {
    /// A short text ("24 °C", "ch 2").
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            kind: Kind::Text(text.into()),
            hover: String::new(),
        }
    }

    /// A status dot. Give it a [`Self::hover`]: colour alone doesn't say what it means.
    pub fn led(status: Status) -> Self {
        Self {
            kind: Kind::Led(status),
            hover: String::new(),
        }
    }

    /// The tooltip: what the item is, its unit.
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

/// The band of `face`: the triangle between the slanted face and the body's side, where the face's items sit.
/// The first two points are the face, the third the inner corner.
pub fn band(rect: Rect, face: HexFace) -> [Pos2; 3] {
    let (from, to) = ends(rect, face);
    let cy = rect.center().y;
    let inner_x = match face {
        HexFace::UpperLeft | HexFace::LowerLeft => to.x,
        HexFace::UpperRight | HexFace::LowerRight => from.x,
    };
    [from, to, pos2(inner_x, cy)]
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

/// The two ends of `face`, ordered so that text along it reads left to right.
fn ends(rect: Rect, face: HexFace) -> (Pos2, Pos2) {
    let [tl, tr, r, br, bl, l] = points(rect);
    match face {
        HexFace::UpperLeft => (l, tl),
        HexFace::UpperRight => (tr, r),
        HexFace::LowerLeft => (l, bl),
        HexFace::LowerRight => (br, r),
    }
}

/// Distance of the items from the face.
const INSET: f32 = 6.0;
/// Free length at the end of a face on the top or bottom edge.
const EDGE: f32 = 6.0;
/// Free length at the left or right vertex, where the items of two faces would meet.
const VERTEX: f32 = 14.0;
/// Gap between items along the face.
const ITEM_GAP: f32 = 8.0;
/// Diameter of an LED item.
const DOT: f32 = 7.0;

/// Draw `items` along `face` of the hexagon in `rect`, inside it. Items that don't fit are left out.
///
/// `top_band` and `bottom_band` are the heights of the header and the footer: the part of a face beside them
/// stays free, so items never run into the header's controls.
pub(crate) fn side(
    ui: &mut Ui,
    theme: &Theme,
    rect: Rect,
    (top_band, bottom_band): (f32, f32),
    face: HexFace,
    items: &[SideItem],
) {
    let (from, to) = ends(rect, face);
    let length = (to - from).length();
    if length <= 0.0 {
        return;
    }
    let along = (to - from) / length;
    let angle = along.y.atan2(along.x);
    // where rotated text grows to ("down" of its line), and the way into the hexagon
    let down = vec2(-along.y, along.x);
    let mid = from + (to - from) * 0.5;
    let inward = if down.dot(rect.center() - mid) > 0.0 {
        down
    } else {
        -down
    };
    let font = FontId::monospace(10.0);
    let line_h = ui.fonts_mut(|f| f.row_height(&font));
    // a face's end at the top or bottom edge lies beside the header or footer: leave that part out
    let sin60 = 3f32.sqrt() / 2.0;
    let (start, end) = match face {
        HexFace::UpperLeft => (VERTEX, length - top_band / sin60),
        HexFace::UpperRight => (top_band / sin60, length - VERTEX),
        HexFace::LowerLeft => (VERTEX, length - bottom_band / sin60 - EDGE),
        HexFace::LowerRight => (bottom_band / sin60 + EDGE, length - VERTEX),
    };
    let mut at = start;
    for (i, item) in items.iter().enumerate() {
        let (width, label) = match &item.kind {
            Kind::Text(text) => {
                let galley =
                    ui.painter()
                        .layout_no_wrap(text.clone(), font.clone(), theme.text_muted);
                let width = galley.size().x;
                if at + width > end {
                    break;
                }
                let top = if inward == down {
                    INSET
                } else {
                    INSET + galley.size().y
                };
                let pos = from + along * at + inward * top;
                ui.painter().add(
                    egui::epaint::TextShape::new(pos, galley, theme.text_muted).with_angle(angle),
                );
                (width, text.clone())
            }
            Kind::Led(status) => {
                if at + DOT > end {
                    break;
                }
                let c = from + along * (at + DOT / 2.0) + inward * (INSET + line_h / 2.0);
                let color = status.color(theme);
                let painter = ui.painter();
                if *status != Status::Off {
                    painter.circle_filled(c, DOT * 0.5 + 2.5, color.gamma_multiply(0.2));
                }
                painter.circle_filled(c, DOT * 0.5, color);
                (DOT, format!("status: {}", item.hover))
            }
        };
        let center = from + along * (at + width / 2.0) + inward * (INSET + line_h / 2.0);
        let extent = Vec2::new(
            along.x.abs() * width + down.x.abs() * line_h,
            along.y.abs() * width + down.y.abs() * line_h,
        );
        let hit = Align2::CENTER_CENTER.anchor_size(center, extent);
        let r = ui.interact(hit, ui.id().with((face, i)), Sense::hover());
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.clone()));
        if !item.hover.is_empty() {
            r.on_hover_text(&item.hover);
        }
        at += width + ITEM_GAP;
    }
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

    #[test]
    fn text_on_every_face_reads_left_to_right() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 120.0));
        for face in HexFace::ALL {
            let (from, to) = ends(rect, face);
            assert!(to.x > from.x, "{face:?}");
        }
    }
}
