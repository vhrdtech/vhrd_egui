//! The side panel's gallery page: the tracks tree of frame B2 linked to a view beside it, the row states
//! panel on the right, and every panel setting above them.

use egui::{
    Align, Frame, Id, Rect, Sense, Theme, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

use super::paint::{fill, snap, snap_pos};
use super::{
    Count, Follow, Group, Icon, PanelForm, PanelSettings, RevealLink, Row, Side, SidePanel,
    SidePanelState, View,
};
use crate::props::Segmented;
use crate::{Density, LightStyle, Tokens, Tone};

/// A track of the demo: what both the tree and the view show.
struct Track {
    key: &'static str,
    label: &'static str,
    icon: &'static str,
    value: &'static str,
    light: Option<Tone>,
    count: Option<Count>,
    open: bool,
    children: &'static [Track],
}

const fn track(key: &'static str, icon: &'static str, value: &'static str) -> Track {
    Track {
        key,
        label: key,
        icon,
        value,
        light: None,
        count: None,
        open: true,
        children: &[],
    }
}

const WAVE: &str = "≈";

const BENCH: &[Track] = &[
    Track {
        light: Some(Tone::Good),
        children: &[track("state", "⊡", "run")],
        ..track("can0", WAVE, "0x123")
    },
    Track {
        children: &[
            track("phase current", WAVE, "12.4 A"),
            track("vbus", WAVE, "47.9 V"),
        ],
        ..track("motor", WAVE, "1 482 rpm")
    },
    Track {
        light: Some(Tone::Warn),
        count: Some(Count::Plain(2)),
        open: false,
        children: &[
            track("rail 5V", WAVE, "5.02 V"),
            track("rail 3V3", WAVE, "3.29 V"),
        ],
        ..track("power", WAVE, "")
    },
    Track {
        label: "link 0 · tap",
        light: Some(Tone::Good),
        open: false,
        children: &[track("frames", "▣", "")],
        ..track("link0", "🔗", "")
    },
    Track {
        count: Some(Count::Plain(38)),
        children: &[Track {
            count: Some(Count::Unread(3)),
            ..track("log", "▣", "")
        }],
        ..track("i2c1", "⊞", "")
    },
];

const IMU: &[Track] = &[
    track("accel", WAVE, "0.98 g"),
    track("gyro", WAVE, "-2.1 °/s"),
];

const PROJECT: &[Track] = &[
    Track {
        count: Some(Count::Plain(4)),
        ..track("notes", "🔖", "")
    },
    Track {
        count: Some(Count::Plain(3)),
        ..track("markers", "📌", "")
    },
];

const GROUPS: [(&str, &str, &[Track]); 3] = [
    ("dev0 · bench rig", "38 transfers · 2 warnings", BENCH),
    ("IMU · own clock", "2 sensors", IMU),
    ("Project", "4 notes · 3 markers", PROJECT),
];

/// The gallery page's state: both panels, their settings and link, the view's selection.
#[derive(Clone, Debug)]
pub struct PanelGallery {
    /// The tracks panel (left by default).
    pub tracks: SidePanelState,
    /// The row-states panel (right).
    pub states: SidePanelState,
    /// Form, follow, lights (S6, S28, S29).
    pub settings: PanelSettings,
    /// The chrome density shown.
    pub density: Density,
    /// The selected track.
    pub selected: &'static str,
    /// The tree to view link.
    pub link: RevealLink<&'static str>,
    /// The last thing picked, said under the settings.
    pub said: String,
    /// Show the settings strip and the line under it.
    pub strip: bool,
}

impl Default for PanelGallery {
    fn default() -> Self {
        Self {
            tracks: SidePanelState::docked(Side::Left),
            states: SidePanelState::docked(Side::Right),
            settings: PanelSettings::default(),
            density: Density::Comfortable,
            selected: "phase current",
            link: RevealLink::default(),
            said:
                "Click a row: it is revealed in the view; click in the view: revealed in the tree"
                    .to_owned(),
            strip: true,
        }
    }
}

impl PanelGallery {
    /// The page in one theme on its background, `height` points tall.
    pub fn themed(&mut self, ui: &mut Ui, theme: Theme, height: f32) {
        ui.set_style(ui.ctx().style_of(theme));
        let t = Tokens::of(ui.ctx(), theme);
        Frame::new().fill(t.colors.bg).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            self.ui(ui, height);
        });
    }

    /// The settings strip, then the window with both panels and the view, `height` points tall.
    pub fn ui(&mut self, ui: &mut Ui, height: f32) {
        let t = Tokens::of_ui(ui);
        let theme = if ui.visuals().dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        };
        if self.strip {
            self.settings_strip(ui, &t);
            ui.label(egui::RichText::new(&self.said).color(t.colors.text_muted));
        }
        self.link.follow = self.settings.follow;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let rect = snap(ui.painter(), rect);
        fill(ui.painter(), rect, 0, t.colors.bg);
        let mut win = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .id_salt(("panel window", theme)),
        );
        win.set_clip_rect(rect);
        self.tracks_panel(&mut win, theme);
        self.states_panel(&mut win, theme);
        self.view(&mut win);
    }

    fn settings_strip(&mut self, ui: &mut Ui, t: &Tokens) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = t.space.m;
            Segmented::new(&mut self.settings.form)
                .item(
                    PanelForm::ListToRail,
                    "List, rail",
                    "B2, the pick: a list with groups that collapses to its rail",
                )
                .item(
                    PanelForm::RailAndList,
                    "Views and list",
                    "B1: a rail of panel views beside the list",
                )
                .item(
                    PanelForm::RailOnly,
                    "Rail only",
                    "B3: the rail only, with hover cards",
                )
                .id_salt("form")
                .show(ui);
            Segmented::new(&mut self.settings.follow)
                .item(
                    Follow::Off,
                    "Apart",
                    "S28: tree and view scroll apart; a click reveals",
                )
                .item(
                    Follow::TreeFollowsView,
                    "Tree follows",
                    "The tree keeps the view's top row in sight",
                )
                .item(
                    Follow::ViewFollowsTree,
                    "View follows",
                    "The view keeps the tree's top row in sight",
                )
                .id_salt("follow")
                .show(ui);
            Segmented::new(&mut self.settings.lights)
                .item(
                    None,
                    "App lights",
                    "S29: follow the app's setting (Tokens::chrome.lights)",
                )
                .item(
                    Some(LightStyle::Perforated),
                    "Perforated",
                    "S29, the pick: the perforated light, for this panel",
                )
                .item(
                    Some(LightStyle::Dot),
                    "Dot",
                    "S29: a plain 8 px dot on whole pixels, for this panel",
                )
                .id_salt("lights")
                .show(ui);
            let before = self.density;
            Segmented::new(&mut self.density)
                .item(Density::Comfortable, "Comfortable", "E1, the pick")
                .item(
                    Density::Compact,
                    "Compact",
                    "E1: every size one step smaller",
                )
                .id_salt("density")
                .show(ui);
            if before != self.density {
                set_density(ui.ctx(), self.density);
            }
        });
    }

    fn tracks_panel(&mut self, ui: &mut Ui, theme: Theme) {
        let selected = self.selected;
        // spirit-level: allow(egui-panel-api) ve_theme's own SidePanel, not egui's old one
        let r = SidePanel::new(("gallery tracks", theme), "Tracks")
            .noun("tracks")
            .settings(self.settings)
            .views([
                View::new("☰", "Tracks"),
                View::new("🔍", "Search"),
                View::new("🔖", "Bookmarks").unread(4),
                View::new("⊞", "Layers"),
            ])
            .link(&mut self.link)
            .actions(|ui| {
                ui.add(egui::Button::new("+").frame(false))
                    .on_hover_text("Add a track");
            })
            .row_menu(|ui, key| {
                ui.label(key.to_string());
                if ui.button("Rename…").clicked() {
                    ui.close();
                }
            })
            .show(ui, &mut self.tracks, |list| {
                let only = (list.view() == 2).then_some(2);
                for (gi, (title, summary, tracks)) in GROUPS.iter().enumerate() {
                    if only.is_some_and(|o| o != gi) {
                        continue;
                    }
                    list.group(
                        Group::new(*title)
                            .summary(*summary)
                            .key_hint(format!("Ctrl+{}", gi + 1)),
                        |g| {
                            for tr in *tracks {
                                g.row(row(tr, selected));
                            }
                        },
                    );
                }
            });
        if let Some(k) = r.clicked {
            self.selected = k;
            self.said = format!("Picked {k} in the tree: revealed in the view");
        }
        if let Some(i) = r.rail_clicked {
            self.said = format!("Rail item {} clicked", i + 1);
        }
    }

    fn states_panel(&mut self, ui: &mut Ui, theme: Theme) {
        // spirit-level: allow(egui-panel-api) ve_theme's own SidePanel, not egui's old one
        SidePanel::new(("gallery states", theme), "Devices")
            .noun("devices")
            .filter(false)
            .settings(PanelSettings {
                form: PanelForm::ListToRail,
                ..self.settings
            })
            .keys(None, None)
            .show(ui, &mut self.states, |list| {
                list.group(Group::new("Row states").icon("⚙"), |g| {
                    let dev = |k: &'static str| Row::new(k, k).icon("⚙");
                    g.row(dev("rest"))
                        .row(dev("hovered"))
                        .row(dev("selected").selected(true))
                        .row(dev("focused (keyboard)"))
                        .row(dev("disabled: offline").light(Tone::Off).disabled_because(Some(
                            "typec dongle is offline. Plug it in or press Rescan (F5) to enable.",
                        )))
                        .row(Row::new("long", "nucleo-g0b1re with a very long product name and serial").icon("⚙").light(Tone::Good))
                        .row(dev("status light and count").light(Tone::Warn).count(Count::Plain(12)))
                        .row(dev("unread count").count(Count::Unread(3)));
                });
                list.group(Group::new("A folded group").default_open(false), |g| {
                    g.row(Row::new("hidden a", "hidden a")).row(Row::new("hidden b", "hidden b"));
                });
            });
    }

    /// The linked view: every track as a row with a head, scrolled on its own.
    fn view(&mut self, ui: &mut Ui) {
        let t = Tokens::of_ui(ui);
        let rect = ui.available_rect_before_wrap();
        let mut ui = ui.new_child(UiBuilder::new().max_rect(rect));
        let mut flat = Vec::new();
        for (_, _, tracks) in GROUPS {
            flatten(tracks, &mut flat);
        }
        let ctx = ui.ctx().clone();
        let reveal = self.link.take_for_view(&ctx);
        let follow = self.link.view_should_show().copied();
        let mut top = None;
        egui::ScrollArea::vertical()
            .id_salt("gallery view")
            .auto_shrink([false, false])
            .show(&mut ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for tr in &flat {
                    let (r_rect, r) = ui.allocate_exact_size(
                        vec2(ui.available_width(), t.chrome.row * 2.0),
                        Sense::click(),
                    );
                    let head = Rect::from_min_size(
                        r_rect.min,
                        vec2(t.chrome.side_panel_min * 0.75, r_rect.height()),
                    );
                    let p = ui.painter();
                    let selected = self.selected == tr.key;
                    let ground = if selected {
                        t.colors.selection
                    } else if self.link.hovered(&ctx) == Some(&tr.key) {
                        t.colors.status.info.hover
                    } else {
                        t.colors.surface
                    };
                    let flash = self.link.flash(ui, &tr.key);
                    fill(p, head, 0, ground);
                    fill(
                        p,
                        head,
                        0,
                        t.colors.focus.gamma_multiply(t.chrome.flash * flash),
                    );
                    fill(
                        p,
                        Rect::from_min_max(
                            pos2(r_rect.left(), r_rect.bottom() - t.stroke.thin),
                            r_rect.max,
                        ),
                        0,
                        t.colors.line,
                    );
                    p.text(
                        snap_pos(p, pos2(head.left() + t.space.m, head.center().y)),
                        egui::Align2::LEFT_CENTER,
                        tr.label,
                        egui::FontId::proportional(t.chrome.text_size),
                        t.colors.text,
                    );
                    fill(
                        p,
                        Rect::from_center_size(
                            pos2(
                                head.right() + (r_rect.right() - head.right()) * 0.5,
                                r_rect.center().y,
                            ),
                            vec2((r_rect.right() - head.right()) * 0.6, t.space.s),
                        ),
                        0,
                        t.colors.cat[1].lerp_to_gamma(t.colors.bg, 0.4),
                    );
                    let name = format!("view: {}", tr.label);
                    r.widget_info(|| {
                        WidgetInfo::selected(WidgetType::Button, true, selected, &name)
                    });
                    if r.clicked() {
                        self.selected = tr.key;
                        self.link.reveal_in_tree(tr.key);
                        self.said = format!("Picked {} in the view: revealed in the tree", tr.key);
                    }
                    if r.hovered() {
                        self.link.hover(&ctx, tr.key);
                    }
                    if reveal == Some(tr.key) || follow == Some(tr.key) && changed(ui, tr.key) {
                        r.scroll_to_me(Some(Align::Center));
                    }
                    if top.is_none() && r_rect.bottom() > ui.clip_rect().top() {
                        top = Some(tr.key);
                    }
                }
            });
        self.link.set_view_top(top);
    }
}

