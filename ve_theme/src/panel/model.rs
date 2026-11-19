//! What a side panel shows, built fresh each frame by the app's closure: groups of rows, rows with children,
//! and the filter's view of them.

use std::borrow::Cow;
use std::hash::Hash;
use std::sync::Arc;

use egui::{Color32, Id, Painter, Rect};

use crate::Tone;

/// What keys a row: any small value that compares, hashes and prints (an enum, an id, a path).
pub trait RowKey: Clone + PartialEq + Hash + std::fmt::Debug {}

impl<T: Clone + PartialEq + Hash + std::fmt::Debug> RowKey for T {}

/// A painter for an icon: draws into the square it is given, in the colour it is given.
pub type IconPainter = Arc<dyn Fn(&Painter, Rect, Color32) + Send + Sync>;

/// A row's, group's or view's icon: a glyph of the monospace font stack, or a painter of the app's own.
#[derive(Clone)]
pub enum Icon {
    /// A glyph (`"∿"`, `"⚙"`), drawn centred in the icon square.
    Glyph(Cow<'static, str>),
    /// The app paints it into the icon square, in the colour given.
    Paint(IconPainter),
}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Glyph(g) => write!(f, "Icon::Glyph({g:?})"),
            Self::Paint(_) => f.write_str("Icon::Paint(..)"),
        }
    }
}

impl From<&'static str> for Icon {
    fn from(glyph: &'static str) -> Self {
        Self::Glyph(Cow::Borrowed(glyph))
    }
}

impl From<String> for Icon {
    fn from(glyph: String) -> Self {
        Self::Glyph(Cow::Owned(glyph))
    }
}

/// A count at a row's right end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// A plain mono number: how many there are.
    Plain(u32),
    /// Unread or new: a `primary` pill, summed on the rail.
    Unread(u32),
}

/// One row: icon, label, then right-aligned a mono value, a status light and a count. Rows nest
/// ([`Row::child`]); a row with children folds.
#[derive(Clone, Debug)]
pub struct Row<K> {
    pub(crate) key: K,
    pub(crate) label: String,
    pub(crate) icon: Option<Icon>,
    pub(crate) value: Option<String>,
    pub(crate) light: Option<Tone>,
    pub(crate) count: Option<Count>,
    pub(crate) selected: bool,
    pub(crate) tooltip: Option<String>,
    pub(crate) disabled_because: Option<String>,
    pub(crate) default_open: bool,
    pub(crate) children: Vec<Row<K>>,
}

impl<K: RowKey> Row<K> {
    /// A row for `key` reading `label` (cut with an ellipsis when long, whole in its tooltip).
    pub fn new(key: K, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
            icon: None,
            value: None,
            light: None,
            count: None,
            selected: false,
            tooltip: None,
            disabled_because: None,
            default_open: true,
            children: Vec::new(),
        }
    }

    /// The icon before the label.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A mono value at the right: a reading, an id, a unit.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// A status light in `tone`, after the value; [`Tone::Off`] draws it unlit.
    pub fn light(mut self, tone: Tone) -> Self {
        self.light = Some(tone);
        self
    }

    /// A count at the right end.
    pub fn count(mut self, count: Count) -> Self {
        self.count = Some(count);
        self
    }

    /// Drawn as the selected row: `selection` fill and a `focus` bar on the inner left.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// More about the row, on hover, under its whole label.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Greyed and not clickable when `why` is `Some`; the tooltip says why and what enables it.
    pub fn disabled_because(mut self, why: Option<impl Into<String>>) -> Self {
        self.disabled_because = why.map(Into::into);
        self
    }

    /// Whether its children show before the person folds or unfolds it (default open).
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// A child row, one indent deeper.
    pub fn child(mut self, row: Self) -> Self {
        self.children.push(row);
        self
    }

    /// Child rows, one indent deeper.
    pub fn children(mut self, rows: impl IntoIterator<Item = Self>) -> Self {
        self.children.extend(rows);
        self
    }

    pub(crate) fn id(&self, salt: Id) -> Id {
        salt.with(&self.key)
    }

    /// The key its fold is saved under: a stable hash of the row's key.
    pub(crate) fn fold_key(&self) -> u64 {
        stable_hash(&("row", &self.key))
    }

    fn count_all(&self) -> usize {
        1 + self.children.iter().map(Self::count_all).sum::<usize>()
    }
}

