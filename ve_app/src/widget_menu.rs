//! The open-widget menu, built from the `WidgetInfo` registry (WID-3).

use egui::Ui;
use ve_widget::WidgetInfo;

/// Every widget registered with `inventory::submit!(WidgetInfo { .. })` in the binary, sorted by group path
/// and title.
pub fn registered_widgets() -> Vec<&'static WidgetInfo> {
    let mut all: Vec<_> = inventory::iter::<WidgetInfo>().collect();
    all.sort_by_key(|w| (w.group_path, w.base_title));
    all
}

/// Draw the registered widgets passing `filter` as nested menus, one submenu per `group_path` segment
/// (`"display/analog"` → *display → analog → …*); widgets with an empty path come last, at the top level.
/// Returns the widget clicked this frame.
pub(crate) fn ui(ui: &mut Ui, filter: fn(&WidgetInfo) -> bool) -> Option<&'static WidgetInfo> {
    let widgets: Vec<_> = registered_widgets()
        .into_iter()
        .filter(|w| filter(w))
        .collect();
    if widgets.is_empty() {
        ui.weak("No widgets registered")
            .on_hover_text("Register widgets with inventory::submit!(WidgetInfo { .. })");
        return None;
    }
    let entries: Vec<_> = widgets
        .iter()
        .map(|w| (segments(w.group_path), *w))
        .collect();
    level(ui, &entries)
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// One menu level: submenus for the first path segment, then the widgets that end here.
fn level(ui: &mut Ui, entries: &[(Vec<&str>, &'static WidgetInfo)]) -> Option<&'static WidgetInfo> {
    let mut picked = None;
    let mut groups: Vec<&str> = entries
        .iter()
        .filter_map(|(path, _)| path.first().copied())
        .collect();
    groups.dedup();
    for group in groups {
        let inner: Vec<_> = entries
            .iter()
            .filter(|(path, _)| path.first() == Some(&group))
            .map(|(path, w)| (path[1..].to_vec(), *w))
            .collect();
        ui.menu_button(group, |ui| {
            if let Some(w) = level(ui, &inner) {
                picked = Some(w);
            }
        });
    }
    for (_, w) in entries.iter().filter(|(path, _)| path.is_empty()) {
        let tooltip = if w.tags.is_empty() {
            format!("Open a new {} as a tab", w.base_title)
        } else {
            format!(
                "Open a new {} as a tab\nTags: {}",
                w.base_title,
                w.tags.join(", ")
            )
        };
        if ui.button(w.base_title).on_hover_text(tooltip).clicked() {
            picked = Some(*w);
            ui.close();
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::segments;

    #[test]
    fn group_path_segments() {
        assert!(segments("").is_empty());
        assert_eq!(segments("display/analog"), ["display", "analog"]);
        assert_eq!(segments("/display//"), ["display"]);
    }
}
