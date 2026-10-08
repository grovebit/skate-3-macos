//! Engine-independent recovered calculations. No rendering, file I/O, or ECS.
//! The game schedules these systems; full gameplay parity remains under review.
//! Arithmetic uses independent Rust implementations. Original-hardware numerical
//! agreement is unverified.
#![forbid(unsafe_code)]
pub mod air;
pub mod animation;
pub mod audio;
pub mod camera;
pub mod graph;
pub mod input;
pub mod math;
pub mod physics;
pub mod player;
pub mod point_graph;
pub mod riding;
pub mod scoring;
pub mod trigonometry;
