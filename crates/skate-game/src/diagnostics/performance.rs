//! Frame timing harness. Active only when `SKATE_PERF_REPORT` names an output
//! path: enters fullscreen with the saved frame limit off, warms up, samples a
//! fixed window while the camera flies once around the map (`flythrough`),
//! writes JSON, then exits.
//!
//! The `Performance` resource is absent in normal play, which is why callers
//! take it as `Option<ResMut<_>>` — instrumentation must never cost anything in
//! a shipping session.
use bevy::{
    prelude::*,
    window::{MonitorSelection, PrimaryWindow, WindowMode},
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Seconds discarded before sampling, so shader compilation, asset streaming and
/// the map publish settle do not pollute the window.
const WARMUP: f32 = 10.0;
/// Seconds of samples retained, and the flythrough's duration.
const SAMPLE: f32 = 60.0;
/// A hitch is a frame longer than this many medians of the frames before it,
/// upstream's `frame_timing` rule. A fixed threshold cannot work under vsync:
/// every 120 Hz FIFO frame takes about 8.33 ms. A frame that misses one vblank
/// lands at about twice the median and counts only when it measures over it;
/// longer stalls always count.
const HITCH_FACTOR: f32 = 2.0;
/// Frames in the hitch median: about a second at the frame rates the harness
/// measures. Warmup fills it, so every sample is classified.
const HITCH_HISTORY: usize = 120;

/// CPU cost of the render schedule, split at phase boundaries.
///
/// Bevy pipelines the render sub-app against the next frame's main schedule, so
/// when `frame_ms` exceeds `main_schedule_ms` the render app is the critical
/// path and this is the only way to see which phase owns it. The main world and
/// the render world hold clones of the same `Arc`, which is why the counters are
/// atomics rather than a plain resource: they are written in the render world
/// and read in the main world with no ordering guarantee between the two.
#[derive(Resource, Clone, Default)]
pub(crate) struct RenderPhases(Arc<Phases>);

#[derive(Default)]
struct Phases {
    started: AtomicU64,
    extract_ns: AtomicU64,
    assets_ns: AtomicU64,
    views_ns: AtomicU64,
    queue_ns: AtomicU64,
    prepare_ns: AtomicU64,
    total_ns: AtomicU64,
    frames: AtomicU64,
}

/// Wall-clock nanoseconds since an arbitrary fixed origin.
///
/// `Instant` cannot live in an atomic, and the render world needs to hand a
/// timestamp to systems that run later in the same schedule.
fn now_ns() -> u64 {
    use std::sync::LazyLock;
    static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);
    ORIGIN.elapsed().as_nanos() as u64
}

impl RenderPhases {
    /// Mean milliseconds per frame for each phase, in report order.
    fn means(&self) -> [f32; 6] {
        let frames = self.0.frames.load(Ordering::Relaxed).max(1) as f32;
        let mean = |counter: &AtomicU64| {
            counter.load(Ordering::Relaxed) as f32 / frames / 1.0e6
        };
        [
            mean(&self.0.extract_ns),
            mean(&self.0.assets_ns),
            mean(&self.0.views_ns),
            mean(&self.0.queue_ns),
            mean(&self.0.prepare_ns),
            mean(&self.0.total_ns),
        ]
    }

    fn reset(&self) {
        for counter in [
            &self.0.extract_ns,
            &self.0.assets_ns,
            &self.0.views_ns,
            &self.0.queue_ns,
            &self.0.prepare_ns,
            &self.0.total_ns,
            &self.0.frames,
        ] {
            counter.store(0, Ordering::Relaxed);
        }
    }
}

/// Accumulates the span from the recorded phase start to now, then re-arms the
/// start for the next phase.
fn mark(phases: &RenderPhases, counter: &AtomicU64) {
    let now = now_ns();
    let start = phases.0.started.swap(now, Ordering::Relaxed);
    counter.fetch_add(now.saturating_sub(start), Ordering::Relaxed);
}

