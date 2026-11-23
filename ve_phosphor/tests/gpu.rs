//! The phosphor on a real GPU, headless: HDR light goes past SDR white only where it should. Skipped, with a
//! note, where there is no GPU adapter.

use ve_phosphor::headless::{Renderer, hdr_info, sdr_info};
use ve_phosphor::{Mode, Settings};

fn renderer(settings: Settings) -> Option<Renderer> {
    match Renderer::new([640, 400], 1.0, settings) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("SKIPPED: {e}");
            None
        }
    }
}

fn max_channel(img: &ve_phosphor::headless::Image) -> f32 {
    img.rgba
        .iter()
        .flat_map(|p| p[..3].iter().copied())
        .fold(0.0, f32::max)
}

#[test]
fn hdr_goes_past_white_and_sdr_stays_below_it() {
    let settings = Settings {
        mode: Mode::Split,
        ..Settings::default()
    };
    let Some(mut r) = renderer(settings) else {
        return;
    };
    let hdr = hdr_info(8.0);
    for _ in 0..30 {
        r.frame(1.0 / 60.0, &hdr).expect("renders");
    }
    r.frame(0.0, &hdr).expect("renders");
    let img = r.read().expect("reads back");
    let peak = max_channel(&img);
    // the densest trace sits at the peak: 8x linear is about 2.4 encoded
    assert!(peak > 2.0, "HDR peak {peak}");

    r.frame(0.0, &sdr_info()).expect("renders");
    let sdr = max_channel(&r.read().expect("reads back"));
    assert!(sdr <= 1.0 + 1e-3, "SDR output max {sdr}");

    // the SDR tone map alone stays at or below white even on an HDR output
    r.demo().settings.mode = Mode::Sdr;
    for _ in 0..30 {
        r.frame(1.0 / 60.0, &hdr).expect("renders");
    }
    let only_sdr = max_channel(&r.read().expect("reads back"));
    assert!(only_sdr <= 1.0 + 1e-3, "SDR tone map max {only_sdr}");
    assert!(only_sdr > 0.5, "the SDR trace is drawn: {only_sdr}");
}
