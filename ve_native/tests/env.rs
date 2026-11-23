//! `native_options` reads the real process environment: this test runs itself again as a child with
//! `WAYLAND_DISPLAY` set and unset and checks what the child's options say.

use std::process::Command;

/// Set in the child: the session it must see, `wayland` or `other`.
const EXPECT: &str = "VE_NATIVE_EXPECT";

/// The child's half: does nothing unless the parent started it.
#[test]
fn child_checks_its_options() {
    let Some(expect) = std::env::var_os(EXPECT) else {
        return;
    };
    let wayland = expect == "wayland";
    let session = ve_native::Session::detect();
    assert_eq!(
        session,
        if wayland {
            ve_native::Session::Wayland
        } else {
            ve_native::Session::Other
        }
    );
    let options = ve_native::native_options();
    assert_eq!(options.glow_options.vsync, !wayland, "glow vsync");
    let mode = if wayland {
        eframe::wgpu::PresentMode::AutoNoVsync
    } else {
        eframe::wgpu::PresentMode::AutoVsync
    };
    assert_eq!(
        options.wgpu_options.surface.present_mode, mode,
        "wgpu present mode"
    );
}

fn run_child(wayland: bool) {
    let mut cmd = Command::new(std::env::current_exe().expect("test binary path"));
    cmd.args(["--exact", "child_checks_its_options", "--nocapture"])
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env("DISPLAY", ":0");
    if wayland {
        cmd.env("WAYLAND_DISPLAY", "wayland-1")
            .env(EXPECT, "wayland");
    } else {
        cmd.env(EXPECT, "other");
    }
    let out = cmd.output().expect("run the child test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "child failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("1 passed"), "child did not run: {stdout}");
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
#[test]
fn wayland_display_set_turns_vsync_off() {
    run_child(true);
}

#[test]
fn wayland_display_unset_keeps_vsync() {
    run_child(false);
}
