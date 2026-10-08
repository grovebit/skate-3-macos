//! Final contact writer in base-disc physical publication82D8D420.

/// Dispatch reads Processed+9CC, not the newly selected State+10.
pub(super) fn wipeout_contact(processed_state: u32, collision_d6: u8, prior: u8) -> u8 {
    //82D8D4CC..D4F4 preserves an already-set byte, including noncanonical ones.
    if processed_state == 300 && collision_d6 != 0 && prior == 0 {
        1
    } else {
        prior
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skating_audio_suppression_dispatch_and_set_only_bytes() {
        for state in [0, 100, 299, 300, 301, 503, 601, 602, u32::MAX] {
            for contact in [0, 1, 2, 255] {
                for prior in [0, 1, 2, 255] {
                    let expected = if state == 300 && contact != 0 && prior == 0 {
                        1
                    } else {
                        prior
                    };
                    assert_eq!(wipeout_contact(state, contact, prior), expected);
                }
            }
        }
    }

    #[test]
    #[ignore = "requires converted stock assets; headless publication regression"]
    fn skating_audio_suppression_packet_lifetime_and_writer_order() {
        use crate::physics::{GamePhysics, SkaterRuntime, body_audio_publication, player_input};
        use skate_core::player::{lifecycle::PhysicalPlayerStateLifecycle, state::PhysicalStateId};

        let root = std::env::var_os("SKATE3_ASSET_ROOT").expect("set SKATE3_ASSET_ROOT");
        let root = std::path::Path::new(&root);
        let assets = skate_data::GameAssets::load(root).unwrap();
        let graphs = crate::animation::graph_runtime::StockGraphs::load(root, &assets).unwrap();
        let mut physics = GamePhysics::load(root).unwrap();
        let mut skater = SkaterRuntime::load(root, &graphs, &physics, "normal").unwrap();
        let p = &skater.player_input.processed;
        skater.player_input.toolkit = Some(
            skate_core::physics::board_toolkit::BoardToolkit::from_board(
                &physics.board,
                p.flags_2468,
                p.scalar_2612,
                p.vectors_464_480_496_512_528[0].map(f32::from_bits),
                [0.0, 1.0, 0.0, 0.0],
            ),
        );
        skater.player_state.lifecycle =
            PhysicalPlayerStateLifecycle::new(PhysicalStateId::WipeoutGround);

        // Execute the actual template clear, selector, driver, final contact
        // writer and scoring adapter. False later writers must retain true.
        for (processed, contact, selector, countdown, driver) in [
            (300, 0, false, -1, false),
            (300, 255, false, -1, false),
            (300, 0, true, -1, false),
            (300, 0, false, 0, false),
            (300, 0, false, -1, true),
            (300, 1, true, 2, true),
            (299, 1, false, -1, false),
            (300, 0, false, -1, false),
        ] {
            skater.player_state.state_flags[68 - 52] = true;
            skater.player_state.state_flags[69 - 52] = true;
            skater.player_input.physical.collision.material_six_214 = 255;
            player_input::reset_outputs(&mut skater.player_input.physical);
            assert_eq!(skater.player_input.physical.collision.material_six_214, 0);
            skater.player_input.processed.state_2508 = processed;
            skater.collision_feedback.flags.material_6 = contact != 0;
            skater.player_state.selector.request_teleport = selector;
            skater.wipeout_state.state.teleport_countdown = countdown;
            skater.wipeout_state.state.request_teleport = driver;
            super::super::publication::publish(&mut physics, &mut skater).unwrap();
            assert_eq!(
                skater.player_input.physical.collision.material_six_214,
                u8::from(contact != 0)
            );
            let expected = selector || driver || (processed == 300 && contact != 0);
            assert_eq!(skater.player_state.state_flags[68 - 52], countdown >= 0);
            assert_eq!(skater.player_state.state_flags[69 - 52], expected);
            assert_eq!(
                skater.player_input.physical.state.flag_69,
                u8::from(expected)
            );
            body_audio_publication::advance(&mut skater);
            assert_eq!(
                skater.body_audio_publication.gate.active_8b4,
                u8::from(!expected && countdown < 0)
            );
        }
    }
}
