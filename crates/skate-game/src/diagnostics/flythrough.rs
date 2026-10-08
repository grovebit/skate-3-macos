//! Free-camera route for the performance harness: flies the gameplay camera once
//! around the current map during the sample window, so a profile covers the
//! district with no user input. The skater stays at the spawn.
//!
//! The route loops through the map's authored travel destinations in
//! nearest-neighbour order from the spawn; a map with fewer than three gets a
//! rectangle inset in its collision bounds. The camera holds `CLEARANCE` over the
//! topmost collision surface, climbing before buildings, and looks `LOOK_AHEAD`
//! metres down the route.
use super::performance::Performance;
use crate::{
    camera::GameplayCamera, config::Config, menu::teleport_menu, physics::GamePhysics,
    world::map_transition::CurrentMap,
};
use bevy::prelude::*;
use skate_core::math::Vector3;

/// Metres between route samples.
const STEP: f32 = 2.0;
/// Camera height over the topmost surface beneath it, near the gameplay camera.
const CLEARANCE: f32 = 3.0;
/// Metres either side within which a taller surface lifts the camera; the same
/// span then smooths the lift into a climb.
const LIFT: f32 = 30.0;
/// Route samples either side that `LIFT` spans.
const REACH: isize = (LIFT / STEP) as isize;
/// The camera looks this far down the route, at `EYE` over the surface there.
const LOOK_AHEAD: f32 = 40.0;
const EYE: f32 = 1.5;

pub(super) struct FlythroughPlugin;
impl Plugin for FlythroughPlugin {
    fn build(&self, app: &mut App) {
        // After every other camera writer, so the flown pose is the one rendered.
        app.add_systems(
            Update,
            (plan, fly.run_if(resource_exists::<Route>))
                .chain()
                .after(crate::modding::ModCameraSet)
                .before(crate::app::FrameSet::Verification),
        );
    }
}

/// Camera positions every `STEP` metres around the closed loop, with the surface
/// height beneath each.
#[derive(Resource)]
struct Route {
    points: Vec<Vec3>,
    ground: Vec<f32>,
}

impl Route {
    fn length(&self) -> f32 {
        self.points.len() as f32 * STEP
    }

    /// Camera position and surface height `distance` metres along the loop.
    fn at(&self, distance: f32) -> (Vec3, f32) {
        let along = distance.rem_euclid(self.length()) / STEP;
        let (i, t) = (along as usize % self.points.len(), along.fract());
        let j = (i + 1) % self.points.len();
        let ground = self.ground[i] + (self.ground[j] - self.ground[i]) * t;
        (self.points[i].lerp(self.points[j], t), ground)
    }
}

/// Builds the route once the map's collision world exists.
fn plan(
    mut commands: Commands,
    route: Option<Res<Route>>,
    map: Res<CurrentMap>,
    config: Res<Config>,
    physics: Res<GamePhysics>,
) {
    let Some(path) = map.path.as_ref() else { return };
    let triangles = physics.world_triangles();
    if route.is_some() || triangles.is_empty() {
        return;
    }
    let (low, high) = triangles
        .iter()
        .flat_map(|t| t.triangle.vertices)
        .map(|v| Vec3::new(v.x, v.y, v.z))
        .fold((Vec3::MAX, Vec3::MIN), |(low, high), v| (low.min(v), high.max(v)));

    let mut waypoints: Vec<Vec3> = teleport_menu::load(&config.asset_root)
        .unwrap_or_else(|error| {
            warn!("SKATE_FLYTHROUGH travel destinations: {error}");
            Vec::new()
        })
        .into_iter()
        .filter(|d| teleport_menu::same_map(path, &d.map))
        .filter_map(|d| d.matrix.map(|m| Vec3::new(m[3][0], m[3][1], m[3][2])))
        .collect();
    if waypoints.len() < 3 {
        let (center, half) = ((low + high) / 2.0, (high - low) * 0.35);
        waypoints = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .map(|(x, z)| Vec3::new(center.x + x * half.x, center.y, center.z + z * half.z))
            .to_vec();
    }
    let mut order = Vec::with_capacity(waypoints.len());
    let mut last = Vec3::from_array(map.spawn);
    while !waypoints.is_empty() {
        let distance = |w: &Vec3| last.xz().distance_squared(w.xz());
        let next = (0..waypoints.len())
            .min_by(|&a, &b| distance(&waypoints[a]).total_cmp(&distance(&waypoints[b])))
            .unwrap();
        last = waypoints.swap_remove(next);
        order.push(last);
    }

    let mut samples = Vec::new();
    for (i, &a) in order.iter().enumerate() {
        let b = order[(i + 1) % order.len()];
        let count = (a.xz().distance(b.xz()) / STEP).ceil().max(1.0) as usize;
        samples.extend((0..count).map(|k| a.lerp(b, k as f32 / count as f32)));
    }
    // A vertical line's nearest hit is the topmost surface: roof, ledge or ground.
    let ground: Vec<f32> = samples
        .iter()
        .map(|p| {
            let top = Vector3::new(p.x, high.y + 1.0, p.z);
            let bottom = Vector3::new(p.x, low.y - 1.0, p.z);
            physics.world().query_thin_line(top, bottom).ok().flatten()
                .map_or(p.y, |hit| hit.geometry.position.y)
        })
        .collect();
    let count = ground.len() as isize;
    let lifted: Vec<f32> = (0..count).map(|i| window(&ground, i).fold(f32::MIN, f32::max)).collect();
    let points = (0..count)
        .map(|i| {
            let height = window(&lifted, i).sum::<f32>() / (2 * REACH + 1) as f32;
            let p = samples[i as usize];
            Vec3::new(p.x, height + CLEARANCE, p.z)
        })
        .collect();
    let route = Route { points, ground };
    info!(
        "SKATE_FLYTHROUGH map={} waypoints={} length={:.0}m",
        map.name,
        order.len(),
        route.length()
    );
    commands.insert_resource(route);
}

/// The values within `REACH` samples of sample `i` around the closed loop.
fn window(values: &[f32], i: isize) -> impl Iterator<Item = f32> + '_ {
    let count = values.len() as isize;
    (-REACH..=REACH).map(move |d| values[(i + d).rem_euclid(count) as usize])
}

/// Holds the route start through the warmup, then flies the loop once over the
/// sample window.
fn fly(
    performance: Res<Performance>,
    route: Res<Route>,
    mut cameras: Query<&mut Transform, With<GameplayCamera>>,
) {
    let distance = performance.sample_progress() * route.length();
    let (eye, _) = route.at(distance);
    let (ahead, ground) = route.at(distance + LOOK_AHEAD);
    let target = Vec3::new(ahead.x, ground + EYE, ahead.z);
    for mut transform in &mut cameras {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}