/// A group of rows under a caption in capitals with its count; it folds. On the rail a group is one icon with
/// the brightest light and the unread total of its rows.
#[derive(Clone, Debug)]
pub struct Group<K> {
    pub(crate) title: Option<String>,
    pub(crate) icon: Option<Icon>,
    pub(crate) summary: Option<String>,
    pub(crate) key_hint: Option<String>,
    pub(crate) key: Option<u64>,
    pub(crate) default_open: bool,
    pub(crate) rows: Vec<Row<K>>,
}

impl<K: RowKey> Group<K> {
    /// A group titled `title` (drawn in capitals).
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            icon: None,
            summary: None,
            key_hint: None,
            key: None,
            default_open: true,
            rows: Vec::new(),
        }
    }

    /// What keys the group's fold (default its title): set it when two groups share a title or the title
    /// changes while the group stays.
    pub fn key(mut self, key: impl Hash) -> Self {
        self.key = Some(stable_hash(&key));
        self
    }

    /// Rows without a caption.
    pub fn untitled() -> Self {
        Self {
            title: None,
            ..Self::new("")
        }
    }

    /// The group's icon on the rail (default: its first row's icon).
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A line under the group's name in the rail's hover card: "38 transfers · 2 warnings".
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// The key that reaches this group, shown as a chip in the rail's hover card ("Ctrl+3").
    pub fn key_hint(mut self, key: impl Into<String>) -> Self {
        self.key_hint = Some(key.into());
        self
    }

    /// Whether it shows its rows before the person folds or unfolds it (default open).
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Add a row.
    pub fn row(&mut self, row: Row<K>) -> &mut Self {
        self.rows.push(row);
        self
    }

    /// The key its fold is saved under: its own key, else a stable hash of its title; only an untitled group
    /// falls back to its place.
    pub(crate) fn fold_key(&self, index: usize) -> u64 {
        match (self.key, &self.title) {
            (Some(k), _) => k,
            (None, Some(title)) => stable_hash(&("group", title)),
            (None, None) => stable_hash(&("untitled group", index)),
        }
    }

    /// Every row in the group, children included.
    pub(crate) fn count_all(&self) -> usize {
        self.rows.iter().map(Row::count_all).sum()
    }

    /// The rail's icon: the group's, else its first row's.
    pub(crate) fn rail_icon(&self) -> Option<&Icon> {
        self.icon
            .as_ref()
            .or_else(|| self.rows.iter().find_map(|r| r.icon.as_ref()))
    }

    /// The light the rail shows: the most urgent tone among the rows, `Off` only when every light is off.
    pub(crate) fn rail_light(&self) -> Option<Tone> {
        fn rank(t: Tone) -> u8 {
            match t {
                Tone::Crit => 5,
                Tone::Warn => 4,
                Tone::Accent => 3,
                Tone::Good => 2,
                Tone::Neutral => 1,
                Tone::Off => 0,
            }
        }
        fn walk(rows: &[Row<impl Clone>], best: &mut Option<Tone>) {
            for r in rows {
                if let Some(t) = r.light
                    && best.is_none_or(|b| rank(t) > rank(b))
                {
                    *best = Some(t);
                }
                walk(&r.children, best);
            }
        }
        let mut best = None;
        walk(&self.rows, &mut best);
        best
    }

    /// The unread total of the rows.
    pub(crate) fn unread(&self) -> u32 {
        fn walk(rows: &[Row<impl Clone>]) -> u32 {
            rows.iter()
                .map(|r| {
                    let own = match r.count {
                        Some(Count::Unread(n)) => n,
                        _ => 0,
                    };
                    own.saturating_add(walk(&r.children))
                })
                .fold(0, u32::saturating_add)
        }
        walk(&self.rows)
    }
}

/// What the app's closure fills: the groups of the panel, and the view of a panel with several (form B1).
pub struct PanelBuilder<K> {
    pub(crate) groups: Vec<Group<K>>,
    pub(crate) view: usize,
}

impl<K: RowKey> PanelBuilder<K> {
    pub(crate) fn new(view: usize) -> Self {
        Self {
            groups: Vec::new(),
            view,
        }
    }

    /// Add `group` with the rows `add` gives it. Past [`GROUPS_MAX`] groups the rest are dropped.
    pub fn group(&mut self, group: Group<K>, add: impl FnOnce(&mut Group<K>)) {
        if self.groups.len() >= GROUPS_MAX {
            return;
        }
        let mut group = group;
        add(&mut group);
        self.groups.push(group);
    }

    /// Add a row outside any group (into an untitled group, after the last group when that one is untitled).
    pub fn row(&mut self, row: Row<K>) {
        match self.groups.last_mut() {
            Some(g) if g.title.is_none() => {
                g.rows.push(row);
            }
            _ => {
                let mut g = Group::untitled();
                g.rows.push(row);
                self.groups.push(g);
            }
        }
    }

