//! The panel's list: group heads, rows with their folds and right-hand parts, the filter's marks and the
//! "Not matching" fold.
//!
//! The model is flattened once per frame into items of one height ([`flatten`]: one pass, the filter
//! included); only the items in sight are laid out, interacted with and given to accessibility ([`draw`]), so
//! a frame's drawing cost does not grow with the row count.

use std::ops::Range;
use std::sync::Arc;

use egui::{
    Align, Color32, CursorIcon, Id, Key, Modifiers, Rect, Response, ScrollArea, Sense, Ui, Vec2,
    WidgetInfo, WidgetType, pos2, vec2,
};

use super::PanelResponse;
use super::model::{Count, Group, Query, Row, RowKey, matches};
use super::paint::{
    self, fill, focus_ring, galley_at, left_bar, line, mono_font, snap_pos, text_font,
};
use super::reveal::RevealLink;
use super::state::SidePanelState;
use crate::{LightStyle, Tokens, Tone};

/// Items (heads and rows) one frame flattens at most; past it the rest are counted, not shown.
pub const ROWS_MAX: usize = 1_000_000;

/// The app's context menu for a row, borrowed for one frame.
pub(crate) type RowMenuRef<'a, K> = &'a mut dyn FnMut(&mut Ui, &K);

/// One line of the flattened list.
pub(crate) enum Item<'r, K> {
    /// A group's head, or the "Not matching" head (`misses`).
    Head {
        key: u64,
        title: &'r str,
        count: String,
        open: bool,
        misses: bool,
    },
    /// A row at `depth`; `fold` is `Some(open)` when it has children; `muted` under "Not matching".
    Row {
        row: &'r Row<K>,
        depth: u16,
        fold: Option<bool>,
        muted: bool,
    },
}

/// The list as drawn this frame.
pub(crate) struct Flat<'r, K> {
    pub items: Vec<Item<'r, K>>,
    /// Rows in the model, children included.
    pub total: usize,
    /// Rows whose label holds the filter.
    pub matched: usize,
    /// Items past [`ROWS_MAX`], not shown.
    pub cut: usize,
    /// Each group's head (or first row) by group index.
    pub group_at: Vec<usize>,
}

/// Flatten `groups` for `query` and the folds in `state`, in one pass over the rows.
pub(crate) fn flatten<'r, K: RowKey>(
    groups: &'r [Group<K>],
    query: &Query,
    state: &SidePanelState,
) -> Flat<'r, K> {
    flatten_capped(groups, query, state, ROWS_MAX)
}

/// [`flatten`] with at most `max` items.
fn flatten_capped<'r, K: RowKey>(
    groups: &'r [Group<K>],
    query: &Query,
    state: &SidePanelState,
    max: usize,
) -> Flat<'r, K> {
    let mut b = Builder {
        max,
        items: Vec::new(),
        misses: Vec::new(),
        matched: 0,
        cut: 0,
        query,
        state,
    };
    let mut total = 0;
    let mut group_at = Vec::with_capacity(groups.len());
    for (gi, g) in groups.iter().enumerate() {
        group_at.push(b.items.len());
        let n = g.count_all();
        total += n;
        let key = g.fold_key(gi);
        let Some(title) = &g.title else {
            for row in &g.rows {
                b.walk(row, 0);
            }
            continue;
        };
        if query.is_empty() {
            let open = state.is_open(key, g.default_open);
            b.head(key, title, n.to_string(), open, false);
            if open {
                for row in &g.rows {
                    b.walk(row, 0);
                }
            }
        } else {
            let head_at = b.items.len();
            let before = b.matched;
            b.head(key, title, String::new(), false, false);
            for row in &g.rows {
                b.walk(row, 0);
            }
            let m = b.matched - before;
            if let Some(Item::Head { count, open, .. }) = b.items.get_mut(head_at) {
                *count = format!("{m} of {n}");
                *open = m > 0;
            }
        }
    }
    if !query.is_empty() && !b.misses.is_empty() {
        let open = state.misses_open;
        let misses = std::mem::take(&mut b.misses);
        b.head(0, "Not matching", misses.len().to_string(), open, true);
        if open {
            for row in misses {
                b.add(Item::Row {
                    row,
                    depth: 0,
                    fold: None,
                    muted: true,
                });
            }
        }
    }
    Flat {
        items: b.items,
        total,
        matched: b.matched,
        cut: b.cut,
        group_at,
    }
}