#[derive(Default, Clone, Copy)]
struct Frame {
    total_ms: f32,
    main_ms: f32,
    physics_ms: f32,
    hitch: bool,
}

/// The last `HITCH_HISTORY` frame times, recorded from the first frame.
#[derive(Default)]
struct HitchHistory {
    frames: VecDeque<f32>,
    scratch: Vec<f32>,
}

impl HitchHistory {
    /// Whether `frame_ms` exceeds `HITCH_FACTOR` times the nearest-rank median
    /// of the frames before it, then records it. False until the history fills.
    fn observe(&mut self, frame_ms: f32) -> bool {
        let full = self.frames.len() == HITCH_HISTORY;
        let hitch = full && {
            self.scratch.clear();
            self.scratch.extend(&self.frames);
            // Lower middle for an even count, matching `percentile(0.50)`.
            let middle = self.scratch.len().div_ceil(2) - 1;
            let (_, median, _) = self.scratch.select_nth_unstable_by(middle, f32::total_cmp);
            frame_ms > HITCH_FACTOR * *median
        };
        if full {
            self.frames.pop_front();
        }
        self.frames.push_back(frame_ms);
        hitch
    }
}

#[derive(Resource)]
pub(crate) struct Performance {
    output: PathBuf,
    started: Instant,
    frame_started: Option<Instant>,
    main_elapsed: Duration,
    physics_elapsed: Duration,
    hitches: HitchHistory,
    samples: Vec<Frame>,
    sampling: bool,
    finished: bool,
}

impl Performance {
    fn new(output: PathBuf) -> Self {
        Self {
            output,
            started: Instant::now(),
            frame_started: None,
            main_elapsed: Duration::ZERO,
            physics_elapsed: Duration::ZERO,
            hitches: HitchHistory::default(),
            // An optimistic 600 FPS; growth beyond this is harmless.
            samples: Vec::with_capacity(SAMPLE as usize * 600),
            sampling: false,
            finished: false,
        }
    }

    /// Physics tick cost, reported by `physics::advance`. FixedUpdate may run
    /// zero or several times per frame, so this accumulates until the frame ends.
    pub(crate) fn physics(&mut self, elapsed: Duration) {
        self.physics_elapsed += elapsed;
    }

    /// Fraction of the sample window elapsed: 0 through the warmup, 1 at its end.
    pub(super) fn sample_progress(&self) -> f32 {
        ((self.started.elapsed().as_secs_f32() - WARMUP) / SAMPLE).clamp(0.0, 1.0)
    }
}

pub(crate) struct PerformancePlugin;
impl Plugin for PerformancePlugin {
    fn build(&self, app: &mut App) {
        let Some(output) = std::env::var_os("SKATE_PERF_REPORT").map(PathBuf::from) else {
            return;
        };
        info!("SKATE_PERF starting: warmup={WARMUP}s sample={SAMPLE}s output={output:?}");
        let phases = RenderPhases::default();
        app.add_plugins(super::flythrough::FlythroughPlugin)
            .insert_resource(Performance::new(output))
            .insert_resource(phases.clone())
            .add_systems(Startup, enter_fullscreen)
            .add_systems(First, frame_begin)
            .add_systems(Last, frame_end);
        // GPU timestamp/statistics queries add work of their own. Ordinary CPU
        // frame comparisons must not enable them implicitly.
        if std::env::var("SKATE_PERF_GPU").as_deref() == Ok("1") {
            app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
        }

        // Phase-boundary systems serialize otherwise overlapping render work.
        // Only enable that detailed attribution when explicitly requested.
        if std::env::var("SKATE_PERF_RENDER").as_deref() != Ok("1") {
            return;
        }

        use bevy::render::{Render, RenderApp, RenderSystems};
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render.insert_resource(phases);
        render.add_systems(
            Render,
            (
                begin_render_frame.before(RenderSystems::ExtractCommands),
                after_extract
                    .after(RenderSystems::ExtractCommands)
                    .before(RenderSystems::PrepareAssets),
                after_assets
                    .after(RenderSystems::PrepareMeshes)
                    .after(RenderSystems::PrepareAssets)
                    .before(RenderSystems::ManageViews),
                after_views
                    .after(RenderSystems::ManageViews)
                    .before(RenderSystems::Queue),
                after_queue
                    .after(RenderSystems::QueueSweep)
                    .before(RenderSystems::PhaseSort),
                after_prepare
                    .after(RenderSystems::PrepareBindGroups)
                    .before(RenderSystems::Render),
                end_render_frame.after(RenderSystems::Cleanup),
            ),
        );
    }
}

