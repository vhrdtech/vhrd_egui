//! The shell's frame recording (WID-11): frame begin and end, repaint causes, the display's refresh, the
//! readout's held numbers and the one repaint that lets it say "idle".

use std::time::Duration;

use ve_basics::perf::{Cause, FrameBegin, FrameEnd, Readout, Recorder, RingRecorder, Settings};

/// Repaint causes taken from one frame; further ones are left out.
pub const CAUSES_PER_FRAME_MAX: usize = 16;

/// The scope the `logic` of all widgets runs in: one row for all of them, so it names no widget. One scope per
/// widget would double the shell's recording cost per frame and halve how many widgets fit [`NAMES_MAX`].
///
/// [`NAMES_MAX`]: ve_basics::perf::NAMES_MAX
pub const LOGIC_SCOPE: &str = "logic";

/// How often the display's refresh rate is read again, in nanoseconds (the window may move to another monitor).
#[cfg(not(target_arch = "wasm32"))]
const REFRESH_RECHECK_NS: u64 = 2_000_000_000;

/// How long after the recorder's idle line ([`Settings::idle_after`]) the shell wakes once to show the readout as idle.
const IDLE_MARGIN: Duration = Duration::from_millis(100);

/// A frame with no repaint request behind it and input in it.
const INPUT: Cause<'static> = Cause::named("input");
/// A frame with neither: the window was resized, exposed or woken by the system.
const SYSTEM: Cause<'static> = Cause::named("window or system");

pub(crate) struct Perf {
    pub(crate) recorder: Recorder,
    pub(crate) readout: Readout,
    /// The host's CPU time of the previous frame, to hand on after the next begin.
    host_cpu: Option<Duration>,
    /// What asked for this frame, taken at its first pass.
    causes: Vec<egui::RepaintCause>,
    /// This frame is recorded: it began in the first pass and ends with the last.
    recording: bool,
    /// This frame had input.
    input: bool,
    /// When the last recorded frame ended, on the recorder's clock.
    last_end_ns: Option<u64>,
    #[cfg(not(target_arch = "wasm32"))]
    refresh_read_ns: Option<u64>,
}

impl Perf {
    pub(crate) fn new(recorder: Option<Recorder>, readout: Readout) -> Self {
        let recorder = recorder.unwrap_or_else(|| {
            Recorder::new(
                RingRecorder::new(Settings::default()).expect("the default settings are valid"),
            )
        });
        Self {
            recorder,
            readout,
            host_cpu: None,
            causes: Vec::new(),
            recording: false,
            input: false,
            last_end_ns: None,
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

    /// This frame is recorded: scopes go into it.
    pub(crate) fn recording(&self) -> bool {
        self.recording
    }

    /// At the start of the shell's ui. egui may run several passes for one painted frame: only the first begins it.
    pub(crate) fn begin(&mut self, ctx: &egui::Context) {
        if !self.recorder.is_enabled() || ctx.current_pass_index() > 0 {
            return;
        }
        // Before the frame begins, so a frame after a pause first shows how long it was idle.
        self.readout.update(&*self.recorder);
        self.causes = ctx.repaint_causes();
        self.input = ctx.input(|i| !i.events.is_empty() || i.pointer.is_moving());
        // The shell's own wake-up to show "idle" is not the app's frame: nothing but its request, after a quiet
        // second. egui lists every request of the last pass, delayed ones too, so the time decides.
        let now = self.recorder.now_ns();
        let idle_after = self.recorder.settings().idle_after;
        let quiet = self
            .last_end_ns
            .is_none_or(|t| Duration::from_nanos(now.saturating_sub(t)) >= idle_after);
        let own_wakeup = quiet
            && !self.input
            && !self.causes.is_empty()
            && self.causes.iter().all(|c| c.file == file!());
        self.recording = !own_wakeup;
        if !self.recording {
            return;
        }
        self.recorder.frame_begin(FrameBegin {
            continuous: ctx.requested_repaint_last_pass(),
        });
        if let Some(cpu) = self.host_cpu.take() {
            self.recorder.previous_frame_cpu(cpu);
        }
    }

    /// At the end of the shell's ui; a pass egui will discard is not the end of the frame.
    pub(crate) fn end(&mut self, ctx: &egui::Context) {
        if !self.recording || ctx.will_discard() {
            return;
        }
        self.recording = false;
        let mut causes = [SYSTEM; CAUSES_PER_FRAME_MAX];
        let mut n = 0;
        if self.input {
            causes[0] = INPUT;
            n = 1;
        }
        // The shell's own idle request is never why a frame was painted: its wake-up is not recorded.
        let asked = self.causes.iter().filter(|c| c.file != file!());
        for (slot, c) in causes[n..].iter_mut().zip(asked) {
            *slot = Cause {
                file: c.file,
                line: c.line,
                reason: &c.reason,
            };
            n += 1;
        }
        self.recorder.frame_end(&FrameEnd {
            causes: &causes[..n.max(1)],
        });
        self.last_end_ns = Some(self.recorder.now_ns());
        // One wake-up once the app goes quiet, so the readout can say so.
        ctx.request_repaint_after(self.recorder.settings().idle_after + IDLE_MARGIN);
    }
}

#[cfg(test)]
mod tests {
    use ve_alloc_count::{CountingAlloc, totals};

    use super::*;

    #[global_allocator]
    static ALLOC: CountingAlloc = CountingAlloc;

    fn allocs() -> u64 {
        totals().map_or(0, |(n, _)| n)
    }

    /// WID-11, review finding 18: what the shell's own recording allocates per frame once the recorder's name and
    /// cause tables hold their entries: one vector, egui's `repaint_causes` copy (the shell's own idle request
    /// is in it every frame).
    #[test]
    fn recording_a_frame_allocates_only_egui_s_cause_list() {
        let ctx = egui::Context::default();
        let mut perf = Perf::new(None, Readout::default());
        let mut measured = Vec::new();
        for i in 0..12 {
            let mut n = 0;
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let before = allocs();
                perf.begin(ui.ctx());
                perf.end(ui.ctx());
                n = allocs() - before;
                if i % 3 == 0 {
                    ui.ctx().request_repaint();
                }
            });
            out.textures_delta.clear();
            measured.push(n);
        }
        let steady = &measured[4..];
        assert!(steady.iter().all(|&n| n == 1), "{measured:?}");
    }
}
