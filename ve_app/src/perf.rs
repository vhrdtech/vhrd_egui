//! The shell's frame recording (WID-11): frame begin and end, repaint causes, the display's refresh, the
//! readout's held numbers.

use std::time::Duration;

use ve_basics::perf::{
    Cause, FrameBegin, FrameEnd, Readout, ReadoutSettings, Recorder, RingRecorder, Settings,
};

/// Repaint causes taken from one frame; further ones are left out.
pub const CAUSES_PER_FRAME_MAX: usize = 16;

/// How often the display's refresh rate is read again, in nanoseconds (the window may move to another monitor).
#[cfg(not(target_arch = "wasm32"))]
const REFRESH_RECHECK_NS: u64 = 2_000_000_000;

/// A frame with no repaint request behind it and input in it.
const INPUT: Cause<'static> = Cause::named("input");
/// A frame with neither: the window was resized, exposed or woken by the system.
const SYSTEM: Cause<'static> = Cause::named("window or system");

pub(crate) struct Perf {
    pub(crate) recorder: Recorder,
    pub(crate) readout: Readout,
    /// The host's CPU time of the previous frame, to hand on after the next begin.
    host_cpu: Option<Duration>,
    #[cfg(not(target_arch = "wasm32"))]
    refresh_read_ns: Option<u64>,
}

impl Perf {
    pub(crate) fn new(recorder: Option<Recorder>, readout: ReadoutSettings) -> Self {
        Self {
            recorder: recorder
                .unwrap_or_else(|| Recorder::new(RingRecorder::new(Settings::default()))),
            readout: Readout::new(readout),
            host_cpu: None,
            #[cfg(not(target_arch = "wasm32"))]
            refresh_read_ns: None,
        }
    }

    /// What eframe knows: the previous frame's whole CPU time and, on native, the monitor's refresh.
    pub(crate) fn host(&mut self, frame: &eframe::Frame) {
        if !self.recorder.is_enabled() {
            return;
        }
        self.host_cpu = frame.info().cpu_usage.map(Duration::from_secs_f32);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let now = self.recorder.now_ns();
            let due = self
                .refresh_read_ns
                .is_none_or(|t| now.saturating_sub(t) >= REFRESH_RECHECK_NS);
            if due && let Some(window) = frame.winit_window() {
                self.refresh_read_ns = Some(now);
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a refresh rate in millihertz fits an f32"
                )]
                let hz = window
                    .current_monitor()
                    .and_then(|m| m.refresh_rate_millihertz())
                    .map(|mhz| mhz as f32 / 1000.0);
                self.recorder.set_display_hz(hz);
            }
        }
    }

    pub(crate) fn begin(&mut self, ctx: &egui::Context) {
        if !self.recorder.is_enabled() {
            return;
        }
        self.recorder.frame_begin(FrameBegin {
            continuous: ctx.requested_repaint_last_pass(),
        });
        if let Some(cpu) = self.host_cpu.take() {
            self.recorder.previous_frame_cpu(cpu);
        }
    }

    pub(crate) fn end(&mut self, ctx: &egui::Context) {
        if !self.recorder.is_enabled() {
            return;
        }
        let asked = ctx.repaint_causes();
        let mut causes = [SYSTEM; CAUSES_PER_FRAME_MAX];
        let mut n = 0;
        for (slot, c) in causes.iter_mut().zip(&asked) {
            *slot = Cause {
                file: c.file,
                line: c.line,
                reason: &c.reason,
            };
            n += 1;
        }
        if n == 0 {
            let input = ctx.input(|i| !i.events.is_empty() || i.pointer.is_moving());
            causes[0] = if input { INPUT } else { SYSTEM };
            n = 1;
        }
        self.recorder.frame_end(&FrameEnd {
            causes: &causes[..n],
        });
        self.readout.update(&*self.recorder);
    }
}
