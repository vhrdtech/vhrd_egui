# ve_spacemouse

3D mouse input for egui apps through [spacenavd](https://spacenav.sourceforge.net/), the free driver for 3Dconnexion devices, and a pie menu on the device's buttons.

- **The client** (`SpaceMouse`) speaks spacenavd's socket protocol in pure Rust: no libspnav, no C. A thread reads the events, and the UI drains them once per frame.
- **The mapping** (`Mapping`) says which raw axis moves, zooms or turns the content, with signs, dead zone, speed and which button is left or right. By default the content follows the hand; a camera convention is a setting. The calibrate step (`Calibration`) measures it with the person.
- **The settings** (`Config`, `SettingsPanel`) are shared by every app: one `spacemouse.toml`, found the way Cargo finds its config, so one calibration and one set of speeds apply everywhere. The panel is the same in every app.
- **The pie menu** (`PieMenu`) opens on the device's buttons, the way a pie menu over KiCad does on the same socket: tap for one action, hold for a ring of eight.

Try it: `cargo run -p ve_spacemouse --example demo`. It shows a canvas of dots that the puck moves, zooms and turns, pie menus on both buttons, the mapping as settings, and *Calibrate…*. `-- --socket PATH` connects to another socket.

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
offset += nav.pan_screen() * dt;                  // what the content does, convention included
scale *= (nav.zoom * dt).exp();
angle += nav.rotate[2] * dt;                      // clockwise on screen
```

`with_repaint` asks egui for a frame on every event, so the app needs no timer. The app keeps the `SpaceMouse` as transient state: it is rebuilt at start and never saved.

### The mapping and the calibrate step (SPM-2)

#### Content follows the puck

The mapping speaks of what the *content* does, not the camera. With `Convention::Content`, the default, the content follows the hand, as KiCad's 2D view with `reverse_pan_x` / `reverse_pan_y` and FreeCAD's object mode do:

| Motion | The page |
|---|---|
| slide right | moves right |
| lift up | moves up |
| push away | gets smaller (zoom out) |
| tilt forward | tilts forward, its top going away from you |
| rock right | turns clockwise on the screen, its right side going down |
| twist clockwise | spins clockwise seen from above, its right side coming towards you |

The puck is read in the screen's frame: up on the desk is up on the screen, towards you is towards the viewer. The three turns are about the screen's axes: tilt about the horizontal, twist about the vertical, rock about the axis towards you. So in a 3D view, rocking the puck rolls the model on the screen and twisting it spins the model like a turntable; a 2D page that turns on the screen turns with a rock.

`Convention::Camera` turns every output around, for people who think of the puck as the camera: slide right and the view moves right, so the page moves left. `Mapping::apply` applies the convention, so the `Nav` it returns is always what the content does: an app moves its content by it and never thinks about the convention.

#### The settings

`Mapping` is `serde` with `#[serde(default)]`: keep it in the app's settings. Each `Output` (move right, move up, grow, tilt forward, spin clockwise, turn clockwise) takes one raw axis (`x y z rx ry rz`) and a sign: the raw motion that makes the content do it under the content convention. A raw value of `full_scale` gives full speed, and the `dead_zone` part of the travel does nothing. `Speed` sets how fast a full push goes: pan in screen points per second (up to 15 000 in `mapping_ui`), zoom in log scale per second (up to 15), turn in radians per second (up to 30).

Apps pan in screen pixels, not in their own units: keep the content's offset in screen points and add `nav.pan_screen() * dt` to it, as the demo does. Then a push moves the page across the screen at the same speed at every zoom. Converting the pan to content units first (dividing by the zoom) would make it crawl when zoomed in and race when zoomed out. `mapping_ui` edits it by hand, with the convention and a live bar per output.

#### The calibrate step

The calibrate step measures the input instead of asking the person to describe it:

1. The panel asks for one physical motion at a time and says what it will do: "Slide the puck to the right. Hold it about 2 s, then let go. In the app the page moves right."
2. It records the motion events from the first push past 15 % of the full scale until the puck is back under 6 %. A hold shorter than 0.8 s is asked again.
3. The axis that moved most over the hold is the answer, and the sign of its mean is the direction. A second axis moving more than half as much is a mixed motion. The same axis for two motions is flagged too. Both get a ⚠ and a hint to redo.
4. Each `Motion` drives one output with a fixed sign (`Motion::drives`: push away is the page getting smaller, every other motion moves or turns the page with the hand), so the mapping follows from the measured axis and sign. Whatever `swap-yz` or `axismapN` say in `/etc/spnavrc`, the content follows the hand.
5. It asks for each button once and records spacenavd's number for it. One number can't be both sides.
6. A table shows what the page does on each motion ("push away: the page gets smaller"), with the raw axis, peak and hold time in the tooltip, and a *Redo* per row. A choice above it switches between content and camera, and the table follows. *Use this mapping* is enabled once every step is done and clean. The full scale becomes 80 % of the median peak, so full speed does not need the hardest push.

`Calibration::two_d` asks for slide right, lift up, push away and both buttons. `Calibration::full` asks for all six motions. `Calibration::feed` takes the time from the caller, so it is tested without a clock.

### One settings file for every app (SPM-5)

The mapping lives in `spacemouse.toml`, not in each app's settings. The files are found the way Cargo finds its `config.toml`, nearest first:

1. `.config/spacemouse.toml` in the current folder, then in each parent folder up to `/`;
2. `spacemouse.toml` in the user's config folder: `$XDG_CONFIG_HOME`, else `~/.config`. When the walk already met that file (the current folder is under the home folder), it is read once, at its place in the walk.

They are merged key by key, and the nearest file wins. Tables merge per key (`speed.pan`, `buttons.left`); the `axes` list is one key, because the calibrate step measures all six together. A key that no file sets takes `Mapping::default`'s value. So a project can make its pans faster with one line and keep the calibration from the user's file:

```toml
# ~/git/board/.config/spacemouse.toml
[speed]
pan = 3000.0
```

A file holds a `Mapping` at its top level: `convention`, `full_scale`, `dead_zone`, `[speed]` (`pan`, `zoom`, `rotate`), `[buttons]` (`left`, `right`) and six `[[axes]]` (`raw` 0 to 5 for `x y z rx ry rz`, `invert`), in the order move right, move up, grow, tilt forward, spin clockwise (about the screen's vertical), turn clockwise (on the screen). A file that does not read, or a value of the wrong type, is skipped and said; a key that means nothing is said and ignored.

`Config` reads the files. `Config::mapping` is the merged mapping, `Config::source(key)` the file a key came from, `Config::layers` the files found and `Config::candidates` every place looked at.

`Config::save(mapping, file)` writes into one file the keys it already had, with their new values, and every key whose value differs from what the farther files and the defaults give. A project file keeps only what the project changes. The save reports which keys a nearer file still sets to another value (`Saved::shadowed`), because those don't apply until that file changes. The rest of the file stays; comments in it are not kept.

#### The panel

`SettingsPanel` is what apps embed, the same in every app:

- *Calibrate…* opens the calibrate step in a window (`CalibrateSteps::TwoD` for a 2D view, `Full` for one that turns); *Use this mapping* there saves at once.
- The mapping by hand, as `mapping_ui`, with a *From* column: `user`, the project's folder (`board/.config`), `default`, or `changed` while not saved. The tooltip names the file.
- *Save to*: *every folder* (the user's file, the default) or *this project* (the nearest project file, or a new `.config/spacemouse.toml` in the current folder). *Save* writes there, *Reload* reads the files again.
- *Files*: the files found, nearest first, with the keys each sets, and under a fold of its own every place looked at with no file.

```rust
// once:
let mut panel = ve_spacemouse::SettingsPanel::load(ve_spacemouse::CalibrateSteps::Full);
// per frame:
let events = mouse.poll();
panel.feed(&events, ui.input(|i| i.time));
if !panel.calibrating() {
    view.navigate(panel.mapping().apply(&mouse.axes()));
}
panel.ui(ui, Some(&mouse.axes()));           // where the app keeps its settings
panel.calibration_window(ui.ctx());
```

Per-app things stay with the app: what the pie menu's slots do, and what the puck moves in a view with more than one thing to move.

#### Moving an app's old mapping

Apps kept the mapping in their own settings before. `Config::adopt(old_table, "where it was")` moves it into the user's file once: keys the user's file lacks are taken from the old section, keys it has keep their values. The app removes the old section only after `adopt` returned `Ok`, so a failed write loses nothing. better_erc_app does this with `[spacemouse]` of its `config.toml` at start.

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

`FakeSpacenavd` listens on a socket of its own and sends the events a test asks for. It speaks the same protocol, so the client cannot tell it apart: `tests/socket.rs` covers events, a restart and a late daemon with it. `tests/convention.rs` plays a whole calibration through it and checks the derived signs: the content follows the hand, and the camera convention turns it around. The pie menu (`tests/pie.rs`), the calibrate panel (`tests/calibrate.rs`) and the settings panel (`tests/panel.rs`, on a temporary folder tree) are headless egui_kittest tests with snapshots; the state machines have unit tests.

## Not a web crate

The client needs Unix sockets and a thread. `client` and `fake` build only on Unix. The mapping, the calibration and the pie menu are plain egui and build anywhere.