/// Measures the game as it is played: in the borderless fullscreen Alt+Enter
/// selects (`graphics_menu::toggle_fullscreen`), entered before the window opens
/// so no transition reaches the sample window.
fn enter_fullscreen(mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Primary);
}

fn begin_render_frame(phases: Res<RenderPhases>) {
    phases.0.started.store(now_ns(), Ordering::Relaxed);
}

fn after_extract(phases: Res<RenderPhases>) {
    mark(&phases, &phases.0.extract_ns);
}

fn after_assets(phases: Res<RenderPhases>) {
    mark(&phases, &phases.0.assets_ns);
}

fn after_views(phases: Res<RenderPhases>) {
    mark(&phases, &phases.0.views_ns);
}

fn after_queue(phases: Res<RenderPhases>) {
    mark(&phases, &phases.0.queue_ns);
}

fn after_prepare(phases: Res<RenderPhases>) {
    mark(&phases, &phases.0.prepare_ns);
}

/// Closes the frame: the residual span is the render graph plus cleanup, and the
/// frame counter is what turns every accumulator into a per-frame mean.
fn end_render_frame(phases: Res<RenderPhases>) {
    mark(&phases, &phases.0.total_ns);
    phases.0.frames.fetch_add(1, Ordering::Relaxed);
}

fn frame_begin(mut performance: ResMut<Performance>) {
    performance.frame_started = Some(Instant::now());
    performance.main_elapsed = Duration::ZERO;
    performance.physics_elapsed = Duration::ZERO;
}

fn frame_end(
    mut performance: ResMut<Performance>,
    time: Res<Time<Real>>,
    phases: Res<RenderPhases>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    if performance.finished {
        return;
    }
    // Wall-clock delta rather than the frame_begin instant: it includes the
    // presentation wait, which is what the player actually experiences.
    let total_ms = time.delta().as_secs_f32() * 1000.0;
    let hitch = performance.hitches.observe(total_ms);
    let elapsed = performance.started.elapsed().as_secs_f32();
    if !performance.sampling {
        if elapsed < WARMUP {
            return;
        }
        performance.sampling = true;
        // Warmup includes shader compilation and asset streaming, whose cost
        // would otherwise dominate the phase accumulators.
        phases.reset();
        info!("SKATE_PERF warmup complete, sampling {SAMPLE}s");
    }

    let main_ms = performance
        .frame_started
        .map_or(0.0, |start| start.elapsed().as_secs_f32() * 1000.0);
    let physics_ms = performance.physics_elapsed.as_secs_f32() * 1000.0;
    performance.samples.push(Frame {
        total_ms,
        main_ms,
        physics_ms,
        hitch,
    });

    if elapsed < WARMUP + SAMPLE {
        return;
    }
    performance.finished = true;
    let mut report = summarise(&performance.samples);
    report.render_phase_ms = phases.means();
    if let Ok(window) = window.single() {
        report.window_mode = format!("{:?}", window.mode);
        report.present_mode = format!("{:?}", window.present_mode);
        report.window_physical_size = window.physical_size().to_array();
    }
    // Every render diagnostic, so GPU pass costs land in the report without
    // this module needing to know the pass names the render graph happens to use.
    report.gpu = diagnostics
        .iter()
        .filter(|d| d.path().as_str().starts_with("render/"))
        .filter_map(|d| d.smoothed().map(|v| (d.path().to_string(), v)))
        .filter(|(_, v)| *v > 0.0)
        .collect();
    report.gpu.sort_by(|a, b| b.1.total_cmp(&a.1));
    match write_report(&performance.output, &report) {
        Ok(()) => info!("SKATE_PERF_REPORT written to {:?}", performance.output),
        Err(error) => error!("SKATE_PERF_REPORT could not be written: {error}"),
    }
    let [extract, assets, views, queue, prepare, graph] = report.render_phase_ms;
    eprintln!(
        "SKATE_PERF fps={:.1} frame_ms_mean={:.3} p99={:.3} hitches={} main={:.3} window={}x{} {} {}",
        report.fps,
        report.frame_ms_mean,
        report.frame_ms_p99,
        report.hitch_frames,
        report.main_schedule_ms_mean,
        report.window_physical_size[0],
        report.window_physical_size[1],
        report.window_mode,
        report.present_mode,
    );
    eprintln!(
        "SKATE_PERF_RENDER extract={extract:.3} assets={assets:.3} views={views:.3} \
         queue={queue:.3} prepare={prepare:.3} graph={graph:.3}"
    );
    // Times first and in full: sorting the whole set by value buries
    // sub-millisecond durations under invocation counts in the millions.
    for (name, value) in report.gpu.iter().filter(|(n, _)| n.ends_with("elapsed_gpu")) {
        eprintln!("SKATE_PERF_GPU_MS {name} {value:.4}");
    }
    for (name, value) in report
        .gpu
        .iter()
        .filter(|(n, _)| n.ends_with("invocations"))
        .take(8)
    {
        eprintln!("SKATE_PERF_GPU {name} {value:.0}");
    }
    exit.write(AppExit::Success);
}

