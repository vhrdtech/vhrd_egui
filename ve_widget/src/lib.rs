pub mod util;

use std::any::Any;
pub use ve_macro::svt;

use egui::{Id, Ui, WidgetText};

#[typetag::serde(tag = "type")]
pub trait Widget {
    /// Initialize non-serializeable state for this widget.
    /// E.g., establish networks connections.
    /// Must be called after deserialization or creation using spawn_fn.
    fn init(&mut self, cx: &Box<dyn Any>);

    /// Return the static part of the widget title.
    fn base_title(&self) -> &'static str;

    /// Actual tile or window header WidgetText.
    fn widget_text(&self) -> WidgetText {
        self.base_title().into()
    }

    /// Render the widget's UI.
    fn ui(&mut self, ui: &mut Ui, cx: &Box<dyn Any>, id: Id);

    /// Return whether tile or window containing this widget is currently closeable.
    fn is_closeable(&self) -> bool {
        true
    }

    /// Called once before each call to [`Self::ui`],
    /// and additionally also called when the UI is hidden, but [`egui::Context::request_repaint`] was called.
    fn logic(&mut self, cx: &Box<dyn Any>) {
        let _ = cx;
    }
}

/// Information about a widget, used to generate menus, filter widgets and spawn actual widgets.
pub struct WidgetInfo {
    /// Static part of the widget title. Actual title can also also contain dynamic information.
    pub base_title: &'static str,
    /// Group this widgets belongs to, e.g., "display/analog".
    /// Used to create hierarchical menus to open tiles or windows with widgets.
    /// Leave empty to show at root.
    pub group_path: &'static str,
    /// Tags associated with this widget, used for filtering and searching.
    /// Can be empty.
    pub tags: &'static [&'static str],
    /// Create a new instance of this widget.
    pub spawn_fn: fn() -> Box<dyn Widget>,
}

inventory::collect!(WidgetInfo);