struct Builder<'r, 'q, K> {
    max: usize,
    items: Vec<Item<'r, K>>,
    misses: Vec<&'r Row<K>>,
    matched: usize,
    cut: usize,
    query: &'q Query,
    state: &'q SidePanelState,
}

impl<'r, K: RowKey> Builder<'r, '_, K> {
    fn add(&mut self, item: Item<'r, K>) {
        if self.items.len() < self.max {
            self.items.push(item);
        } else {
            self.cut += 1;
        }
    }

    fn head(&mut self, key: u64, title: &'r str, count: String, open: bool, misses: bool) {
        self.add(Item::Head {
            key,
            title,
            count,
            open,
            misses,
        });
    }

    /// Add `row` and what of its subtree shows; returns whether it shows. Filtering, a row shows when its label
    /// holds the query or a child shows (then it is open); the rest go to the misses.
    fn walk(&mut self, row: &'r Row<K>, depth: u16) -> bool {
        let has_children = !row.children.is_empty();
        if self.query.is_empty() {
            let open = self.state.is_open(row.fold_key(), row.default_open);
            self.add(Item::Row {
                row,
                depth,
                fold: has_children.then_some(open),
                muted: false,
            });
            if open {
                for c in &row.children {
                    self.walk(c, depth.saturating_add(1));
                }
            }
            return true;
        }
        let at = self.items.len();
        let cut = self.cut;
        let misses = self.misses.len();
        self.add(Item::Row {
            row,
            depth,
            fold: None,
            muted: false,
        });
        let hit = self.query.hit(&row.label);
        let mut child_shows = false;
        for c in &row.children {
            child_shows |= self.walk(c, depth.saturating_add(1));
        }
        if hit || child_shows {
            self.matched += usize::from(hit);
            if let Some(Item::Row { fold, .. }) = self.items.get_mut(at) {
                *fold = child_shows.then_some(true);
            }
            return true;
        }
        self.items.truncate(at);
        self.cut = cut;
        self.misses.truncate(misses);
        self.miss(row);
        false
    }

    fn miss(&mut self, row: &'r Row<K>) {
        self.misses.push(row);
        for c in &row.children {
            self.miss(c);
        }
    }
}

/// What drawing the list needs besides the rows.
pub(crate) struct ListCx<'a, K> {
    pub t: Arc<Tokens>,
    pub id: Id,
    pub query: Query,
    pub lights: LightStyle,
    pub state: &'a mut SidePanelState,
    pub link: Option<&'a mut RevealLink<K>>,
    /// A row to scroll to and flash (revealed from the view).
    pub reveal: Option<K>,
    /// A group to scroll to (picked on the rail).
    pub scroll_group: Option<usize>,
    pub row_menu: Option<RowMenuRef<'a, K>>,
    pub out: &'a mut PanelResponse<K>,
    /// The first row in sight.
    pub top: Option<K>,
    /// The row the tree should keep in sight (following the view), scrolled to once per change.
    pub follow: Option<K>,
}

/// Where to scroll before drawing: an item index and how to align it (`None`: only when out of sight).
fn target<K: RowKey>(cx: &ListCx<'_, K>, flat: &Flat<'_, K>) -> Option<(usize, Option<Align>)> {
    let find = |key: &K| {
        flat.items
            .iter()
            .position(|it| matches!(it, Item::Row { row, .. } if &row.key == key))
    };
    if let Some(i) = cx.reveal.as_ref().and_then(find) {
        return Some((i, Some(Align::Center)));
    }
    if let Some(&i) = cx.scroll_group.and_then(|g| flat.group_at.get(g)) {
        return Some((i, Some(Align::TOP)));
    }
    cx.follow.as_ref().and_then(find).map(|i| (i, None))
}

