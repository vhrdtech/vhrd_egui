//! The demo's pie menu, shared with the kittests (`tests/pie.rs` includes this file).

use ve_spacemouse::{PieMenu, Slot, icons};

/// What the demo's slots do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    AddDot,
    DeleteDot,
    Copy,
    Paste,
    Cut,
    Measure,
    ZoomToFit,
    Recenter,
    Snap,
    Highlight,
    Route,
    Flip,
    Fill,
    Grid,
    Save,
    Undo,
    Redo,
}

/// What decides which slots are enabled.
#[derive(Clone, Copy, Debug, Default)]
pub struct Can {
    pub paste: bool,
    pub save: bool,
    pub undo: bool,
    pub redo: bool,
}

/// The menu: centre halves add and delete, the left ring edits, the right ring views, both together save and undo.
pub fn menu(can: Can) -> PieMenu<Action> {
    use Action::*;
    let s = |label: &str, icon: &str, a| Some(Slot::new(label, icon, a));
    PieMenu {
        center_left: Some(Slot::new("Add dot", icons::PLUS, AddDot)),
        center_right: Some(Slot::new("Delete dot", icons::TRASH, DeleteDot)),
        ring_left: vec![
            s("Copy", icons::COPY, Copy),
            Some(
                Slot::new("Paste", icons::CLIPBOARD, Paste)
                    .disabled_if(!can.paste, "Nothing copied yet: copy a dot first"),
            ),
            s("Cut", icons::SCISSORS, Cut),
            s("Measure", icons::RULER, Measure),
            s("Zoom to fit", icons::CORNERS_OUT, ZoomToFit),
            s("Recenter", icons::CROSSHAIR, Recenter),
            s("Snap to grid", icons::MAGNET, Snap),
            None,
        ],
        ring_right: vec![
            s("Highlight", icons::HIGHLIGHTER, Highlight),
            s("Route", icons::PATH, Route),
            s("Flip", icons::FLIP_HORIZONTAL, Flip),
            None,
            s("Fill", icons::PAINT_BUCKET, Fill),
            None,
            s("Show grid", icons::DOTS_NINE, Grid),
            None,
        ],
        ring_both: vec![
            Some(
                Slot::new("Save", icons::FLOPPY_DISK, Save)
                    .disabled_if(!can.save, "Nothing changed since the last save"),
            ),
            None,
            Some(
                Slot::new("Redo", icons::ARROW_CLOCKWISE, Redo)
                    .disabled_if(!can.redo, "Nothing undone to redo"),
            ),
            None,
            None,
            None,
            Some(
                Slot::new("Undo", icons::ARROW_COUNTER_CLOCKWISE, Undo)
                    .disabled_if(!can.undo, "Nothing to undo yet"),
            ),
            None,
        ],
        free_hint: "Nothing assigned to this slot yet: set it in examples/demo/menu.rs".into(),
        ..Default::default()
    }
}
