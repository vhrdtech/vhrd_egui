# Upgrading an app to a newer template version

Applies to apps with a `ve_app.json` (generated, or adopted with `"adopted": true`).

1. **What changed**: in vhrd_egui, `git log <template.commit>..HEAD -- egui_app_skill ve_template` and the
   `CHANGELOG.md` sections newer than `template.version`; template entries are marked `(PLT-2)` and carry
   **Upgrade notes**. Stop here if nothing template-related changed (update `ve_app.json`'s commit only when asked).
2. **New options**: `ve_template check-answers <app>/ve_app.json` lists options added since generation with their
   defaults. Ask the user about each, write the answer into `answers`.
3. **Regenerate**: `ve_template new --answers <app>/ve_app.json --out <tmp>` with the current template, and the
   recorded commit into another folder as the 3-way base:
   `git -C ~/git/vhrd_egui worktree add <tmp-old> <template.commit>` and
   `cargo run -q --manifest-path <tmp-old>/Cargo.toml -p ve_template -- new --answers <app>/ve_app.json --out <tmp-old-gen>`
   (an older template without `ve_template` has no base: compare two-way and read every diff).
4. **Compare**: `ve_template compare --current <app> --new <tmp>/<name> --base <tmp-old-gen>/<name> --diff`.
   Per file: `same`, `update` (the app never changed it: take the new file), `add` (new template file), `local`
   (only the app changed it: keep), `merge` (both changed: read the diff, apply the template's change by meaning),
   `dropped` (the template no longer ships it: decide), `app-only`, `noise` (Cargo.lock, ve_app.json).
5. **Report** per logical upgrade (not per file): what the template changed, why (the CHANGELOG note), what it
   means for this app, skipping items listed in `rejected`, respecting `nuances`. Apply **only after approval**.
6. **Apply**, then `just lint && just test`; refresh snapshots when the look changed on purpose and look at them.
7. **Record** in `ve_app.json`: `template.version` / `commit` / `dirty`, an `upgrades` entry
   (`{"from": "0.8.0", "to": "0.9.0", "date": "2026-10-20", "notes": "..."}`), the new `rejected` items, new
   `nuances`. Commit `ve_app.json` with the upgrade, CHANGELOG entry `### Changed: template 0.9.0 (...)`.
8. Remove the temp worktree: `git -C ~/git/vhrd_egui worktree remove <tmp-old>`.
