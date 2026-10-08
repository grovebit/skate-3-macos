//! Duration dependency of the scoring worker, after completed physical output.
use super::SkaterRuntime;
use skate_core::audio::hom::duration::PublicationInputs;

pub(super) fn advance(skater: &mut SkaterRuntime) {
    let flags = &skater.player_state.state_flags;
    let physical = &mut skater.player_input.physical;
    physical.scoring.wipeout_duration_3518 =
        skater.body_audio_publication.advance(PublicationInputs {
            filtered: physical.filtered_state_0 as i32,
            state_44: u8::from(flags[68 - 52]),
            state_45: u8::from(flags[69 - 52]),
            motion_1c4: physical.air.use_air_reckoning_452,
            skeleton_257: physical.skeleton.over_599,
        });
}
