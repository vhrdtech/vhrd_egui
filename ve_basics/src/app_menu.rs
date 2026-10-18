//! The app menu every vhrd egui app shares (BAS-4): it opens from the version badge, says what is running and
//! offers to reset what the app has saved.

use egui::{Color32, Id, Modal, Popup, PopupCloseBehavior, Response, RichText, Sense, Ui};

use crate::BuildInfo;

/// What the user picked in the [`AppMenu`]; the app carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMenuAction {
    /// Confirmed *Reset storage to defaults*: put every saved setting, layout and frame state back to the
    /// defaults and carry on from there; [`reset_egui_memory`] does egui's part. What the app writes to its
    /// storage next time is then the defaults.
    ResetStorage,
}

/// The app menu: name, description and build in full, then *Reset storage to defaults…* behind a confirm
/// dialog.
///
/// An app with its own top bar puts it on its version badge with [`AppMenu::badge`] (or [`AppMenu::attach`] for
/// a badge it draws itself); a shell with a menu bar draws the same entries into its *Help* menu with
/// [`AppMenu::contents`] and calls [`AppMenu::dialog`] once per frame.
///
/// ```ignore
/// let menu = AppMenu::new(&build_info).title("TPM mesh").description("Live dashboard of the PC mesh");
/// if menu.badge(ui).action == Some(AppMenuAction::ResetStorage) {
///     self.settings = Settings::default();
///     ve_basics::reset_egui_memory(ui.ctx());
/// }
/// ```
pub struct AppMenu<'a> {
    info: &'a BuildInfo,
    title: &'a str,
    description: &'a str,
    resets: &'a str,
}

/// What [`AppMenu::badge`] gives back.
pub struct AppMenuResponse {
    /// The badge.
    pub response: Response,
    /// Set in the frame the user confirmed an action.
    pub action: Option<AppMenuAction>,
}

/// Where the confirm dialog's open flag lives in egui memory.
fn confirm_id() -> Id {
    Id::new("ve_basics::app_menu::confirm")
}

/// Accessible label and hover of the badge; tests find the badge by it.
pub const BADGE_HOVER: &str = "Click for the app menu: about, build, reset storage";

impl<'a> AppMenu<'a> {
    /// The menu of the app `info` was built for; the title is the crate name until [`Self::title`].
    pub fn new(info: &'a BuildInfo) -> Self {
        Self {
            info,
            title: info.name,
            description: "",
            resets: "",
        }
    }

    /// The app's name as people know it ("TPM mesh", not `tpm_mesh_dash`).
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// One or two sentences: what the app is for.
    pub fn description(mut self, description: &'a str) -> Self {
        self.description = description;
        self
    }

    /// What a reset puts back, for the confirm dialog ("the layout, every panel's size and fold, the
    /// backdrop"); the window positions and scroll offsets egui keeps are always named.
    pub fn resets(mut self, what: &'a str) -> Self {
        self.resets = what;
        self
    }

    /// The version badge (`version · sha · debug|release`, the debug marker orange, as
    /// [`crate::build_info_label`]) that opens the menu on click, with the confirm dialog.
    pub fn badge(&self, ui: &mut Ui) -> AppMenuResponse {
        let text = RichText::new(self.info.short()).monospace().small();
        let text = if self.info.debug {
            text.color(Color32::from_rgb(0xe0, 0x8a, 0x2e))
        } else {
            text.weak()
        };
        let response = ui.add(egui::Button::new(text).frame(false).sense(Sense::click()));
        let action = self.attach(&response);
        AppMenuResponse { response, action }
    }