    /// The panel view picked on the views rail (form B1); 0 without views.
    pub fn view(&self) -> usize {
        self.view
    }
}

/// The byte ranges of `label` that match the filter `query`: case-insensitive, every occurrence. Empty when the
/// query is empty or does not occur.
pub(crate) fn matches(label: &str, query: &str) -> Vec<std::ops::Range<usize>> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let lower = label.to_lowercase();
    let q = query.to_lowercase();
    if lower.len() != label.len() {
        // A case fold that changes byte lengths: match, but mark the whole label rather than misplace a mark.
        return if lower.contains(&q) {
            std::iter::once(0..label.len()).collect()
        } else {
            Vec::new()
        };
    }
    lower
        .match_indices(&q)
        .map(|(at, m)| at..at + m.len())
        .collect()
}

/// The filter's query, folded once per frame: matching a label allocates nothing when both are ASCII.
pub(crate) struct Query {
    lower: String,
}

impl Query {
    /// `text` trimmed and lower-cased.
    pub(crate) fn new(text: &str) -> Self {
        Self {
            lower: text.trim().to_lowercase(),
        }
    }

    /// No filter.
    pub(crate) fn is_empty(&self) -> bool {
        self.lower.is_empty()
    }

    /// The query as typed, folded.
    pub(crate) fn text(&self) -> &str {
        &self.lower
    }

    /// Whether `label` holds the query, ignoring case.
    pub(crate) fn hit(&self, label: &str) -> bool {
        let q = self.lower.as_bytes();
        if q.is_empty() {
            return true;
        }
        if label.is_ascii() && self.lower.is_ascii() {
            return label
                .as_bytes()
                .windows(q.len())
                .any(|w| w.eq_ignore_ascii_case(q));
        }
        label.to_lowercase().contains(&self.lower)
    }
}

/// Groups a panel takes at most; the rest are dropped.
pub const GROUPS_MAX: usize = 1024;

/// A stable 64-bit FNV-1a hash of `value`: the same across runs, egui versions and Rust versions, for keys
/// that are saved.
pub(crate) fn stable_hash(value: &impl Hash) -> u64 {
    struct Fnv(u64);
    impl std::hash::Hasher for Fnv {
        fn finish(&self) -> u64 {
            self.0
        }
        fn write(&mut self, bytes: &[u8]) {
            for b in bytes {
                self.0 ^= u64::from(*b);
                self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    value.hash(&mut h);
    std::hash::Hasher::finish(&h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_are_case_insensitive_byte_ranges() {
        assert_eq!(matches("phase current", "PH"), vec![0..2]);
        assert_eq!(matches("Phaser phase", "ph"), vec![0..2, 7..9]);
        assert!(matches("vbus", "ph").is_empty());
        assert!(matches("vbus", "  ").is_empty());
    }

    #[test]
    fn query_ignores_case_and_allocates_only_for_non_ascii() {
        let q = Query::new(" PH ");
        assert!(q.hit("phase current") && q.hit("Phaser") && !q.hit("vbus"));
        assert!(Query::new("ü").hit("Über"));
        assert!(Query::new("").hit("anything"));
    }

    /// A group's fold key follows its title or key, never its place; a row's follows its key.
    #[test]
    fn fold_keys_are_stable() {
        let a = Group::<u32>::new("Beta");
        assert_eq!(a.fold_key(0), a.fold_key(5));
        assert_ne!(a.fold_key(0), Group::<u32>::new("Alpha").fold_key(0));
        assert_eq!(
            Group::<u32>::new("x").key("k").fold_key(1),
            Group::<u32>::new("y").key("k").fold_key(2)
        );
        assert_eq!(
            Row::new(7u32, "a").fold_key(),
            Row::new(7u32, "b").fold_key()
        );
        // Pinned: a saved fold must still match after an egui or Rust update.
        assert_eq!(stable_hash(&"beta"), 0x0b0b_335e_a628_f588);
    }

    #[test]
    fn rail_takes_the_most_urgent_light_and_sums_unread() {
        let mut g = Group::new("dev0");
        g.row(Row::new(1, "a").light(Tone::Good))
            .row(
                Row::new(2, "b")
                    .light(Tone::Off)
                    .child(Row::new(3, "c").light(Tone::Warn).count(Count::Unread(2))),
            )
            .row(
                Row::new(4, "d")
                    .count(Count::Unread(3))
                    .count(Count::Unread(1)),
            );
        assert_eq!(g.rail_light(), Some(Tone::Warn));
        assert_eq!(g.unread(), 3);
        assert_eq!(g.count_all(), 4);
    }
}
