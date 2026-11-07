# UX guide: how our GUIs look and behave

This page is for every GUI built on vhrd_egui: the egui apps, their web builds and Telegram Mini Apps, and the buttons of a Telegram bot. It says what each rule is and why it holds. The egui code for each rule (helper names, snippets) is in the skill reference [`egui_app_skill/references/ui-guide.md`](../egui_app_skill/references/ui-guide.md); this page says what and why, that page says how in egui.

Its command-line twin is the [CLI UX guide in vhrd_cli](https://github.com/vhrdtech/vhrd_cli/blob/main/docs/ux-guide.md). Both pages have the same shape. The rules every UI shares, GUI and CLI alike, live once, in [section 2](#2-rules-for-every-ui) here, and the CLI guide links to them.

Outside practice is borrowed where it fits, each point marked with its source.

How to read it:
- Each rule is a bold line, then its parts, then why, then an example, then its source in brackets.
- "House rule" means a rule all our projects share; "user, date" is the owner's own word in a task or note.
- 🔜 marks a part that is planned and not built yet.
- Feature IDs (`BAS-5`, `FRM-1`, ...) point into [FEATURES.md](../FEATURES.md); task ids like are references into our project tracker.

## 1. The short version

1. **Nothing is hidden.** A cut list ends in a fold that says what is in it, never in "N more". [house rule]
2. **Disabled things stay visible and say why**, and what would enable them. [house rule]
3. **Every control, number and badge explains itself on hover**: what it is, its unit, what a click does. [vhrd_egui]
4. **Related things light up together on hover.** [vhrd_egui]
5. **Colours and sizes come from the theme**, dark and light; red and orange mean trouble only. [vhrd_egui]
6. **Nothing jumps**: learned widths, fixed bar steps, tabular digits. [vhrd_egui]
7. **Feedback is near what it is about** and matches its weight: a status line, a toast, an alert only for real danger. [another in-house app, Apple HIG]
8. **Actions are named as actions**, and a tap that throws work away asks first or can be undone. [user, Apple HIG]
9. **The UI never freezes**: slow work runs off the UI thread and shows progress. [vhrd_egui]
10. **Errors say what happened and what to do**, in the UI, never as a panic. [UX checklist, Nielsen 9]
11. **Every widget sits in the shared frame**, and reusable UI lands in vhrd_egui, not in the app. [vhrd_egui]
12. 🔜 **Everything works from the keyboard.** [user 27 Oct 2026, GNOME HIG]

## 2. Rules for every UI

These hold in a GUI, a web page, a bot and a CLI alike. The CLI guide links here for them.

- **Nothing is hidden: the rest goes under a fold, never a count.** [house rule, user 20 Oct 2026]
  - A list cut for room (rows, subagents, sessions, log lines, chips, a bot reply's list) never ends in "N more" or "…and N others" alone, and never drops items silently.
  - The rest sits under a fold, collapsed by default. Its header says what is in it and how many. Opening it shows every item.
  - A tooltip that lists the items is a nice extra, not a replacement.
  - Where nothing folds (a bot, a CLI), the rest is offered: a button, `--all`.
  - It is built this way from the first version, not added on review.
  - Two narrow exceptions in egui. A widget's smallest fixed form (`min` of its size switch) may sum a list up as a count ("usb 3 devices") when the count's tooltip lists the items and names the size switch. A popup on a `+N` stands in for the fold in a fixed-width cell with no room below it. [ui-guide.md]
  - Why: a count hides exactly the item the person was looking for, and they cannot tell whether it is there. [Nielsen 1, 6]
  - Example: "3 more subagents, not busy ▸" opens to all three rows (`ve_basics::fold_rest`, BAS-5); the bot ends a long list with a "Show all 14" button.
- **Disabled things stay visible and say why.** [house rule, user 8 Oct 2026]
  - Never hide a control, row or action because it can't be used now. Show it disabled.
  - Its tooltip says why it is disabled and what would enable it. Where there is no hover (a phone, a bot), a short line next to it says it.
  - When the cause is on screen (a full login, a stopped node, a missing setting), hovering the disabled item highlights the cause.
  - Why: a UI that changes shape with state surprises people; they look for a button that vanished and don't learn when it comes back. [another in-house app "Minimize surprises"]
  - Example: a greyed "Open transcript" whose tooltip reads "not clickable: interactive session, no transcript served"; hovering a greyed "Deploy" lights up the stopped node it waits on (`hover_link`).
- **Measure the input, then let the person confirm.** [house rule, user 30 Oct 2026]
  - Setup that isn't obvious (axis mapping, calibration, which button is which, sensor orientation) is a guided recording: a few motions one at a time, the raw input recorded, the mapping worked out from the data, then a table of expected behaviour to confirm.
  - It ships as a step of the tool (a setup wizard in the GUI, a `calibrate` subcommand), not a one-off script.
  - Why: people describe what they saw badly, and guessing fixes from a description takes many rounds.
  - Example: a joystick setup page says "push the left stick up and hold 2 s", records it, and shows "left stick up → pitch +" for the person to confirm.
- **Say why when doing something non-obvious.** [vhrd_egui design notes, 8 Oct 2026]
  - Why: an action with no reason looks like a bug. [Nielsen 1]
  - Example: "Installing ve_theme 0.32, because the gallery needs THM-15".
- **Text a person reads is in their language and in plain words.** [our policy-engine app AGENTS.md, UX checklist]
  - Labels, notices and button texts come from locale files, never from strings in the code (our policy-engine app: `locales/<lang>/*.ftl`).
  - Words are the person's, not our internal names. [Nielsen 2]
  - Why: a Russian-speaking user should not meet English labels, and an internal id on a button means nothing to anyone.
  - Example: a bot button reads "Остановить наблюдение" for a Russian chat, "Stop watching" for an English one.

## 3. Look: theme, colour, type

- **Every app uses the shared theme.** [vhrd_egui AGENTS.md]
  - `ve_theme::setup(&cc.egui_ctx)` at startup installs the IBM Plex fonts and the dark and light styles; egui follows the system preference.
  - Colours, spacing, radii and type come from `Tokens` (`ui.tokens()`), never hex literals or `Color32` in app code. What `Style` can't express goes into a `UiExt` helper.
  - The look is tuned in the gallery (`cargo run -p ve_theme --example gallery`), and its snapshots are updated with it.
  - Why: one place to change the look, and apps that look like one family. [UX checklist "the same task done the same way in every app"]
  - Example: `ui.tokens().colors.warn` for a warning, not `Color32::from_rgb(230, 140, 0)`.
- **Status colour never carries meaning alone.** [ui-guide.md; Apple HIG Accessibility "doesn't rely on any single method"]
  - Good, warn, crit and off always come with a text label or icon.
  - Chart series accents get direct labels on the chart.
  - Why: colour-blind readers, grey screens and screenshots in a chat lose the colour. The two series accents in particular sit in the colour-vision band where labels are required to tell them apart.
  - Example: a red LED with "crit: disk 97 %" beside it, not the LED alone.
- **Categorical colours avoid red and orange.** [vhrd_egui AGENTS.md, user 9 Oct 2026]
  - Labels, chips, tags, series, hosts and models never use red, orange or red-ish brown (OKLCH 5°-85°, `ve_theme::hue::ALERT_HUES`). Those hues mean error and warning (the `crit`, `warn` and `red` tokens).
  - Colours that share a view are clearly distinct (OKLab ΔE, hue gap), checked by a unit test in both themes (`hue::is_alert_hue`, `hue::delta_e`, `hue::hue_gap`).
  - Why: an orange host name reads as a warning that isn't there.
  - Example: hosts get blue, green, violet and teal; a failing host gets its red LED, not a red name.
- **A status glyph is drawn in its tone.** [ui-guide.md, THM-15, DASH-25]
  - A status emoji (✅ 🚧 ⛔ …) is painted in its tone's colour, never as a plain character inside a label, where it takes the text colour.
  - Why: egui draws every glyph as a one-colour mask, so ⛔ in a label turns grey and loses its meaning.
  - Example: `ve_theme::status_icon(ui, name, tooltip)`; in a `GanttRow` the glyph goes in its `glyph` field, the status word in its hover.
- **Brand colours outside egui come from one palette too.** [vhrd_brand AGENTS.md]
  - Typst documents and KiCad themes use `brand.*` and `signal.*` from `palette.typ`, never hex values.
  - Why: a datasheet and the app that drives the board should look like the same company.
  - Example: a block diagram uses `signal.power`, the same red-free family the app uses for series.

## 4. Help and explanation

- **Generous tooltips on everything.** [vhrd_egui AGENTS.md]
  - Every control, number, badge, status light and abbreviation has one: what it is, its unit, what clicking or dragging does, and what the value means when that isn't obvious.
  - Multi-line is fine. Name the unit (`ms`, `%`, `MiB/s`), the range and where the number comes from.
  - If a user could ask "what is this?", hovering answers it.
  - Why: our apps are dense engineering screens; the tooltip is the manual, one hover away. [another in-house app "Help is one click away"; Nielsen 10]
  - Example: "12.4 ms" → "Round trip to the node over the mesh, median of the last 10 pings".
- **A tooltip adds to the label, it doesn't repeat it.** [GNOME HIG Tooltips]
  - Why: "Deploy" with the tooltip "Deploy" wastes the hover.
  - Example: "Deploy" → "Builds the release binary and installs it on this PC (just install); no server is touched".
- **Where there is no hover, the essential part is a visible line.** [GNOME HIG Tooltips "don't rely on tooltips to communicate essential information"; house rule "disabled things say why"]
  - On touch (the Mini Apps, phones) the reason a thing is disabled, or what a number means when it matters, is a short line or a tap-to-explain, not a tooltip.
  - Why: a tooltip doesn't exist on a phone.
  - Example: a phone page shows "read-only: you are a viewer" under its greyed buttons.
- **Build info on hover and one app menu in every app.** [vhrd_egui AGENTS.md, BAS-1, BAS-4]
  - The top bar shows `version · sha · debug|release` at its right; its tooltip gives the full version, SHA, build time and profile, the same data `--version` prints.
  - The version badge opens the shared app menu (`AppMenu`): name, description, build, and "Reset storage to defaults…", which asks first. `ve_app` has it in *Help* and on the status bar.
  - The window title shows the app name.
  - Why: a bug report needs the exact build, and every app should answer "which one is this?" the same way.
  - Example: `ve_basics::build_info_label(ui, &build_info!())` at the right of the top bar.
- 🔜 **Context help on F1 and help searched in the app.** [ UX sweep of CAD tools: Altium, COMSOL; Nielsen 10]
  - Why: the docs page that explains a panel should be one key away from it.
  - Example: F1 over the meter opens its section of the app's help.

## 5. Hover, selection and copy

- **Related items light up together on hover.** [vhrd_egui AGENTS.md, BAS-3]
  - When one thing refers to another (a session to its PC, a lane count to its sessions, a chart line to its legend entry, a row to its chart and status light), hovering one highlights the others.
  - Why: the eye finds the connection without reading. [Nielsen 6]
  - Example: hover "eth0" in the table and its rx chart and status light light up (`ve_basics::hover_link(ui, "iface-eth0", &r)`).
- **Labels are not selectable; text worth copying is.** [vhrd_egui AGENTS.md, BAS-2, BAS-7]
  - Plain labels are unselectable, set once at startup.
  - Ids, paths, commands and SHAs are copyable; so is a region worth copying, such as a detail panel or a log.
  - Why: selectable labels swallow clicks meant for the widget under them and show a text cursor everywhere.
  - Example: a task id in the dash copies on click; the word "status" next to it does not.
- 🔜 **Shift-hover shows the equivalent command.**
  - Any element hovered with a modifier shows the Rust call, Python statement, CLI command or file text it would produce, each with a copy button.
  - Why: what the person clicked becomes a script or an agent's step without reverse engineering. It is planned for one CAD app first; a general `ve_app` version is an idea.
  - Example: Shift-hover on "Export STEP" shows the CLI command that writes the same `part.step`.

## 6. Lists, numbers and layout

- **Percent bars use a fixed block count.** [vhrd_egui AGENTS.md, DASH-4, DASH-13]
  - 10 blocks by default; 20, 25, 50 or 100 where finer steps help. `Blocks::Auto` (the count grows with the width) only where the width itself should carry meaning, not for ordinary load bars.
  - Any non-zero value lights at least one block; the tooltip says the percentage and the block count.
  - Jumpy per-frame values are smoothed (`ve_dash::Decay`) before they drive a bar.
  - Why: bars across an app stay comparable, a 0.4 % load is not shown as nothing, and the eye can follow a smoothed 60 Hz bar.
  - Example: `"63 % (6 of 10 blocks)"`.
- **Widths are learned, never hard-coded.** [vhrd_egui AGENTS.md, user 8 Oct 2026]
  - Anything whose content changes length (live numbers, names, a column shared by rows) keeps the widest it has been (`ve_dash::steady_width`, `StatTile::steady()`).
  - No fixed pixel widths in app code. Numbers use tabular digits.
  - Why: a layout that jumps every frame can't be read.
  - Example: a "1.2 MiB/s" tile that grows to "12.4 MiB/s" stays that wide when the rate drops again.
- **Every widget sits in the frame.** [vhrd_egui AGENTS.md, FRM-1]
  - No home-made title bars, panel borders or size toggles in app code. `ve_frame::WidgetFrame` has the parts.
  - What a widget has to say goes into the frame's parts: its state on the LED, what it is and its stats into the ⓘ, its smallest fixed form and its normal and wide arrangements into the sizes, extra controls on the sides or in the footer.
  - Content that can outgrow its place scrolls.
  - Why: every panel in every app has the same handles, so people learn them once. [Nielsen 4]
  - Example: a `ve_app` widget returns a `FrameSpec` from `Widget::frame_spec`; try `cargo run -p ve_frame --example playground`.
- **The UI keeps its shape.** [another in-house app "Minimize surprises"]
  - Prefer all options shown, some greyed out with a reason, over a UI that changes shape with state.
  - Why: people find things where they were last time.
  - Example: the toolbar of a disconnected device keeps every button, greyed, with "connect a device first".
- **Big data stays fast.** [another in-house app AGENTS.md]
  - Segment and decimate what is drawn, never walk a whole track per frame, and cap tooltips on overlapping events.
  - Why: a frame that takes 200 ms makes every hover feel broken.
  - Example: a 10-million-sample trace draws one min-max pair per pixel column.
- 🔜 **Windows, tabs and layouts are the person's.** [vhrd_egui design notes, 8 Oct 2026]
  - A tab can become a window and back, and move to another monitor.
  - A widget can be maximized: menus, bars and tiling controls hide; with OS fullscreen it gets every pixel.
  - Multi-window, multi-monitor layouts are saved in a clear file a person can edit (window name and kind, position, screen UUID).
  - Why: lab benches have two or three screens, and a layout that took ten minutes to arrange must survive a restart.
  - Example: `layout.toml` with `[[window]] kind = "scope" screen = "DELL-U2723" x = 0 y = 0`.

## 7. Controls, menus and dialogs

- **A list of checkboxes stays open while it is ticked.** [ui-guide.md, BAS-6]
  - A dropdown of checkboxes (a filter's values, the columns to show) closes only on a click outside, Esc or its own button.
  - A menu of actions, where one click does one thing, closes after the click.
  - Why: a menu that closes after every click makes ticking three values three trips.
  - Example: "tracks: 3 ▾" stays open while the person ticks dev, gui and dash (`ve_basics::Checklist`).
- **Buttons are labelled as actions.** [user; Apple HIG Buttons "start the label with a verb"]
  - A button says what it does, starting with a verb. It never reads like a link or a heading.
  - A button that opens another window or dialog ends in "…". [Apple HIG Buttons]
  - Why: "⏹️ messages from kicad-textconv" was tapped to read the messages and stopped the watch instead.
  - Example: "Stop watching: messages from kicad-textconv"; "Reset storage to defaults…".
- **The most likely action stands out; a destructive one never does.** [Apple HIG Buttons]
  - One or two prominent buttons per view at most.
  - A destructive action never gets the primary, Return-key role, even when it is the likely choice.
  - Why: people press the prominent button without reading it.
  - Example: in "Delete 3 layouts?" the prominent button is "Keep", and "Delete" is plain with the crit tone.
- **A tap that throws work away asks first, or can be undone for a few seconds.** [user 11 Oct 2026; Apple HIG Feedback]
  - The question says in plain words what will be lost. On touch screens the confirm step is the default, since a mis-tap is easy.
  - Where an action can be undone for a few seconds, that can replace the question.
  - After the action, say what it meant.
  - Why: one stray tap on Undo silently dropped an agent's file edits in a group chat.
  - Example: "Undo this turn? The 3 files the assistant changed go back to how they were." then "↩️ undone by Alex".
- **Undo covers every edit and names what it undoes.** [Apple HIG Undo and Redo; UX sweep: undo in every CAD tool surveyed; egui_tabular AGENTS.md]
  - Many steps, not one. The menu item names the step ("Undo Move 3 parts"). After an undo, the restored thing is shown, scrolled into view if needed.
  - Why: people press undo until something changes; if they can't see what changed they press too often.
  - Example: egui_tabular keeps each command's inverse for undo.
- **Interruptions match their weight.** [another in-house app AGENTS.md; Apple HIG Feedback]
  - No banners or windows popping up over the work. Short notices are toasts; things that need attention blink in place; changes fade with smooth colour or opacity transitions.
  - An alert is only for something critical and actionable, such as data loss the person didn't expect.
  - Why: an app that cries wolf trains people to close alerts unread.
  - Example: "Saved" is a toast; "The device firmware is older than this app can talk to" is an alert with an "Update firmware…" button.
- **A warning sits next to what it is about.** [vhrd_egui design notes; Apple HIG Feedback "status near the items it describes"]
  - An entry that doesn't validate shows its message beside the field, for as long as it is wrong.
  - Why: a warning in a status bar far away is not connected to the field by the eye.
  - Example: "1.2.3.4.5 is not an IPv4 address" in warn colour right under the field.
- **Nothing updates itself unasked.** [another in-house app AGENTS.md]
  - Updates install only when the person asks, and older versions stay selectable.
  - Why: a bench tool that changes mid-measurement ruins the measurement.
  - Example: the app menu says "0.33 is out: Update…" and the version list keeps 0.31 and 0.32.
- 🔜 **One modal layer.** [from Zed's workspace modal layer]
  - A dialog, picker or confirm shows on top in one place, takes keyboard focus, closes with Escape or a click outside, and returns a typed result.
  - Why: every app builds its own modal today, each closing differently.
  - Example: `ve_app` opens a confirm and gets back `Confirm::Yes`.
- 🔜 **Settings forms are made from the settings schema.** [; house rule "every tunable is a setting"]
  - Typed layered settings in `ve_settings`, the form generated from the schema with the disabled-says-why and nothing-hidden rules, the same schema the config portal uses.
  - Why: no form written by hand per setting, and the app and the portal never disagree.
  - Example: a `max_rows: u32` field with its doc line becomes a number box with that tooltip in both places.

## 8. Keyboard

All 🔜: these come from the owner's own wish and GNOME HIG, and the egui layer for them is not built yet.

- 🔜 **Every part of the UI works without a mouse.** [user 27 Oct 2026; GNOME HIG Keyboard "every action should also be possible with the keyboard"]
  - Focus moves with Tab and Shift+Tab between panes and with arrows inside lists and tables. A visible focus ring shows where it is.
  - Return activates, Space toggles, Escape backs out of menus and dialogs. [GNOME HIG Keyboard]
  - It is tested headless with egui_kittest.
  - Why: keyboard-first speed is what makes tools like Linear and Zed feel fast; and some people can't use a mouse. [ UX review]
  - Example: Tab from the session list into the detail panel, arrows down three rows, Escape back.
- 🔜 **Named actions, a keymap file and a command palette from one registry.** [Zed; UX sweep: command palette and remappable hotkeys]
  - Each action has a name; keys bind to actions in a user-editable keymap file, chords included; the command palette lists every action with its key.
  - Standard keys keep their standard meaning (Ctrl+Z, Ctrl+Shift+Z, Ctrl+F). [GNOME HIG Keyboard, Apple HIG Undo]
  - Why: a person learns the palette once and finds every key from it. [Nielsen 7]
  - Example: Ctrl+Shift+P, type "fold", Return opens every fold in the view.
- **Keyboard behaviour is documented and tested where it exists today.** [egui_tabular AGENTS.md]
  - A changed key or mouse gesture updates the keyboard and mouse table in the README and gets a test.
  - Example: egui_tabular's selection keys in `tests/ui/keyboard`.

## 9. Speed, state and errors

- **The UI thread stays free.** [vhrd_egui AGENTS.md, another in-house app AGENTS.md]
  - No blocking I/O or long computation inside `ui()`. Long work runs elsewhere, and the UI polls for its result per frame.
  - A crashed worker reports its error to the UI instead of vanishing.
  - Why: a frozen window looks like a crashed app.
  - Example: a device scan runs on a thread and the panel shows "scanning USB… 3 found".
- **Slow work shows its progress.** [CLI guide "Progress"; Nielsen 1; Apple HIG Buttons activity indicator]
  - Anything that can take more than a few seconds shows what runs and how far; a button that started it shows that it is working.
  - Why: without feedback people click again.
  - Example: "Deploy" turns into "Deploying… 2 of 5 PCs".
- **Three kinds of state, kept apart.** [vhrd_egui AGENTS.md, another in-house app AGENTS.md]
  - Settings or seed (what the person configured) and visual state (layout, view) are saved; transient state (connections, caches, devices) is rebuilt at start (`Widget::init` in `ve_app`) and never saved.
  - Saved state loads from older files: new fields get defaults.
  - Why: an app must open after an update, and a saved socket is meaningless.
  - Example: a widget's device connection is skipped when saving and opened again at start.
- **Errors say what happened and what to do.** [UX checklist "error messages that don't say what to do"; Nielsen 9; vhrd_egui AGENTS.md]
  - No `unwrap` or `expect` on data from disk, the network or the user. The error shows in the UI (toast or inline), in plain words, with the next step.
  - Why: a panic loses the person's work and tells them nothing.
  - Example: "Can't open layout.toml: line 4, unknown key 'scren'. Fix the line or Reset layout…".
- **The app checks itself after an update and can go back.** [house rule "Everything recovers by itself"; PLT-4]
  - A GUI runs its own self-check after an install; if it fails, the previous release comes back by itself, and a person can roll back by hand.
  - A start that died is noticed at the next start, and the saved state that caused it is moved aside (`ve_basics::StartupGuard`).
  - Example: our internal release tool's per-app rollback command.
- 🔜 **A web build checks the browser before a big download.** [vhrd_egui design notes]
  - Why: a 20 MB wasm bundle that can't run in this browser wastes the download and shows a blank page.
  - Example: "This browser has no WebGPU: the 3D view won't work. Open anyway?" before fetching the bundle.

## 10. Web pages and Telegram Mini Apps

Everything above applies; these are the differences.

- **No hover: reasons and explanations are visible lines.** See [section 4](#4-help-and-explanation). [GNOME HIG Tooltips; house rule]
- **Page text is built safely.** [a third in-house app AGENTS.md]
  - Build the DOM with `textContent`, never `innerHTML` with data. Plain JS, no build step, no outside libraries beyond Telegram's `telegram-web-app.js`.
  - Why: data from people and mail is untrusted, and a page that runs it is a hole.
  - Example: `row.textContent = task.title`.
- **Forms come from the schema.** [a third in-house app AGENTS.md; house rule "every tunable is a setting"]
  - The portal's forms are generated from each tool's typed settings schema, never written by hand per setting.
- **What the page asks of Telegram lives in one place.** [the mesh dashboard app AGENTS.md]
  - Colours and the back button come from Telegram through one module (in the dash: `src/telegram.rs` and `web/boot.js`).
  - Why: the Mini App should look like part of the chat and follow its theme.

## 11. Telegram bot buttons

A bot has no hover and no fold. The [section 2](#2-rules-for-every-ui) rules still hold, in this form.

- **The rest is a button, not a count.** [house rule "Nothing is hidden"]
  - Example: a list of 40 tasks shows 10 and a "Show all 40" button.
- **A button that can't be used now stays, with a short line that says why.** [house rule "Disabled things say why"]
  - A used button stays visible in its used state instead of disappearing. [user]
  - Example: "↩️ undone by Alex" where the Undo button was.
- **A button is offered, never told.** [our policy-engine app AGENTS.md]
  - A release note gets one quiet line with a button, once; texts never say "then /new".
  - Why: a person chooses; the bot doesn't give orders.
- **A button's text is built from known values, never from model output.** [our policy-engine app policy model]
  - Example: "Always for Alex" is built from the rule's key and the person, not from what the model wrote.
- **A wall has no button.** [our policy-engine app policy model]
  - A refusal that nobody can lift from the chat says it is a wall and offers nothing to press.
  - Why: a button that can't work trains people to press buttons blindly.
- **Labels are actions, risky taps ask first, and each tap is logged with who pressed it.** See [section 7](#7-controls-menus-and-dialogs). [user]
- **Every button a person can see is tested.** [our policy-engine app AGENTS.md]
  - Each user-visible behaviour (a command, a button, a file) gets a named end-to-end scenario against a fake Telegram server in the test run, and on Telegram where Telegram itself could refuse it.

## 12. Where the code lives and how it is checked

- **Reusable UI lands in vhrd_egui, not in the app.** [vhrd_egui AGENTS.md; house rule "egui stack"]
  - A widget, layout helper or fix another app could use goes into vhrd_egui (ve_frame, ve_dash, ve_basics, ve_theme) with a doc example; the app calls it. A big one gets its own crate (egui_tabular, eventfull).
  - When a sync or a talk turns up a reusable piece inside an app, file an extraction task on.
  - Why: the same task should be done the same way in every app. [UX checklist]
- **One egui version for the whole stack.** [vhrd_egui AGENTS.md, PLT-1]
  - `egui` and `eframe` sit in the root `[workspace.dependencies]`; a bump says which apps must follow.
- **UI behaviour is tested headless.** [vhrd_egui AGENTS.md, egui_tabular AGENTS.md]
  - egui_kittest tests, as egui_tabular does; the theme gallery keeps image snapshots.
  - A bug fix starts with a failing test when the bug can be reproduced in one.
- **A new screen passes the UX checklist.** [UX checklist]
  - Who uses it and for what; defaults that work; errors that say what to do; does the pattern already exist in vhrd_egui; tried by someone who didn't build it.
  - An agent reviews screenshots against this page.
- **A feature a person can learn gets its skill.** [house rule "Skill tree"]
  - A new view, button or gesture records a usage signal and gets its skill in the skill tree, or a task.

## 13. Looks to borrow from

Ideas, not rules. [vhrd_egui design notes, 8 Oct 2026; UX review]
- Zed (density, the command palette, the modal layer), Oxide Computer (dark scheme with good contrast, ASCII art, animations, the 3D view), ZSA, the yevm page by sergey-melnychuk (background gradient, frames, hover-out animation), Bevy's variables panel (pane, subpane, group).
- Linear and Notion for keyboard-first speed, Figma and Blender for tool modes and operator search, Ableton and Bitwig for modulation you can see.
- Talks: Bret Victor "Inventing on principle", Jack Rusher "Stop writing dead programs".
- Borrowing rule: ideas and mechanics are free; names, art, icons and text are not. [house rule "Look around before you dive in"]

## Sources

Ours:
- House rules: "Nothing is hidden", "Disabled things say why", "Measure the input", "egui stack", "Everything recovers by itself", "every tunable is a setting", "Skill tree", as of 3 Nov 2026.
- vhrd_egui [`AGENTS.md`](../AGENTS.md) "Code conventions", "Tests"; [`egui_app_skill/references/ui-guide.md`](../egui_app_skill/references/ui-guide.md); the vhrd_egui design notes (a UI ideas page, imported 8 Oct 2026).
- another in-house app `AGENTS.md` "UI conventions"; egui_tabular `AGENTS.md`; the mesh dashboard app `AGENTS.md`; a third in-house app `AGENTS.md`; our policy-engine app `AGENTS.md` and its policy model; vhrd_brand `AGENTS.md`.
- The UX checklist of our UX review role.
- Tasks: (keymap, keyboard) (modal layer) (ve_settings) (bot buttons) (shift-hover).
- The UX sweep of CAD tools (undo, command palette, hotkeys, context help, keyboard reach) and its UX review.

Outside (read 3 Nov 2026):
- Nielsen Norman Group, "10 Usability Heuristics for User Interface Design", https://www.nngroup.com/articles/ten-usability-heuristics/ (numbered 1-10 above).
- GNOME HIG, Tooltips, https://developer.gnome.org/hig/patterns/feedback/tooltips.html; Keyboard, https://developer.gnome.org/hig/guidelines/keyboard.html.
- Apple HIG, Buttons, Undo and redo, Feedback, Accessibility, https://developer.apple.com/design/human-interface-guidelines/.

## Where the rules point here

- [`AGENTS.md`](../AGENTS.md) "UI guide rules" points here for the rules and to `ui-guide.md` for the egui code.
- [`egui_app_skill/references/ui-guide.md`](../egui_app_skill/references/ui-guide.md), the reference the egui skill reads before writing UI, has the egui code for each rule and links back here for the why.
- The [CLI UX guide](https://github.com/vhrdtech/vhrd_cli/blob/main/docs/ux-guide.md), the command-line twin of this page, links to [section 2](#2-rules-for-every-ui) for the rules every UI shares.
