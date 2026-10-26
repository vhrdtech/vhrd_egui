//! Labels: not selectable by default, selectable where copying makes sense.

use egui::{Context, Label, Response, Ui, WidgetText};

/// App-setup, once at startup: labels stop being selectable (`style.interaction.selectable_labels = false`).
/// Selectable labels swallow clicks and drags meant for the widget under them and show a text cursor over
/// everything; apps then opt in with [`copyable_label`].
pub fn setup_labels(ctx: &Context) {
    ctx.all_styles_mut(|style| style.interaction.selectable_labels = false);
}

/// A label whose text can be selected and copied (ids, paths, commands), whatever the global setting.
pub fn copyable_label(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(Label::new(text).selectable(true))
}

/// A region whose text can be selected and copied (a detail panel, a log), whatever the global setting:
/// every label drawn inside is selectable, a drag over it selects across lines and Ctrl+C copies the
/// selection. Buttons, links and other widgets inside keep their clicks.
pub fn copyable<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.style_mut().interaction.selectable_labels = true;
        add_contents(ui)
    })
    .inner
}
