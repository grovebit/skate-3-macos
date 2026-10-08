//! Measurements behind docs/physics/solver-iterations.md: the stock skater with
//! Simulation+0xB0 at the setup value (25) and at the live value
//! (SIMULATION_ITERATIONS). They need the private stock assets and only print
//! the numbers the record quotes.
use super::*;
use bevy::log::tracing::{self, Subscriber, span};
use bevy::log::tracing_subscriber::{
    self, Layer, layer::Context, prelude::*, registry::LookupSpan,
};
use skate_core::{input::xbox::XboxState, player::state::PhysicalStateId};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

type StockGraphs = crate::animation::graph_runtime::StockGraphs;

/// Setup's hard-coded value (`li r27,0x19` at 0x8273B1C4) and the writer's
/// mode != 3 value, against the value the port solves with.
const COUNTS: [u32; 2] = [25, settings::SIMULATION_ITERATIONS];
const RELEASED: XboxState = XboxState {
    buttons: 0,
    triggers: [0; 2],
    left: [0; 2],
    right: [0; 2],
};

struct Run {
    physics: GamePhysics,
    skater: SkaterRuntime,
    controls: PlayerControls,
    camera: crate::camera::CameraRuntime,
    input: crate::input::ControllerInput,
}

impl Run {
    fn new(root: &std::path::Path, graphs: &StockGraphs, iterations: u32) -> Self {
        let mut physics = GamePhysics::load(root).unwrap();
        physics.settings.step.iterations = iterations;
        let skater = SkaterRuntime::load(root, graphs, &physics, "normal").unwrap();
        Self {
            physics,
            skater,
            controls: PlayerControls::load(root).unwrap(),
            camera: crate::camera::CameraRuntime::load(root).unwrap(),
            input: crate::input::ControllerInput::default(),
        }
    }

    fn tick(&mut self, graphs: &StockGraphs, pad: XboxState) {
        self.input.sample_raw_for_test(pad);
        let mut actions = self.input.player_actions();
        self.controls.update(
            &mut actions,
            self.physics.settings.step.simulation.time_step,
            self.physics.settings.input_magnitude_threshold,
            self.skater.player_input.physical.scoring.capabilities_204,
        );
        self.controls.publish_gestures(
            self.physics.animation_profile.physics_mode,
            self.skater.player_input.physical.state.state_16,
        );
        frame::advance(
            &mut self.physics,
            &mut self.skater,
            &mut self.controls,
            graphs,
            &mut actions,
            true,
            &mut self.camera,
        )
        .unwrap();
    }

    /// The spawn drops the board; every measurement starts from it at rest.
    fn settle(&mut self, graphs: &StockGraphs) {
        for _ in 0..120 {
            self.tick(graphs, RELEASED);
        }
        assert_eq!(
            self.skater.player_state.current(),
            PhysicalStateId::PhysicsGround
        );
        assert_eq!(self.physics.riding.ground.wheel_contact_count, 4);
    }
}

fn setup() -> (std::path::PathBuf, StockGraphs) {
    let root = std::path::PathBuf::from(
        std::env::var_os("SKATE3_ASSET_ROOT").expect("set SKATE3_ASSET_ROOT"),
    );
    let manifest = skate_data::GameAssets::load(&root).unwrap();
    let graphs = StockGraphs::load(&root, &manifest).unwrap();
    (root, graphs)
}

/// TU3 (upstream doc 12): wheels at rest -0.012 m/s with 25, -0.001 m/s retail.
#[test]
#[ignore = "measurement; requires private stock assets"]
fn settled_wheel_velocity() {
    let (root, graphs) = setup();
    for iterations in COUNTS {
        let mut run = Run::new(&root, &graphs, iterations);
        run.settle(&graphs);
        let mut wheel_vy = Vec::new();
        let mut wheel_y = Vec::new();
        for _ in 0..480 {
            run.tick(&graphs, RELEASED);
            let wheels = &run.physics.board.bodies()[..4];
            wheel_vy.push(
                wheels
                    .iter()
                    .map(|b| b.rates.linear_velocity.y)
                    .sum::<f32>()
                    / 4.0,
            );
            wheel_y.push(wheels.iter().map(|b| b.rates.position.y).sum::<f32>() / 4.0);
        }
        assert_eq!(run.physics.riding.ground.wheel_contact_count, 4);
        let (lo, hi) = wheel_vy
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        eprintln!(
            "SINK iterations={iterations} wheel_vy_mean={:.6} range=[{lo:.6},{hi:.6}] \
             wheel_y={:.6} drift_over_8s={:.6} deck_y={:.5}",
            wheel_vy.iter().sum::<f32>() / wheel_vy.len() as f32,
            wheel_y[0],
            wheel_y[wheel_y.len() - 1] - wheel_y[0],
            run.physics.board.bodies()[6].rates.position.y,
        );
    }
}

