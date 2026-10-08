use super::*;
use skate_core::{
    camera::{SlowMotionController, SlowMotionSettings},
    point_graph::PointGraph,
};

#[test]
fn camera_rate_requests_expire_and_preserve_message_order() {
    let mut clock = SimulationClock::default();
    let normal = clock.period();
    let slow = SimulationRateRequest {
        timestep: 1.0 / 35.4,
        ticks: 1,
    };
    clock.apply(slow).unwrap();
    clock.finish_tick();
    assert_eq!(clock.period(), Duration::from_nanos(28_571_400));
    clock.finish_tick();
    assert_eq!(clock.period(), normal);

    //An End followed by a new Begin must retain the new request.
    clock.apply(SlowMotionController::end()).unwrap();
    clock.apply(slow).unwrap();
    clock.finish_tick();
    assert_ne!(clock.period(), normal);
    clock.apply(SlowMotionController::end()).unwrap();
    clock.finish_tick();
    assert_eq!(clock.period(), normal);

    clock
        .apply(SimulationRateRequest { ticks: 0, ..slow })
        .unwrap();
    for _ in 0..179 {
        clock.finish_tick();
    }
    assert_ne!(clock.period(), normal);
    clock.finish_tick();
    assert_eq!(clock.period(), normal);
}

#[test]
fn publication_timestep_retains_precision_until_request_expiry() {
    let mut clock = SimulationClock::default();
    assert_eq!(clock.requested_timestep(), NORMAL_STEP);
    let first = SimulationRateRequest {
        timestep: 1.0 / 35.4,
        ticks: 1,
    };
    let second = SimulationRateRequest {
        timestep: 1.0 / 35.2,
        ..first
    };
    clock.apply(first).unwrap();
    let quantized_period = clock.period();
    clock.apply(second).unwrap();
    // Both requests schedule at 35 Hz, but original publication distinguishes
    // them. Reconstructing the request from Duration would lose this input.
    assert_eq!(clock.period(), quantized_period);
    assert_eq!(clock.requested_timestep(), second.timestep);
    assert_ne!(clock.requested_timestep(), first.timestep);
    assert_ne!(clock.requested_timestep(), clock.period().as_secs_f32());
    assert!(
        clock
            .apply(SimulationRateRequest {
                timestep: f32::NAN,
                ..first
            })
            .is_err()
    );
    assert_eq!(clock.requested_timestep(), second.timestep);
    assert_eq!(clock.period(), quantized_period);
    clock.finish_tick();
    assert_eq!(clock.requested_timestep(), second.timestep);
    clock.finish_tick();
    assert_eq!(clock.requested_timestep(), NORMAL_STEP);
    clock.apply(first).unwrap();
    clock.apply(SlowMotionController::end()).unwrap();
    assert_eq!(clock.requested_timestep(), NORMAL_STEP);
}

#[test]
fn camera_slow_motion_waits_for_a_positive_air_prediction() {
    let settings = SlowMotionSettings {
        timescale: PointGraph {
            x: std::array::from_fn(|i| i as f32 / 15.0),
            y: std::array::from_fn(|i| i as f32 / 15.0),
        },
        fps_at_scale_one: 35.0,
    };
    let (mut controller, _) = SlowMotionController::begin(settings);
    let waiting = controller.update(0.1, f32::NAN, settings);
    assert!((waiting.timestep - 1.0 / 57.5).abs() < 1e-6);
    let predicted = controller.update(0.1, 1.0, settings);
    let expected = 1.0 / (60.0 - 25.0 * (0.2 / 1.1));
    assert!((predicted.timestep - expected).abs() < 1e-6);
}
