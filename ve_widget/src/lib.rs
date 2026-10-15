pub mod context;
pub mod util;

pub use erased_serde;
pub use ve_frame;
pub use ve_macro::svt;

use egui::{Id, Ui, WidgetText};
use erased_serde::{Deserializer, Serialize};

use ve_frame::{Height, Size, SizeHints, Status};

use crate::context::Context;

/// What the frame around a widget shows beside its title ([`Widget::frame_spec`]): everything is optional, and
/// a part that is not given is not drawn. The frame is `ve_frame::WidgetFrame`; the host (`ve_app::Shell`)
/// builds it and adds the menu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameSpec {
    /// The LED left of the title and its state in words (the LED's tooltip).
    pub led: Option<(Status, String)>,
    /// What the widget is, in a sentence: the first part of the ⓘ tooltip.
    pub about: String,
    /// The widget's own stats in the ⓘ tooltip, as label and value rows ("samples", "1204").
    pub stats: Vec<(String, String)>,
    /// The sizes the widget offers; turns the size switches on and makes [`Widget::ui_sized`] worth
    /// implementing. The height switch is live when they have a tall form ([`SizeHints::tall`]).
    pub sizes: Option<SizeHints>,
    /// Let the body scroll on these axes when it wants more than its place gives (a tile, a small window);
    /// off on both by default (`ve_frame::WidgetFrame::scroll`).
    pub scroll: egui::Vec2b,
}

#[typetag::serde(tag = "type")]
pub trait Widget {
    /// Initialize non-serializeable state for this widget.
    /// E.g., establish networks connections.
    /// Must be called after deserialization or creation using spawn_fn.
    fn init(&mut self, cx: &Context);

    /// Return the static part of the widget title.
    fn base_title(&self) -> &'static str;

    /// Actual tile or window header WidgetText.
    fn widget_text(&self) -> WidgetText {
        self.base_title().into()
    }

    /// Render the widget's UI.
    fn ui(&mut self, ui: &mut Ui, cx: &Context, id: Id);

    /// Render the widget's UI arranged for the width step `size` (min: exactly [`FrameSpec::sizes`]' `min`
    /// wide; nom or wide) and the height step `height` (min: exactly `min` high; nom or tall). Hosts call this
    /// one; by default it ignores both and calls [`Self::ui`]. Implement it together with
    /// [`FrameSpec::sizes`].
    fn ui_sized(&mut self, ui: &mut Ui, cx: &Context, id: Id, size: Size, height: Height) {
        let _ = (size, height);
        self.ui(ui, cx, id);
    }

    /// What the frame around the widget shows: LED, ⓘ text and stats, sizes. Asked every frame; nothing by
    /// default.
    fn frame_spec(&self) -> FrameSpec {
        FrameSpec::default()
    }

    /// Return whether tile or window containing this widget is currently closeable.
    fn is_closeable(&self) -> bool {
        true
    }

    /// Called once before each call to [`Self::ui`],
    /// and additionally also called when the UI is hidden, but [`egui::Context::request_repaint`] was called.
    fn logic(&mut self, cx: &Context) {
        let _ = cx;
    }

    fn seed(&self) -> Box<dyn Serialize> {
        Box::new(())
    }

    fn set_seed(&mut self, seed: Box<dyn Deserializer>) {
        let _ = seed;
        let _ = self;
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