/// TU3 (upstream doc 12): with 25 the pop's deck is about 1 cm low by frame 4.
#[test]
#[ignore = "measurement; requires private stock assets"]
fn standing_ollie_deck_height() {
    let (root, graphs) = setup();
    for iterations in COUNTS {
        let mut run = Run::new(&root, &graphs, iterations);
        run.settle(&graphs);
        let rest = run.physics.board.bodies()[6].rates.position.y;
        let mut rows = Vec::new();
        // Right stick down for half a second, then flicked up.
        for tick in 0..120 {
            let right = match tick {
                0..30 => [0, -32767],
                30..34 => [0, 32767],
                _ => [0; 2],
            };
            run.tick(&graphs, XboxState { right, ..RELEASED });
            let deck = run.physics.board.bodies()[6].rates;
            rows.push((
                run.skater.player_state.current(),
                deck.position.y - rest,
                deck.linear_velocity.y,
            ));
        }
        let pop = rows
            .iter()
            .position(|r| r.0 != PhysicalStateId::PhysicsGround)
            .expect("ollie never left Ground");
        let apex = rows[pop..].iter().map(|r| r.1).fold(f32::MIN, f32::max);
        eprintln!(
            "OLLIE iterations={iterations} rest_deck_y={rest:.5} pop_tick={pop} apex={apex:.5}"
        );
        for (frame, r) in rows[pop..pop + 8].iter().enumerate() {
            eprintln!(
                "OLLIE iterations={iterations} frame={} state={:?} deck_above_rest={:.5} deck_vy={:.4}",
                frame + 1,
                r.0,
                r.1,
                r.2
            );
        }
    }
}

/// Accumulates wall time per span name; frame.rs names its phases.
#[derive(Clone, Default)]
struct SpanClock(Arc<Mutex<BTreeMap<&'static str, Duration>>>);

thread_local! {
    static ENTERED: RefCell<Vec<(span::Id, Instant)>> = const { RefCell::new(Vec::new()) };
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SpanClock {
    fn on_enter(&self, id: &span::Id, _ctx: Context<'_, S>) {
        ENTERED.with(|e| e.borrow_mut().push((id.clone(), Instant::now())));
    }
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let start = ENTERED.with(|e| {
            let mut e = e.borrow_mut();
            e.iter().rposition(|(i, _)| i == id).map(|p| e.remove(p).1)
        });
        if let (Some(start), Some(span)) = (start, ctx.span(id)) {
            *self.0.lock().unwrap().entry(span.name()).or_default() += start.elapsed();
        }
    }
}

#[derive(Default)]
struct Window {
    ms_per_tick: Vec<f64>,
    spans: BTreeMap<&'static str, Duration>,
    ticks: u64,
    wipeout_ticks: u64,
    max_contacts: usize,
}

impl Window {
    fn time(&mut self, run: &mut Run, graphs: &StockGraphs, pads: impl Iterator<Item = XboxState>) {
        let clock = SpanClock::default();
        let subscriber = tracing_subscriber::registry().with(clock.clone());
        let (elapsed, ticks) = tracing::subscriber::with_default(subscriber, || {
            let mut ticks = 0;
            let start = Instant::now();
            for pad in pads {
                run.tick(graphs, pad);
                self.max_contacts = self.max_contacts.max(run.physics.contact_count);
                if run.skater.player_state.current() == PhysicalStateId::WipeoutGround {
                    self.wipeout_ticks += 1;
                }
                ticks += 1;
            }
            (start.elapsed(), ticks)
        });
        self.ms_per_tick
            .push(elapsed.as_secs_f64() * 1e3 / ticks as f64);
        self.ticks += ticks;
        for (name, time) in clock.0.lock().unwrap().iter() {
            *self.spans.entry(name).or_default() += *time;
        }
    }

    fn report(&self, iterations: u32, name: &str) {
        let mut sorted = self.ms_per_tick.clone();
        sorted.sort_by(f64::total_cmp);
        eprintln!(
            "COST iterations={iterations} window={name} tick_ms_median={:.4} max_contacts={} \
             wipeout_ticks={}/{} rounds={:.4?}",
            sorted[sorted.len() / 2],
            self.max_contacts,
            self.wipeout_ticks,
            self.ticks,
            self.ms_per_tick
        );
        for (span, time) in &self.spans {
            eprintln!(
                "COST iterations={iterations} window={name} span={span} ms_per_tick={:.4}",
                time.as_secs_f64() * 1e3 / self.ticks as f64
            );
        }
    }
}

/// CPU cost of one 60 Hz tick: four seconds at rest on the board, then three
/// seconds from the raw bail chord (wipeout_playback.rs) into the ragdoll.
#[test]
#[ignore = "measurement; requires private stock assets"]
fn tick_cost() {
    let (root, graphs) = setup();
    let mut idle: [Window; 2] = Default::default();
    let mut bail: [Window; 2] = Default::default();
    for round in 0..8 {
        // Alternate the order each round so drift does not favour one count.
        let order = if round % 2 == 0 { [0, 1] } else { [1, 0] };
        for index in order {
            let mut run = Run::new(&root, &graphs, COUNTS[index]);
            run.settle(&graphs);
            idle[index].time(&mut run, &graphs, std::iter::repeat_n(RELEASED, 240));
            let chord = XboxState {
                buttons: 0x00C0,
                triggers: [255; 2],
                ..RELEASED
            };
            let pads = std::iter::once(chord).chain(std::iter::repeat_n(RELEASED, 179));
            bail[index].time(&mut run, &graphs, pads);
        }
    }
    assert!(idle.iter().all(|w| w.wipeout_ticks == 0));
    assert!(
        bail.iter().all(|w| w.wipeout_ticks > 0),
        "the bail chord never reached WipeoutGround"
    );
    for (index, iterations) in COUNTS.into_iter().enumerate() {
        idle[index].report(iterations, "idle");
        bail[index].report(iterations, "bail");
    }
}
