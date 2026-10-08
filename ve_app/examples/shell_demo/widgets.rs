//! Two demo widgets, ported from eframe_template's widget_a / widget_b. Also used by `tests/ui.rs`.

use ve_app::prelude::*;

const NOTE_INFO: WidgetInfo = WidgetInfo {
    base_title: "Note",
    group_path: "",
    tags: &["text"],
    spawn_fn: || Box::new(Note::default()),
};
inventory::submit!(NOTE_INFO);

/// A one-line note: the seed / visual / transient split with `svt!`.
#[derive(Default, Serialize, Deserialize)]
pub struct Note {
    seed: Seed,
    visual: Visual,
    #[serde(skip)]
    transient: Option<Transient>,
}

/// Everything needed to make an identical widget, maybe on another machine (e.g. a database entry id).
/// Serialized; also used to reset the widget to a fresh one with the same seed.
#[derive(Clone, Default, Serialize, Deserialize)]
struct Seed {}

/// State from user interaction (text input, selection). Serialized with the layout.
#[derive(Default, Serialize, Deserialize)]
struct Visual {
    text: String,
}

/// What can't or needn't be saved: connections, caches. Rebuilt in `init`.
struct Transient {
    edits: usize,
}

#[typetag::serde]
impl Widget for Note {
    fn init(&mut self, _cx: &Context) {
        self.transient = Some(Transient { edits: 0 });
    }

    fn base_title(&self) -> &'static str {
        NOTE_INFO.base_title
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &Context, _id: Id) {
        svt!();
        let r = ui
            .text_edit_singleline(&mut v.text)
            .on_hover_text("A note; saved with the layout");
        if r.changed() {
            t.edits += 1;
        }
        ui.label(format!("{} edits since start", t.edits))
            .on_hover_text("Transient state: counted since the widget was created, not saved");
    }

    fn seed(&self) -> Box<dyn erased_serde::Serialize> {
        Box::new(self.seed.clone())
    }
}

const LEVEL_INFO: WidgetInfo = WidgetInfo {
    base_title: "Level",
    group_path: "inputs/analog",
    tags: &["number"],
    spawn_fn: || Box::new(Level::default()),
};
inventory::submit!(LEVEL_INFO);

/// A slider with a "busy" switch: while busy it can't be closed and quitting asks first.
#[derive(Default, Serialize, Deserialize)]
pub struct Level {
    level: f32,
    busy: bool,
}

#[typetag::serde]
impl Widget for Level {
    fn init(&mut self, _cx: &Context) {}

    fn base_title(&self) -> &'static str {
        LEVEL_INFO.base_title
    }

    fn widget_text(&self) -> WidgetText {
        format!("Level {:.1}", self.level).into()
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &Context, _id: Id) {
        ui.add(egui::Slider::new(&mut self.level, 0.0..=10.0).suffix(" V"))
            .on_hover_text("Output level in volts, 0 to 10 V");
        ui.checkbox(&mut self.busy, "Busy").on_hover_text(
            "Pretend a transfer is running: the tab can't be closed, quitting asks first",
        );
    }

    fn is_closeable(&self) -> bool {
        !self.busy
    }
}