/// Draw the items in sight of `ui`'s rectangle, scrolled to a reveal, a rail pick or the followed row.
pub(crate) fn draw<K: RowKey>(ui: &mut Ui, cx: &mut ListCx<'_, K>, flat: &Flat<'_, K>) {
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let h = cx.t.chrome.row;
    let view_h = ui.available_height();
    let offset_id = cx.id.with("list offset");
    let last = ui.data(|d| d.get_temp::<f32>(offset_id)).unwrap_or(0.0);
    let mut area = ScrollArea::vertical()
        .id_salt(cx.id.with("list"))
        .auto_shrink([false, false]);
    if let Some((i, align)) = target(cx, flat) {
        let y = i as f32 * h;
        let centred = (y - (view_h - h) * 0.5).max(0.0);
        let to = match align {
            Some(Align::TOP) => Some(y),
            Some(_) => Some(centred),
            None if y < last || y + h > last + view_h => Some(centred),
            None => None,
        };
        if let Some(to) = to {
            area = area.vertical_scroll_offset(to);
        }
        cx.reveal = None;
        cx.follow = None;
    }
    let out = area.show_rows(ui, h, flat.items.len(), |ui, range| {
        for i in range {
            match &flat.items[i] {
                Item::Head {
                    key,
                    title,
                    count,
                    open,
                    misses,
                } => {
                    let r = head(ui, &cx.t, cx.id.with(key), title, count, *open);
                    if toggled(ui, &r, *open) {
                        if *misses {
                            cx.state.misses_open = !open;
                        } else if cx.query.is_empty() {
                            cx.state.set_open(*key, !open);
                        }
                    }
                }
                Item::Row {
                    row,
                    depth,
                    fold,
                    muted,
                } => tree_row(ui, cx, row, *depth, *fold, *muted),
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(offset_id, out.state.offset.y));
}

/// Whether a fold's head was clicked, or ← / → asked to fold or open it while it has focus.
fn toggled(ui: &mut Ui, r: &Response, open: bool) -> bool {
    r.clicked() || toggled_by_key(ui, r, open)
}

/// Whether ← (open) or → (folded) was pressed while `r` has focus.
fn toggled_by_key(ui: &mut Ui, r: &Response, open: bool) -> bool {
    let key = if open {
        Key::ArrowLeft
    } else {
        Key::ArrowRight
    };
    r.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, key))
}

/// A row: drawn, folded by its chevron or ← →, picked by a click elsewhere.
fn tree_row<K: RowKey>(
    ui: &mut Ui,
    cx: &mut ListCx<'_, K>,
    row: &Row<K>,
    depth: u16,
    fold: Option<bool>,
    muted: bool,
) {
    let marks = if cx.query.is_empty() {
        Vec::new()
    } else {
        matches(&row.label, cx.query.text())
    };
    let r = row_ui(ui, cx, row, depth, fold, &marks, muted);
    let on_chevron = r
        .interact_pointer_pos()
        .is_some_and(|p| p.x < chevron_rect(&cx.t, r.rect, depth).right());
    if let Some(open) = fold
        && cx.query.is_empty()
        && (toggled_by_key(ui, &r, open) || (r.clicked() && on_chevron))
    {
        cx.state.set_open(row.fold_key(), !open);
    }
    after_row(ui, cx, row, &r, !on_chevron);
}

/// Report a row's click (unless it landed on the chevron, `picks` false), double click, hover; the first row
/// in sight; its context menu.
fn after_row<K: RowKey>(ui: &Ui, cx: &mut ListCx<'_, K>, row: &Row<K>, r: &Response, picks: bool) {
    if picks && r.clicked() {
        cx.out.clicked = Some(row.key.clone());
        if let Some(link) = cx.link.as_deref_mut() {
            link.reveal_in_view(row.key.clone());
        }
    }
    if r.double_clicked() {
        cx.out.double_clicked = Some(row.key.clone());
    }
    if r.hovered() {
        cx.out.hovered = Some(row.key.clone());
        if let Some(link) = cx.link.as_deref_mut() {
            link.hover(ui.ctx(), row.key.clone());
        }
    }
    if cx.top.is_none() && r.rect.bottom() > ui.clip_rect().top() {
        cx.top = Some(row.key.clone());
    }
    if let Some(menu) = cx.row_menu.as_deref_mut() {
        r.context_menu(|ui| menu(ui, &row.key));
    }
}

fn chevron_rect(t: &Tokens, rect: Rect, depth: u16) -> Rect {
    let x = rect.left() + t.space.s + f32::from(depth) * t.chrome.tree_indent;
    Rect::from_min_size(
        pos2(x, rect.top()),
        vec2(t.chrome.tree_indent, rect.height()),
    )
}

/// A group head (or the "Not matching" head): chevron, caption in capitals, count at the right.
fn head(ui: &mut Ui, t: &Tokens, id: Id, title: &str, count: &str, open: bool) -> Response {
    let rect = ui
        .allocate_space(vec2(ui.available_width(), t.chrome.row))
        .1;
    let r = ui.interact(rect, id.with("head"), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let inner = rect.shrink2(vec2(t.space.s, 0.0));
        if r.hovered() {
            fill(p, inner, t.radius.s, t.colors.hover);
        }
        let ch = chevron_rect(t, inner, 0);
        paint::text_in(
            p,
            ch,
            if open { "▾" } else { "▸" },
            mono_font(t),
            t.colors.text_muted,
        );
        let font = t.type_scale.caption.font_id();
        let cg = p.layout_no_wrap(count.to_owned(), mono_font(t), t.colors.text_muted);
        let right = inner.right() - t.space.m;
        let max = right - cg.size().x - t.space.m - ch.right();
        let g = line(p, &title.to_uppercase(), font, t.colors.title, max);
        galley_at(p, g, ch.right(), inner.center().y, t.colors.title);
        let cw = cg.size().x;
        galley_at(p, cg, right - cw, inner.center().y, t.colors.text_muted);
        if r.has_focus() {
            focus_ring(p, t, inner);
        }
    }
    let name = format!("{title}, {count}");
    r.widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, open, &name));
    r.on_hover_cursor(CursorIcon::PointingHand)
}

