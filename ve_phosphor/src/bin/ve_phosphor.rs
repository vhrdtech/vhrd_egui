//! The HDR digital-phosphor demo: a window (HDR on macOS EDR, SDR elsewhere), or PNGs rendered headless.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory as _, Parser};
use egui_wgpu::wgpu;
use ve_phosphor::demo::{Demo, SurfaceInfo};
use ve_phosphor::{Phosphor, Scene, Settings, headless, host};

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_SHA"),
    ", commit ",
    env!("GIT_TIME"),
    ")"
);

/// GPU digital-phosphor scope and spectrum persistence, made to show what an HDR screen adds.
///
/// On macOS with an XDR or other EDR display the window runs on an Rgba16Float extended-sRGB surface: the
/// traces glow up to the display's headroom while the UI stays at SDR white. Elsewhere it runs SDR and says so.
///
/// Examples:
///   ve_phosphor                       open the window
///   ve_phosphor --sdr                 the same in SDR, to compare
///   ve_phosphor --headless /tmp/phos  render the scenes to PNG files, no window
///   ve_phosphor --bench               waveforms per second this GPU sustains, no window
#[derive(Parser, Debug)]
#[command(name = "ve_phosphor", version = VERSION, verbatim_doc_comment)]
struct Cli {
    /// Render every scene to PNGs in DIR without a window (three per scene: sdr-screen, hdr-clipped,
    /// hdr-scaled).
    #[arg(long, value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    headless: Option<PathBuf>,
    /// Measure the waveforms per second the GPU sustains (headless) and exit.
    #[arg(long)]
    bench: bool,
    /// Use an 8-bit SDR surface even where a float HDR one is offered.
    #[arg(long)]
    sdr: bool,
    /// Display headroom × SDR white, instead of what the display reports (the headless HDR pictures' peak too).
    #[arg(long, value_name = "X")]
    headroom: Option<f32>,
    /// Waveforms per second to start with (headless default 1.5M, so the 1-in-a-million spike shows).
    #[arg(long, value_name = "N")]
    rate: Option<f32>,
    /// Seconds each scene runs before its headless picture.
    #[arg(long, value_name = "S", default_value_t = 4.0)]
    seconds: f32,
}

struct App(Demo);

impl host::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, dt: f32, info: &SurfaceInfo) {
        self.0.ui(ui, dt, info);
    }
}

fn main() -> ExitCode {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ve_phosphor: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let mut opts = headless::Options {
        seconds: cli.seconds,
        ..Default::default()
    };
    if let Some(h) = cli.headroom {
        opts.headroom = h.max(1.0);
    }
    if let Some(r) = cli.rate {
        opts.sweeps_per_s = r;
    }
    if cli.bench {
        for scene in [Scene::Clock, Scene::Spectrum, Scene::Eye] {
            let (gpu, rate) = headless::bench(scene, 60, &opts)?;
            println!(
                "{gpu}: {}: {:.0}k waveforms/s",
                scene.label(),
                rate / 1000.0
            );
        }
        return Ok(());
    }
    if let Some(dir) = cli.headless {
        let files = headless::render_all(&dir, &opts, &mut |l| eprintln!("{l}"))?;
        for f in files {
            println!("{}", f.display());
        }
        return Ok(());
    }
    let settings = Settings {
        sweeps_per_s: cli.rate.unwrap_or(Settings::default().sweeps_per_s),
        ..Settings::default()
    };
    host::run(
        host::Options {
            title: format!("ve_phosphor {}", env!("CARGO_PKG_VERSION")),
            size: [1440.0, 900.0],
            force_sdr: cli.sdr,
            headroom: cli.headroom,
        },
        move |device: &wgpu::Device, format, ctx: &egui::Context| {
            ve_theme::setup(ctx);
            ctx.set_theme(egui::Theme::Dark);
            App(Demo::new(Phosphor::new(device, format), settings))
        },
    )
}
