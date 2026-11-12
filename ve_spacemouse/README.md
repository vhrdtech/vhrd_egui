# ve_spacemouse

3D mouse input for egui apps through [spacenavd](https://spacenav.sourceforge.net/), the free driver for 3Dconnexion devices, and a pie menu on the device's buttons.

- **The client** (`SpaceMouse`) speaks spacenavd's socket protocol in pure Rust: no libspnav, no C. A thread reads the events, and the UI drains them once per frame.
- **The mapping** (`Mapping`) says which raw axis pans, zooms or rotates the view, with signs, dead zone, speed and which button is left or right. It is a per-app setting. The calibrate step (`Calibration`) measures it with the person.
- **The pie menu** (`PieMenu`) opens on the device's buttons, the way a pie menu over KiCad does on the same socket: tap for one action, hold for a ring of eight.

Try it: `cargo run -p ve_spacemouse --example demo`. It shows a canvas of dots that the puck pans and zooms, pie menus on both buttons, the mapping as settings, and *Calibrate…*. `-- --socket PATH` connects to another socket.

## Setup

spacenavd must run and own the device:

```sh
sudo pacman -S spacenavd        # or yay -S spacenavd; other distros: their spacenavd package
sudo systemctl enable --now spacenavd
```

It listens on `/var/run/spnav.sock`. Its settings live in `/etc/spnavrc`: sensitivity per axis, `swap-yz`, `axismapN`. Those change what the app receives. Run the calibrate step again after changing them.

## How it works

### The socket (SPM-1)

A client that connects and sends nothing gets spacenavd's original protocol. Every event is eight native-endian `i32`s:

| Word | Motion (type 0) | Button press (1) / release (2) |
|---|---|---|
| 0 | `0` | `1` or `2` |
| 1-6 | `x y z rx ry rz` | the button number, then zeros |
| 7 | ms since the last motion event | 0 |

spacenavd sends a motion event whenever the deflection changes, and all zeros when the puck comes back to rest. `SpaceMouse::axes()` is the last motion, so it holds the deflection now.

The client thread reconnects every 2 s (`Options::retry`) when spacenavd is not there or restarts. `SpaceMouse::status()` says why it is down and what to do ("no socket at /var/run/spnav.sock: is spacenavd installed and running?"). On a lost connection the client releases every button it saw held and puts the axes back to rest, so a pie menu closes and the view stops moving. Dropping the client wakes the thread and joins it at once.

```rust
let mouse = ve_spacemouse::SpaceMouse::start(Default::default()).with_repaint(&cc.egui_ctx);
// per frame:
let events = mouse.poll();
let nav = mapping.apply(&mouse.axes());           // per second
let dt = ui.input(|i| i.stable_dt);
view.pan -= nav.pan_screen() * dt;                // the view pans right: content moves left
view.scale *= (nav.zoom * dt).exp();
```

`with_repaint` asks egui for a frame on every event, so the app needs no timer. The app keeps the `SpaceMouse` as transient state: it is rebuilt at start and never saved.

### The mapping and the calibrate step (SPM-2)

`Mapping` is `serde` with `#[serde(default)]`: keep it in the app's settings. Each `Output` (pan right, pan up, zoom in, tilt forward, turn right, spin clockwise) takes one raw axis (`x y z rx ry rz`) and a sign. A raw value of `full_scale` gives full speed, and the `dead_zone` part of the travel does nothing. `mapping_ui` edits it by hand, with a live bar per output.

The calibrate step measures the input instead of asking the person to describe it:

1. The panel asks for one motion at a time: "Move the puck the way that should pan the view right. Hold it about 2 s, then let go." So the person's own idea of pan and zoom decides, whatever `swap-yz` says.
2. It records the motion events from the first push past 15 % of the full scale until the puck is back under 6 %. A hold shorter than 0.8 s is asked again.
3. The axis that moved most over the hold is the answer, and the sign of its mean is the direction. A second axis moving more than half as much is a mixed motion. The same axis for two outputs is flagged too. Both get a ⚠ and a hint to redo.
4. It asks for each button once and records spacenavd's number for it. One number can't be both sides.
5. A table shows what each motion will do in the app, with the raw axis, peak and hold time in the tooltip, and a *Redo* per row. *Use this mapping* is enabled once every step is done and clean. The full scale becomes 80 % of the median peak, so full speed does not need the hardest push.

`Calibration::two_d` asks for pan right, pan up, zoom in and both buttons. `Calibration::full` asks for all six outputs. `Calibration::feed` takes the time from the caller, so it is tested without a clock.

### The pie menu (SPM-3)

| Do | Get |
|---|---|
| Tap a button (released within 0.25 s, pointer still in the centre) | its centre slot fires; the menu never shows |
| Hold a button | a ring of slots opens at the pointer after 0.25 s |
| Move out across the outer circle | the slot under the pointer fires at once and flies off |
| Back through the centre, out again | the next action: actions chain while the button is held |
| Click | the slot under the pointer fires; a click on a free or disabled slot closes the menu |
| Release | closes; nothing fires |
| Press the other button too | the third ring, of both buttons |

The centre has two halves, one per button. The pressed button's half is lit until the pointer moves; then the half under the pointer is lit. Every slot shows its icon and its name. A free slot says *Free*, and a disabled one (`Slot::disabled(why)`) is drawn muted. Under the pointer, either one says below the circle why it does nothing. `Fired::at` is where the menu opened, the place the action is about.

The menu draws on egui's foreground layer over the whole window and takes the clicks while open, so nothing under it reacts. A press while the window has no focus, or with the pointer outside it, is ignored. Colours come from `ve_theme` tokens: `surface` for the slices, `accent` for the lit one.

```rust
let menu = PieMenu {
    center_left: Some(Slot::new("Highlight net", icons::HIGHLIGHTER, Action::Highlight)),
    ring_left: vec![Some(Slot::new("Copy", icons::COPY, Action::Copy)), None, /* ... 8 in all */],
    ..Default::default()
};
for fired in menu.show(ui.ctx(), &mut pie_state, &ve_spacemouse::buttons(&events, &mapping)) {
    app.act(fired.action, fired.at);
}
```

Call `show` after the app's panels. `ve_spacemouse::install_icons(ctx)` (feature `icons`, on by default) adds egui-phosphor's regular icons behind the app's fonts; `ve_spacemouse::icons::*` are their glyphs.

## Testing

`FakeSpacenavd` listens on a socket of its own and sends the events a test asks for. It speaks the same protocol, so the client cannot tell it apart: `tests/socket.rs` covers events, a restart and a late daemon with it. The pie menu (`tests/pie.rs`) and the calibrate panel (`tests/calibrate.rs`) are headless egui_kittest tests with snapshots; the state machines have unit tests.

## Not a web crate

The client needs Unix sockets and a thread. `client` and `fake` build only on Unix. The mapping, the calibration and the pie menu are plain egui and build anywhere.