/// A row's ground, most important first.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ground {
    Selected,
    Linked,
    Hovered,
    Plain,
}

/// How a row's text is inked.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ink {
    Normal,
    /// Listed under "Not matching": muted, still clickable.
    Muted,
    /// Disabled: muted, its light unlit.
    Disabled,
}

/// One row; `fold` is `Some(open)` for a row with children, `marks` the filter's matches in its label.
fn row_ui<K: RowKey>(
    ui: &mut Ui,
    cx: &mut ListCx<'_, K>,
    row: &Row<K>,
    depth: u16,
    fold: Option<bool>,
    marks: &[Range<usize>],
    muted: bool,
) -> Response {
    let t = cx.t.clone();
    let enabled = ui.is_enabled() && row.disabled_because.is_none();
    let rect = ui
        .allocate_space(vec2(ui.available_width(), t.chrome.row))
        .1;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let r = ui.interact(rect, row.id(cx.id), sense);
    let mut elided = false;
    if ui.is_rect_visible(rect) {
        let inner = rect.shrink2(vec2(t.space.s, 0.0));
        let linked = cx.link.as_deref().and_then(|l| l.hovered(ui.ctx())) == Some(&row.key);
        let flash = cx.link.as_deref().map_or(0.0, |l| l.flash(ui, &row.key));
        let ground = if row.selected {
            Ground::Selected
        } else if linked {
            Ground::Linked
        } else if enabled && r.hovered() {
            Ground::Hovered
        } else {
            Ground::Plain
        };
        background(ui, &t, inner, ground, flash);
        let ink = match (enabled, muted) {
            (false, _) => Ink::Disabled,
            (true, true) => Ink::Muted,
            (true, false) => Ink::Normal,
        };
        elided = contents(ui, cx, row, inner, depth, fold, marks, ink);
        if r.has_focus() {
            focus_ring(ui.painter(), &t, inner);
        }
    }
    let label = row.label.clone();
    let selected = row.selected;
    r.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, &label));
    let tip = match (&row.disabled_because, &row.tooltip) {
        (Some(why), _) => Some(format!("{}\n{why}", row.label)),
        (None, Some(tip)) => Some(format!("{}\n{tip}", row.label)),
        (None, None) => elided.then(|| row.label.clone()),
    };
    // A disabled row senses hover only, so its tooltip is an ordinary one.
    match tip {
        Some(tip) => r.on_hover_text(tip),
        None => r,
    }
}