struct Report {
    frames: usize,
    fps: f32,
    fps_1_percent_low: f32,
    slowest_1_percent_ms_mean: f32,
    frame_ms_mean: f32,
    frame_ms_median: f32,
    frame_ms_p95: f32,
    frame_ms_p99: f32,
    frame_ms_max: f32,
    hitch_frames: usize,
    main_schedule_ms_mean: f32,
    physics_ms_mean: f32,
    /// extract, assets, views, queue, prepare, graph — see `RenderPhases`.
    render_phase_ms: [f32; 6],
    /// Presentation the samples were taken at: mode and drawable pixels.
    window_mode: String,
    window_physical_size: [u32; 2],
    present_mode: String,
    /// Render diagnostics, highest first. Includes GPU pass timings when the
    /// adapter supports timestamp queries.
    gpu: Vec<(String, f64)>,
    samples: Vec<Frame>,
}

fn summarise(samples: &[Frame]) -> Report {
    let frames = samples.len();
    let mut sorted: Vec<f32> = samples.iter().map(|f| f.total_ms).collect();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mean = |values: &[f32]| -> f32 {
        if values.is_empty() { 0.0 } else { values.iter().sum::<f32>() / values.len() as f32 }
    };
    // Nearest-rank percentile. Exact interpolation is not worth it here: the
    // sample count is in the thousands and the budget is stated in whole ms.
    let percentile = |q: f32| -> f32 {
        if sorted.is_empty() {
            return 0.0;
        }
        // Nearest-rank: the smallest sample at or below which `q` of the frames
        // fall, i.e. rank `ceil(q*N)` counting from one. Reporting "95% of frames
        // were at or under this" only means that under this definition.
        let rank = (q * sorted.len() as f32).ceil() as usize;
        sorted[rank.clamp(1, sorted.len()) - 1]
    };
    let frame_ms_mean = mean(&sorted);
    // A 1% low averages the slowest ceil(N / 100) frame times, then takes
    // their reciprocal. It is distinct from the reciprocal of the p99 cutoff.
    let slowest_1_percent_ms_mean = mean(&sorted[frames - frames.div_ceil(100)..]);
    Report {
        frames,
        fps: if frame_ms_mean > 0.0 { 1000.0 / frame_ms_mean } else { 0.0 },
        fps_1_percent_low: if slowest_1_percent_ms_mean > 0.0 {
            1000.0 / slowest_1_percent_ms_mean
        } else { 0.0 },
        slowest_1_percent_ms_mean,
        frame_ms_mean,
        frame_ms_median: percentile(0.50),
        frame_ms_p95: percentile(0.95),
        frame_ms_p99: percentile(0.99),
        frame_ms_max: sorted.last().copied().unwrap_or(0.0),
        hitch_frames: samples.iter().filter(|f| f.hitch).count(),
        main_schedule_ms_mean: mean(&samples.iter().map(|f| f.main_ms).collect::<Vec<_>>()),
        physics_ms_mean: mean(&samples.iter().map(|f| f.physics_ms).collect::<Vec<_>>()),
        render_phase_ms: [0.0; 6],
        window_mode: String::new(),
        window_physical_size: [0; 2],
        present_mode: String::new(),
        gpu: Vec::new(),
        samples: samples.to_vec(),
    }
}