/// Whether the view's follow target changed since it last scrolled to it.
fn changed(ui: &Ui, key: &'static str) -> bool {
    let id = Id::new("ve_theme::panel gallery followed");
    let new = ui.data(|d| d.get_temp::<&'static str>(id)) != Some(key);
    ui.data_mut(|d| d.insert_temp(id, key));
    new
}

fn flatten<'a>(tracks: &'a [Track], out: &mut Vec<&'a Track>) {
    for tr in tracks {
        out.push(tr);
        flatten(tr.children, out);
    }
}

fn row(tr: &Track, selected: &'static str) -> Row<&'static str> {
    let mut r = Row::new(tr.key, tr.label)
        .icon(Icon::from(tr.icon))
        .selected(tr.key == selected)
        .default_open(tr.open);
    if !tr.value.is_empty() {
        r = r.value(tr.value);
    }
    if let Some(l) = tr.light {
        r = r.light(l);
    }
    if let Some(c) = tr.count {
        r = r.count(c);
    }
    r.children(tr.children.iter().map(|c| row(c, selected)))
}

/// Install the chrome sizes of `density` in both themes' tokens, keeping the app's chrome choices that are
/// not sizes: status lights (S29), flash, the panel's width share.
pub fn set_density(ctx: &egui::Context, density: Density) {
    for theme in [Theme::Dark, Theme::Light] {
        let mut t = (*Tokens::of(ctx, theme)).clone();
        let keep = t.chrome;
        t.chrome = crate::Chrome {
            lights: keep.lights,
            flash_secs: keep.flash_secs,
            flash: keep.flash,
            side_panel_max_share: keep.side_panel_max_share,
            ..crate::Chrome::for_density(density)
        };
        t.apply(ctx, theme);
    }
}
