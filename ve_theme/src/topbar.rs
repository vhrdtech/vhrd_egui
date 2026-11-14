//! The brand strip every vhrd egui app's top bar starts with (THM-19): the app's icon, its name as a
//! wordmark in the brand face and colour, and the version badge that opens the shared app menu
//! (`ve_basics::AppMenu`, BAS-4). `ve_basics::load_icon` decodes the icon's PNG once at startup; the app
//! keeps the texture and passes it in every frame.
//!
//! ```ignore
//! let brand = ve_theme::topbar::Brand::new("Better ERC", &info).icon(self.icon.as_ref());
//! if ve_theme::topbar::show(ui, &brand) == Some(ve_basics::AppMenuAction::ResetStorage) {
//!     self.reset_storage(ui.ctx());
//! }
//! ```

use egui::{Color32, RichText, TextureHandle, Ui, vec2};
use ve_basics::{AppMenu, AppMenuAction, BuildInfo};

use crate::Tokens;

const ICON_PX: f32 = 22.0;

/// What [`show`] draws: an app's icon, name and build.
pub struct Brand<'a> {
    icon: Option<&'a TextureHandle>,
    name: &'a str,
    connected: Option<bool>,
    info: &'a BuildInfo,
}

impl<'a> Brand<'a> {
    /// The name people know the app by ("Mesh Dash", "Better ERC"), and its build (`ve_basics::build_info!`).
    pub fn new(name: &'a str, info: &'a BuildInfo) -> Self {
        Self {
            icon: None,
            name,
            connected: None,
            info,
        }
    }

    /// The icon texture (`ve_basics::load_icon`); `None` draws no icon.
    pub fn icon(mut self, icon: Option<&'a TextureHandle>) -> Self {
        self.icon = icon;
        self
    }

    /// A live connection's state: the icon breathes while `true`, dims while `false`. Left unset, the icon
    /// draws at full strength and does neither.
    pub fn connected(mut self, connected: bool) -> Self {
        self.connected = Some(connected);
        self
    }
}

/// Draws the strip; `Some` in the frame the app menu's *Reset storage to defaults* was confirmed, for the app
/// to carry out.
pub fn show(ui: &mut Ui, brand: &Brand<'_>) -> Option<AppMenuAction> {
    if let Some(tex) = brand.icon {
        let alpha = match brand.connected {
            Some(true) => {
                let t = ui.input(|i| i.time) as f32;
                0.8 + 0.2 * (0.5 + 0.5 * (t * 1.6).sin())
            }
            Some(false) => 0.45,
            None => 1.0,
        };
        ui.add(
            egui::Image::new((tex.id(), vec2(ICON_PX, ICON_PX)))
                .tint(Color32::WHITE.gamma_multiply(alpha)),
        );
    }
    let tokens = Tokens::of_ui(ui);
    ui.label(
        RichText::new(brand.name)
            .color(tokens.colors.red)
            .font(crate::brand_font(ui.ctx(), tokens.type_scale.brand.size)),
    );
    AppMenu::new(brand.info).title(brand.name).badge(ui).action
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    const INFO: BuildInfo = BuildInfo {
        name: "demo",
        version: "1.2.3",
        git_sha: "abc1234",
        build_time: "14 Nov 2026 10:00",
        debug: false,
    };

    #[test]
    fn draws_the_wordmark_and_opens_the_app_menu() {
        let got = std::rc::Rc::new(std::cell::Cell::new(None));
        let mut h = Harness::builder().with_size(vec2(400.0, 300.0)).build_ui({
            let got = got.clone();
            move |ui| {
                let brand = Brand::new("Demo App", &INFO);
                if let Some(a) = show(ui, &brand) {
                    got.set(Some(a));
                }
            }
        });
        h.run();
        h.get_by_label("Demo App");
        h.get_by_label(ve_basics::BADGE_HOVER).click();
        h.run();
        h.get_by_label("Reset storage to defaults…").click();
        h.run();
        h.get_by_label("Reset").click();
        h.run();
        assert_eq!(got.get(), Some(AppMenuAction::ResetStorage));
    }

    #[test]
    fn no_icon_texture_draws_no_image() {
        let mut h = Harness::builder()
            .with_size(vec2(300.0, 100.0))
            .build_ui(|ui| {
                let brand = Brand::new("Demo App", &INFO);
                show(ui, &brand);
            });
        h.run();
        assert!(h.query_by_label("Demo App").is_some());
    }
}