    /// The menu on a badge the app drew itself (clickable): the badge's hover spells out the build, a click
    /// opens the menu below it, and the confirm dialog is drawn. Returns the action confirmed this frame.
    pub fn attach(&self, badge: &Response) -> Option<AppMenuAction> {
        let badge = badge
            .clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let open = Popup::is_id_open(&badge.ctx, Popup::default_response_id(&badge));
        if !open {
            badge
                .clone()
                .on_hover_text(format!("{}\n\n{BADGE_HOVER}", self.info.details()));
        }
        badge
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, BADGE_HOVER));
        Popup::menu(&badge)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(240.0);
                self.contents(ui);
            });
        self.dialog(&badge.ctx)
    }

    /// The menu's entries, into a menu the app already has (a *Help* menu). Picking the reset asks to confirm
    /// in [`Self::dialog`], which the app calls every frame.
    pub fn contents(&self, ui: &mut Ui) {
        ui.label(RichText::new(self.title).strong().size(15.0));
        if !self.description.is_empty() {
            ui.label(RichText::new(self.description).weak());
        }
        ui.add_space(4.0);
        egui::Grid::new("ve_basics::app_menu::build")
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                let row = |ui: &mut Ui, key: &str, hover: &str, value: &str| {
                    ui.label(RichText::new(key).weak()).on_hover_text(hover);
                    crate::copyable_label(ui, RichText::new(value).monospace());
                    ui.end_row();
                };
                row(
                    ui,
                    "version",
                    "The app's version (Cargo.toml)",
                    self.info.version,
                );
                row(
                    ui,
                    "commit",
                    "The git commit built; -dirty means uncommitted changes were built in",
                    self.info.git_sha,
                );
                row(
                    ui,
                    "built",
                    "When this binary was built",
                    self.info.build_time,
                );
                row(
                    ui,
                    "profile",
                    "release is optimized; debug is unoptimized, with debug assertions, slower",
                    self.info.profile(),
                );
            });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(RichText::new("Built with ").weak());
            ui.hyperlink_to("egui", "https://github.com/emilk/egui")
                .on_hover_text("The immediate-mode GUI library (opens the browser)");
            ui.label(RichText::new(" and vhrd_egui.").weak());
        });
        ui.separator();
        if ui
            .button("Reset storage to defaults…")
            .on_hover_text(format!(
                "Forget everything {} saved and start again from the defaults; asks first",
                self.title
            ))
            .clicked()
        {
            ui.ctx().data_mut(|d| d.insert_temp(confirm_id(), true));
            ui.close();
        }
    }

    /// The confirm dialog of the reset, while it is open; `Some` in the frame the user confirmed.
    pub fn dialog(&self, ctx: &egui::Context) -> Option<AppMenuAction> {
        let open = ctx
            .data(|d| d.get_temp::<bool>(confirm_id()))
            .unwrap_or(false);
        if !open {
            return None;
        }
        let mut action = None;
        let mut close = false;
        let modal = Modal::new(Id::new("ve_basics::app_menu::dialog")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.label(RichText::new("Reset storage to defaults?").strong().size(15.0));
            ui.add_space(4.0);
            let what = if self.resets.is_empty() {
                "every saved setting and the layout".to_owned()
            } else {
                self.resets.to_owned()
            };
            ui.label(format!(
                "{} forgets {what}, and the window positions, scroll offsets and folds egui keeps, and starts \
                 again from the defaults. This can't be undone.",
                self.title
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let danger = ui.visuals().error_fg_color;
                if ui
                    .button(RichText::new("Reset").color(danger).strong())
                    .on_hover_text("Forget the saved state now and go back to the defaults")
                    .clicked()
                {
                    action = Some(AppMenuAction::ResetStorage);
                    close = true;
                }
                if ui
                    .button("Cancel")
                    .on_hover_text("Keep everything as it is (Esc)")
                    .clicked()
                {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            ctx.data_mut(|d| d.remove::<bool>(confirm_id()));
        }
        action
    }
}

/// egui's part of a storage reset: forget window positions, scroll offsets and folds, but keep the options the
/// app set up (styles and fonts from `ve_theme::setup`, the zoom, the theme preference; an app with a theme
/// switch resets that itself).
pub fn reset_egui_memory(ctx: &egui::Context) {
    ctx.memory_mut(|m| {
        let options = m.options.clone();
        *m = Default::default();
        m.options = options;
    });
}
