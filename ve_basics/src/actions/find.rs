//! Lookups the palette and the menus make: fuzzy match by title and id, the menu tree from the groups.

use super::Actions;

/// One palette result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The action's id.
    pub id: &'static str,
    /// Higher is better; results come sorted by it.
    pub score: i32,
    /// Indices of the title's chars the query matched, for marking them; empty when it matched the id.
    pub title_hits: Vec<usize>,
}

/// An entry of a menu: an action, or a submenu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuItem {
    /// The action with this id.
    Action(&'static str),
    /// A submenu.
    Menu(MenuNode),
}

/// A menu with its entries in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuNode {
    /// The menu's name, the last part of the group path.
    pub name: String,
    /// Entries by order, then title; a submenu sits where its first entry would.
    pub items: Vec<MenuItem>,
}

/// Longest query looked at; the rest is ignored.
pub const QUERY_CHARS_MAX: usize = 128;

/// A match on the id instead of the title counts this much less.
const ID_MATCH_PENALTY: i32 = 10;

impl Actions {
    /// The palette's lookup: actions whose title or id contain every word of `query` as a subsequence, best
    /// first, at most `limit`. Disabled actions are found too (the palette shows why they cannot run). An empty
    /// query lists them as the menus do, then those in no menu by title.
    pub fn find(&self, query: &str, limit: usize) -> Vec<Found> {
        let query: String = query.chars().take(QUERY_CHARS_MAX).collect();
        let terms: Vec<Vec<char>> = query
            .split_whitespace()
            .map(|t| t.chars().map(fold).collect())
            .collect();
        if terms.is_empty() {
            return self
                .menu_order_ids()
                .into_iter()
                .take(limit)
                .map(|id| Found {
                    id,
                    score: 0,
                    title_hits: Vec::new(),
                })
                .collect();
        }
        let mut found: Vec<(Found, &'static str)> = Vec::new();
        for e in &self.entries {
            let a = &e.action;
            let by_title = match_terms(a.title, &terms);
            let by_id = match_terms(a.id, &terms).map(|(s, _)| (s - ID_MATCH_PENALTY, Vec::new()));
            let best = match (by_title, by_id) {
                (Some(t), Some(i)) => Some(if i.0 > t.0 { i } else { t }),
                (t, i) => t.or(i),
            };
            if let Some((score, title_hits)) = best {
                found.push((
                    Found {
                        id: a.id,
                        score,
                        title_hits,
                    },
                    a.title,
                ));
            }
        }
        found.sort_by(|(a, at), (b, bt)| {
            b.score.cmp(&a.score).then(at.cmp(bt)).then(a.id.cmp(b.id))
        });
        found.into_iter().take(limit).map(|(f, _)| f).collect()
    }

    /// The menus: every action with a group, under its group path (`View/Panels` is the submenu Panels of View).
    /// Top-level menus come in [`Actions::menu_order`], then by name; destructive actions come last in theirs.
    pub fn menu_tree(&self) -> Vec<MenuNode> {
        let mut root = Building::default();
        for e in &self.entries {
            let a = &e.action;
            if e.menu.is_empty() {
                continue;
            }
            let mut node = &mut root;
            for part in &e.menu {
                let at = match node.children.iter().position(|c| &c.name == part) {
                    Some(at) => at,
                    None => {
                        node.children.push(Building {
                            name: part.to_owned(),
                            ..Building::default()
                        });
                        node.children.len() - 1
                    }
                };
                node = &mut node.children[at];
            }
            node.actions.push((a.destructive, a.order, a.title, a.id));
        }
        let mut menus: Vec<(MenuNode, i32)> =
            root.children.into_iter().map(Building::finish).collect();
        menus.sort_by(|(a, _), (b, _)| {
            self.menu_rank(&a.name)
                .cmp(&self.menu_rank(&b.name))
                .then(a.name.cmp(&b.name))
        });
        menus.into_iter().map(|(m, _)| m).collect()
    }

    /// Every id as the menus list them, then the actions in no menu by title.
    fn menu_order_ids(&self) -> Vec<&'static str> {
        fn walk(items: &[MenuItem], out: &mut Vec<&'static str>) {
            for item in items {
                match item {
                    MenuItem::Action(id) => out.push(id),
                    MenuItem::Menu(menu) => walk(&menu.items, out),
                }
            }
        }
        let mut ids = Vec::with_capacity(self.entries.len());
        for menu in self.menu_tree() {
            walk(&menu.items, &mut ids);
        }
        let mut rest: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.menu.is_empty())
            .map(|e| &e.action)
            .collect();
        rest.sort_by_key(|a| (a.title, a.id));
        ids.extend(rest.into_iter().map(|a| a.id));
        ids
    }

    fn menu_rank(&self, name: &str) -> usize {
        self.menu_order
            .iter()
            .position(|m| m == name)
            .unwrap_or(usize::MAX)
    }
}