/// Paint a row's ground: selection with its bar, linked hover, hover, a reveal's flash over it.
fn background(ui: &Ui, t: &Tokens, rect: Rect, ground: Ground, flash: f32) {
    let p = ui.painter();
    let base = match ground {
        Ground::Selected => t.colors.selection,
        Ground::Linked => t.colors.status.info.hover,
        Ground::Hovered => t.colors.hover,
        Ground::Plain => Color32::TRANSPARENT,
    };
    fill(p, rect, t.radius.s, base);
    if flash > 0.0 {
        fill(
            p,
            rect,
            t.radius.s,
            t.colors.focus.gamma_multiply(t.chrome.flash * flash),
        );
    }
    if ground == Ground::Selected {
        left_bar(p, rect, t.stroke.thick, t.colors.focus);
    }
}

/// Chevron, icon, label with marks, then from the right: count, light, value.
/// Returns whether the label was cut.
#[expect(
    clippy::too_many_arguments,
    reason = "one row's parts, all needed to draw it"
)]
fn contents<K: RowKey>(
    ui: &Ui,
    cx: &ListCx<'_, K>,
    row: &Row<K>,
    rect: Rect,
    depth: u16,
    fold: Option<bool>,
    marks: &[Range<usize>],
    ink: Ink,
) -> bool {
    let t = &cx.t;
    let p = ui.painter();
    let cy = rect.center().y;
    let color = match ink {
        Ink::Disabled | Ink::Muted => t.colors.text_muted,
        Ink::Normal if row.selected => t.legible(t.colors.text, t.colors.selection),
        Ink::Normal => t.colors.text,
    };
    let ch = chevron_rect(t, rect, depth);
    if let Some(open) = fold {
        paint::text_in(
            p,
            ch,
            if open { "▾" } else { "▸" },
            mono_font(t),
            t.colors.text_muted,
        );
    }
    let mut x = ch.right();
    if let Some(icon) = &row.icon {
        let side = t.space.xl;
        let icon_ink = if row.selected && ink == Ink::Normal {
            color
        } else {
            t.colors.text_muted
        };
        paint::icon(p, t, icon, pos2(x + side * 0.5, cy), side, icon_ink);
        x += side + t.space.s;
    }
    let mut right = rect.right() - t.space.m;
    match row.count {
        Some(Count::Unread(n)) => right -= paint::pill(p, t, right, cy, n) + t.space.s,
        Some(Count::Plain(n)) => {
            let g = p.layout_no_wrap(n.to_string(), mono_font(t), t.colors.text_muted);
            let w = g.size().x;
            galley_at(p, g, right - w, cy, t.colors.text_muted);
            right -= w + t.space.m;
        }
        None => {}
    }
    if let Some(tone) = row.light {
        let side = cx.lights.side(t, p.pixels_per_point());
        let tone = if ink == Ink::Disabled {
            Tone::Off
        } else {
            tone
        };
        cx.lights.paint(p, t, tone, pos2(right - side * 0.5, cy));
        right -= side + t.space.m;
    }
    if let Some(v) = &row.value {
        let g = p.layout_no_wrap(v.clone(), mono_font(t), t.colors.text_muted);
        let w = g.size().x;
        galley_at(p, g, right - w, cy, t.colors.text_muted);
        right -= w + t.space.m;
    }
    let g = line(p, &row.label, text_font(t), color, right - x);
    let elided = g.elided;
    let pos = snap_pos(p, pos2(x, cy - g.size().y * 0.5));
    for m in marks {
        mark(p, t, &g, &row.label, m.clone(), pos);
    }
    p.galley(pos, g, color);
    elided
}

