# ve_native

The eframe `NativeOptions` every vhrd egui app starts from (PLT-11). Build the app's options on top of them:

```rust
let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default().with_app_id("my_app"),
    renderer: eframe::Renderer::Wgpu,
    ..ve_native::native_options()
};
eframe::run_native("my_app", options, Box::new(|cc| Ok(Box::new(MyApp::new(cc)))))
```

## Why

Hyprland and other wlroots compositors stop sending frame callbacks to a window on a hidden workspace. A swap with vsync (glow with `vsync`, wgpu with `PresentMode::AutoVsync` or `Fifo`) waits for the next callback, so the event loop blocks until the workspace is shown again, and the compositor flags the app as not responding. eframe's defaults have vsync on.

`native_options()` turns vsync off when the app runs on Wayland (`WAYLAND_DISPLAY` or `WAYLAND_SOCKET` set, the way winit picks its backend): glow swaps without waiting, wgpu presents `AutoNoVsync`. X11, Windows and macOS keep vsync. egui still paints only when asked, so an idle window costs nothing. With wgpu, winit still holds each redraw until the next frame callback, so a visible window keeps the display rate; glow has no such hold, so a glow app that repaints every frame paces itself with `request_repaint_after`.

## Features

| Feature | Sets |
|---|---|
| `glow` (default) | `glow_options.vsync` |
| `wgpu` (default) | `wgpu_options.surface.present_mode` |

An app on one renderer turns off the default features and names its own, e.g. `default-features = false, features = ["wgpu"]`. A renderer the crate is built without keeps eframe's vsync, so name every renderer the app can use. Without either feature only `Session` is there, for a host that drives wgpu itself (`ve_phosphor`).