#[derive(Default)]
struct Building {
    name: String,
    actions: Vec<(bool, i32, &'static str, &'static str)>,
    children: Vec<Building>,
}

impl Building {
    /// The finished menu and its lowest order, where it sits in its parent.
    fn finish(self) -> (MenuNode, i32) {
        let mut items: Vec<(bool, i32, String, MenuItem)> = self
            .actions
            .into_iter()
            .map(|(destructive, order, title, id)| {
                (destructive, order, title.to_owned(), MenuItem::Action(id))
            })
            .collect();
        for child in self.children {
            let (menu, order) = child.finish();
            items.push((false, order, menu.name.clone(), MenuItem::Menu(menu)));
        }
        items.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
        let first = items.iter().map(|i| i.1).min().unwrap_or(0);
        (
            MenuNode {
                name: self.name,
                items: items.into_iter().map(|(_, _, _, item)| item).collect(),
            },
            first,
        )
    }
}

/// Every term matched in `text`, each anywhere: the summed score and the matched char indices, sorted.
fn match_terms(text: &str, terms: &[Vec<char>]) -> Option<(i32, Vec<usize>)> {
    let chars: Vec<char> = text.chars().map(fold).collect();
    let mut score = 0;
    let mut hits = Vec::new();
    for term in terms {
        let (s, h) = match_term(&chars, term)?;
        score += s;
        hits.extend(h);
    }
    hits.sort_unstable();
    hits.dedup();
    Some((score, hits))
}

/// The best placement of one term as a subsequence: every start tried, the rest matched greedily. Runs of
/// matched chars, word starts and an early start score; skipped chars cost.
fn match_term(chars: &[char], term: &[char]) -> Option<(i32, Vec<usize>)> {
    let first = *term.first()?;
    let mut best: Option<(i32, Vec<usize>)> = None;
    for start in (0..chars.len()).filter(|&i| chars[i] == first) {
        let mut hits = vec![start];
        let mut at = start + 1;
        for &c in &term[1..] {
            match chars[at..].iter().position(|&x| x == c) {
                Some(p) => {
                    hits.push(at + p);
                    at += p + 1;
                }
                None => break,
            }
        }
        if hits.len() < term.len() {
            break;
        }
        let score = score_hits(chars, &hits);
        if best.as_ref().is_none_or(|(b, _)| score > *b) {
            best = Some((score, hits));
        }
    }
    best
}

fn score_hits(chars: &[char], hits: &[usize]) -> i32 {
    let mut score = 0;
    for (n, &i) in hits.iter().enumerate() {
        score += 1;
        if i == 0 || matches!(chars[i - 1], ' ' | '.' | '_' | '-' | '/') {
            score += 8;
        }
        if n > 0 {
            let gap = i - hits[n - 1] - 1;
            score += if gap == 0 { 5 } else { -(gap.min(10) as i32) };
        }
    }
    if hits.first() == Some(&0) {
        score += 10;
    }
    score
}

/// One char folded for matching, the same on the query and the text, so indices stay char indices: its first
/// lower-case char (`İ` to `i`), final sigma as sigma.
fn fold(c: char) -> char {
    match c.to_lowercase().next().unwrap_or(c) {
        'ς' => 'σ',
        l => l,
    }
}