/// A `warning.soft` mark under the letters `range` of `label`, as laid out in `galley` at `pos`.
fn mark(
    p: &egui::Painter,
    t: &Tokens,
    galley: &egui::Galley,
    label: &str,
    range: Range<usize>,
    pos: egui::Pos2,
) {
    let chars = |b: usize| label[..b.min(label.len())].chars().count();
    let a = galley.pos_from_cursor(egui::text::CCursor::new(chars(range.start)));
    let b = galley.pos_from_cursor(egui::text::CCursor::new(chars(range.end)));
    let rect = Rect::from_min_max(pos + a.min.to_vec2(), pos + vec2(b.min.x, a.max.y));
    fill(p, rect, 0, t.colors.status.warning.soft);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<Group<u32>> {
        let mut g = Group::new("dev0");
        g.row(Row::new(1, "can0").child(Row::new(2, "state")))
            .row(
                Row::new(3, "motor")
                    .default_open(false)
                    .child(Row::new(4, "phase current"))
                    .child(Row::new(5, "vbus")),
            )
            .row(Row::new(6, "Phaser"));
        vec![g]
    }

    fn rows<K: Clone>(flat: &Flat<'_, K>) -> Vec<K> {
        flat.items
            .iter()
            .filter_map(|it| match it {
                Item::Row { row, .. } => Some(row.key.clone()),
                Item::Head { .. } => None,
            })
            .collect()
    }

    /// Unfiltered, folds hide children; filtered, a folded parent of a match opens, misses fold away.
    #[test]
    fn flatten_follows_folds_and_the_filter() {
        let g = groups();
        let state = SidePanelState::default();
        let all = flatten(&g, &Query::new(""), &state);
        assert_eq!(rows(&all), vec![1, 2, 3, 6], "motor starts folded");
        assert_eq!(all.total, 6);

        let ph = flatten(&g, &Query::new("ph"), &state);
        assert_eq!(
            rows(&ph),
            vec![3, 4, 6],
            "the folded parent of a match opens"
        );
        assert_eq!(ph.matched, 2);
        assert!(
            matches!(ph.items[0], Item::Head { ref count, open: true, .. } if count == "2 of 6")
        );
        assert!(
            matches!(ph.items.last(), Some(Item::Head { misses: true, count, .. }) if count == "3")
        );

        let open = SidePanelState {
            misses_open: true,
            ..SidePanelState::default()
        };
        let ph = flatten(&g, &Query::new("ph"), &open);
        assert_eq!(
            rows(&ph),
            vec![3, 4, 6, 1, 2, 5],
            "misses listed after, nothing vanishes"
        );
    }

    /// The flattened list is cut at its limit and counts what it left out.
    #[test]
    fn flatten_is_bounded() {
        let mut g = Group::new("many");
        for i in 0..20 {
            g.row(Row::new(i, ""));
        }
        let groups = vec![g];
        let flat = flatten_capped(&groups, &Query::new(""), &SidePanelState::default(), 5);
        assert_eq!(flat.items.len(), 5, "the head and 4 rows");
        assert_eq!(flat.cut, 16);
    }
}
