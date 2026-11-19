//! The link between a tree in a side panel and the view beside it (a timeline's rows, a table): they scroll
//! apart, and a click on one reveals the item in the other.
//!
//! - A click on a row in the tree reveals it in the view: the view scrolls it into sight and its row head
//!   flashes once ([`RevealLink::take_for_view`], [`RevealLink::flash`]; `_at` variants take the caller's clock).
//! - A selection in the view reveals it in the tree: the panel unfolds its parents, scrolls to it and flashes
//!   it ([`RevealLink::reveal_in_tree`]).
//! - Hover lights both ([`RevealLink::hover`], [`RevealLink::hovered`]).
//! - [`Follow`] (S28): off, the tree keeps the view's top row in sight, or the view keeps the tree's.

use egui::Context;

use super::model::RowKey;
use super::state::Follow;

/// The link between a tree and its view, shared by both. Transient: rebuild it at start, never save it.
#[derive(Clone, Debug)]
pub struct RevealLink<K> {
    /// Which side follows the other (S28).
    pub follow: Follow,
    to_view: Option<K>,
    to_tree: Option<K>,
    flash: Option<(K, f64)>,
    hover: Option<(K, u64)>,
    view_top: Option<K>,
    tree_top: Option<K>,
}

impl<K> Default for RevealLink<K> {
    fn default() -> Self {
        Self {
            follow: Follow::Off,
            to_view: None,
            to_tree: None,
            flash: None,
            hover: None,
            view_top: None,
            tree_top: None,
        }
    }
}

impl<K: RowKey> RevealLink<K> {
    /// A link with `follow` (S28).
    pub fn new(follow: Follow) -> Self {
        Self {
            follow,
            ..Self::default()
        }
    }

    /// The tree asks the view to show `key` (a click on its row).
    pub fn reveal_in_view(&mut self, key: K) {
        self.to_view = Some(key);
    }

    /// The view asks the tree to show `key` (a selection in the view).
    pub fn reveal_in_tree(&mut self, key: K) {
        self.to_tree = Some(key);
    }

    /// For the view: the item to scroll into sight now, once. Starts its flash at the frame's time.
    pub fn take_for_view(&mut self, ctx: &Context) -> Option<K> {
        let key = self.take_for_view_at(ctx.input(|i| i.time))?;
        ctx.request_repaint();
        Some(key)
    }

    /// [`Self::take_for_view`] on a clock of the caller's: the flash starts at `now`, seconds.
    pub fn take_for_view_at(&mut self, now: f64) -> Option<K> {
        let key = self.to_view.take()?;
        self.flash = Some((key.clone(), now));
        Some(key)
    }

    /// For the tree: the item to reveal now, once. Starts its flash at `now`, seconds.
    pub(crate) fn take_for_tree_at(&mut self, now: f64) -> Option<K> {
        let key = self.to_tree.take()?;
        self.flash = Some((key.clone(), now));
        Some(key)
    }

    /// How strongly `key` flashes in `ui`'s frame: [`Self::flash_at`] on egui's clock with the `Ui`'s
    /// `Tokens::chrome.flash_secs`; asks for repaints while it lasts.
    pub fn flash(&self, ui: &egui::Ui, key: &K) -> f32 {
        let secs = f64::from(crate::Tokens::of_ui(ui).chrome.flash_secs);
        let f = self.flash_at(key, ui.input(|i| i.time), secs);
        if f > 0.0 {
            ui.ctx().request_repaint();
        }
        f
    }

    /// How strongly `key` flashes at `now` (seconds, the clock the flash started on): 1 at the reveal, fading
    /// to 0 over `secs`.
    pub fn flash_at(&self, key: &K, now: f64, secs: f64) -> f32 {
        let Some((k, at)) = &self.flash else {
            return 0.0;
        };
        let age = now - at;
        if k != key || !(0.0..secs).contains(&age) {
            return 0.0;
        }
        (1.0 - age / secs) as f32
    }

    /// Either side: the pointer is over `key` this frame.
    pub fn hover(&mut self, ctx: &Context, key: K) {
        self.hover = Some((key, ctx.cumulative_pass_nr()));
    }

    /// The item the pointer is over on either side, this frame or the one before (the other side may draw
    /// first).
    pub fn hovered(&self, ctx: &Context) -> Option<&K> {
        let (k, pass) = self.hover.as_ref()?;
        (pass + 1 >= ctx.cumulative_pass_nr()).then_some(k)
    }

    /// For the view: report its top visible row each frame (used when the tree follows the view).
    pub fn set_view_top(&mut self, key: Option<K>) {
        self.view_top = key;
    }

    /// For the view: the row to keep at its top when it follows the tree; `None` otherwise.
    pub fn view_should_show(&self) -> Option<&K> {
        (self.follow == Follow::ViewFollowsTree)
            .then_some(self.tree_top.as_ref())
            .flatten()
    }

    pub(crate) fn tree_should_show(&self) -> Option<&K> {
        (self.follow == Follow::TreeFollowsView)
            .then_some(self.view_top.as_ref())
            .flatten()
    }

    pub(crate) fn set_tree_top(&mut self, key: Option<K>) {
        self.tree_top = key;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reveal from the view starts a flash in the tree that fades over its duration, for that row only.
    #[test]
    fn a_reveal_in_the_tree_flashes_then_fades() {
        let mut link = RevealLink::<u8>::default();
        link.reveal_in_tree(3);
        assert_eq!(link.take_for_tree_at(10.0), Some(3));
        assert_eq!(link.take_for_tree_at(10.1), None, "once");
        assert_eq!(link.flash_at(&3, 10.0, 0.8), 1.0);
        assert!((link.flash_at(&3, 10.4, 0.8) - 0.5).abs() < 1e-6);
        assert_eq!(link.flash_at(&3, 10.8, 0.8), 0.0);
        assert_eq!(link.flash_at(&4, 10.1, 0.8), 0.0);
    }

    /// The view follows the tree only with that setting; the tree follows the view only with its own.
    #[test]
    fn follow_asks_the_right_side() {
        let mut link = RevealLink::new(Follow::ViewFollowsTree);
        link.set_tree_top(Some(5_u8));
        link.set_view_top(Some(9));
        assert_eq!(link.view_should_show(), Some(&5));
        assert_eq!(link.tree_should_show(), None);
        link.follow = Follow::TreeFollowsView;
        assert_eq!(link.view_should_show(), None);
        assert_eq!(link.tree_should_show(), Some(&9));
    }
}
