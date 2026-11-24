# ve_update

The update bar of a native egui app (CHR-9): "New version installed: 0.4.2 (a1b2c3d, 2 h ago) · Restart" when an installer put a newer release in place, and a menu of the kept releases with "Switch to this one".

The app implements `Releases` over its installer and keeps a `Relaunch`:

```rust
let relaunch = ve_update::Relaunch::new();
let viewport = match ve_update::PlacementStore::runtime().take_own() {
    Some(p) => p.apply(viewport),
    None => viewport,
};
// in the app's bar:
ve_update::UpdateBar::new(&releases, &relaunch).busy(busy_reason).show(ui);
// after eframe::run_native returns:
relaunch.exec(&ve_update::PlacementStore::runtime())?;
```

On Unix the restart replaces the process, so its pid, parent and arguments stay.
