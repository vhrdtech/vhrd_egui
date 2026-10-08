//! What a widget implementation needs: `use ve_app::prelude::*;`.
//!
//! The app crate still depends on `serde` directly (the derive needs it); `typetag` and `inventory` resolve
//! through this prelude.

pub use egui::{self, Id, Ui, WidgetText};
pub use inventory;
pub use serde::{Deserialize, Serialize};
pub use typetag;
pub use ve_widget::context::Context;
pub use ve_widget::{Widget, WidgetInfo, erased_serde, svt};

pub use crate::{Repainter, Shell, ShellOptions};