fn write_report(path: &std::path::Path, report: &Report) -> std::io::Result<()> {
    let json = serde_json::json!({
        "frames": report.frames,
        "gpu_queries_enabled": std::env::var("SKATE_PERF_GPU").as_deref() == Ok("1"),
        "render_phase_instrumentation": std::env::var("SKATE_PERF_RENDER").as_deref() == Ok("1"),
        "fps": report.fps,
        "fps_1_percent_low": report.fps_1_percent_low,
        "slowest_1_percent_ms_mean": report.slowest_1_percent_ms_mean,
        "frame_ms_mean": report.frame_ms_mean,
        "frame_ms_median": report.frame_ms_median,
        "frame_ms_p95": report.frame_ms_p95,
        "frame_ms_p99": report.frame_ms_p99,
        "frame_ms_max": report.frame_ms_max,
        "hitch_frames": report.hitch_frames,
        "hitch_factor": HITCH_FACTOR,
        "hitch_history_frames": HITCH_HISTORY,
        "main_schedule_ms_mean": report.main_schedule_ms_mean,
        "physics_ms_mean": report.physics_ms_mean,
        "render_extract_ms_mean": report.render_phase_ms[0],
        "render_assets_ms_mean": report.render_phase_ms[1],
        "render_views_ms_mean": report.render_phase_ms[2],
        "render_queue_ms_mean": report.render_phase_ms[3],
        "render_prepare_ms_mean": report.render_phase_ms[4],
        "render_graph_ms_mean": report.render_phase_ms[5],
        "window_mode": report.window_mode,
        "window_physical_size": report.window_physical_size,
        "present_mode": report.present_mode,
        "render_diagnostics": report.gpu.iter()
            .map(|(name, value)| serde_json::json!({ "name": name, "ms": value }))
            .collect::<Vec<_>>(),
        "warmup_seconds": WARMUP,
        "sample_seconds": SAMPLE,
        "samples": report.samples.iter().map(|f| serde_json::json!({
            "frame_ms": f.total_ms,
            "main_ms": f.main_ms,
            "physics_ms": f.physics_ms,
            "hitch": f.hitch,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(path, serde_json::to_vec_pretty(&json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(total_ms: f32) -> Frame {
        Frame { total_ms, ..default() }
    }

    #[test]
    fn percentiles_follow_sample_order() {
        // Values 1..=100, so the Nth smallest sample is exactly N ms and every
        // rank is readable directly.
        let samples: Vec<Frame> = (1..=100).map(|ms| frame(ms as f32)).collect();
        let report = summarise(&samples);
        assert_eq!(report.frames, 100);
        assert_eq!(report.frame_ms_median, 50.0);
        assert_eq!(report.frame_ms_p95, 95.0);
        assert_eq!(report.frame_ms_p99, 99.0);
        assert_eq!(report.frame_ms_max, 100.0);
        // A single sample is every percentile of itself.
        let one = summarise(&[frame(7.0)]);
        assert_eq!(
            (one.frame_ms_median, one.frame_ms_p95, one.frame_ms_p99),
            (7.0, 7.0, 7.0)
        );
    }

    #[test]
    fn fps_is_the_reciprocal_of_mean_frame_time() {
        // 300 FPS is the contract; 3.333 ms per frame must read back as ~300.
        let samples = vec![frame(10.0 / 3.0); 64];
        let report = summarise(&samples);
        assert!((report.fps - 300.0).abs() < 1.0, "fps={}", report.fps);
    }

    /// Classifies every frame as `frame_end` does, warmup included, and keeps
    /// the frames after the first `warmup` as samples.
    fn sampled(frames: &[f32], warmup: usize) -> Vec<Frame> {
        let mut history = HitchHistory::default();
        let flags: Vec<bool> = frames.iter().map(|&ms| history.observe(ms)).collect();
        frames
            .iter()
            .zip(flags)
            .skip(warmup)
            .map(|(&total_ms, hitch)| Frame { total_ms, hitch, ..default() })
            .collect()
    }

    #[test]
    fn fifo_jitter_is_not_a_hitch_but_a_stall_is() {
        // 120 Hz vsync intervals jitter around 8.33 ms. The former fixed 8 ms
        // threshold counted every other one of these frames.
        let mut frames: Vec<f32> = (0..3 * HITCH_HISTORY)
            .map(|i| if i % 2 == 0 { 7.9 } else { 8.8 })
            .collect();
        frames[2 * HITCH_HISTORY] = 25.0;
        let report = summarise(&sampled(&frames, HITCH_HISTORY));
        assert_eq!(report.frames, 2 * HITCH_HISTORY);
        assert_eq!(report.hitch_frames, 1);
        assert!(report.samples[HITCH_HISTORY].hitch);
    }

    #[test]
    fn a_hitch_is_strictly_over_twice_the_median_of_a_full_history() {
        let mut history = HitchHistory::default();
        assert!((1..HITCH_HISTORY).all(|_| !history.observe(5.0)));
        assert!(!history.observe(1000.0), "history not yet full");
        // The one long frame now in the history leaves the median at 5 ms.
        assert!(!history.observe(10.0), "exactly twice the median");
        assert!(history.observe(10.01));
    }

    #[test]
    fn the_median_of_an_even_history_is_its_lower_middle() {
        // As `percentile(0.50)`: 60 frames of 5 ms and 60 of 50 ms give 5 ms.
        // The upper middle (50 ms) or the midpoint (27.5 ms) would not flag 10.01.
        let mut history = HitchHistory::default();
        for ms in [5.0, 50.0] {
            for _ in 0..HITCH_HISTORY / 2 {
                history.observe(ms);
            }
        }
        assert!(history.observe(10.01));
    }

    #[test]
    fn a_sustained_slowdown_stops_counting_once_it_is_the_median() {
        let mut frames = vec![1000.0 / 120.0; HITCH_HISTORY];
        frames.extend(vec![20.0; 2 * HITCH_HISTORY]);
        let report = summarise(&sampled(&frames, HITCH_HISTORY));
        // Slow frames count until they fill the lower half of the history.
        assert_eq!(report.hitch_frames, HITCH_HISTORY / 2 + 1);
    }

    #[test]
    fn empty_sample_set_does_not_panic() {
        let report = summarise(&[]);
        assert_eq!(report.frames, 0);
        assert_eq!(report.fps, 0.0);
        assert_eq!(report.fps_1_percent_low, 0.0);
    }

    #[test]
    fn one_percent_low_averages_the_tail_including_fractional_sample_counts() {
        let mut samples = vec![frame(2.0); 199];
        samples.extend([frame(10.0), frame(18.0)]);
        let report = summarise(&samples);
        assert_eq!(report.frame_ms_p99, 2.0);
        assert_eq!(report.slowest_1_percent_ms_mean, 10.0);
        assert_eq!(report.fps_1_percent_low, 100.0);
        let one = summarise(&[frame(8.0)]);
        assert_eq!(one.fps_1_percent_low, 125.0);
    }
}
