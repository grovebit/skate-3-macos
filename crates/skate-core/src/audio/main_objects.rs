//! Main-family SFXObj input updates (vtable +24) for the collision volume
//! cone. Each function ports the slots of one original update that reach
//! the collision voices' volume outputs, from explicit original fields;
//! slots outside that cone are not published here. Base-disc default.xex
//! SHA-256 1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f.

/// Audio-manager (*(830734E4)) fields read by these updates. `Default` is
/// constructor 824724E8. They change only through front-end, NIS, movie and
/// challenge flows (82478850, virtual +40/+44/+50/+54, 82476170).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManagerState {
    /// +49C, eAudioFEStates of the active fe_hudstate row (82478850).
    pub fe_state: i32,
    /// +418: 1 after a row with music state 4 or 5 (82478930..82478950).
    pub music_418: i32,
    /// +1C4 and +1CC, set by virtual +50 and cleared by +54.
    pub nis_state: i32,
    pub nis_flag: u8,
    /// +36C, the event type from 82476170; 0x37 without an event.
    pub event_type: i32,
    /// +368, set by virtual +40 and cleared by +44 (movie/flow screens).
    pub movie: u8,
}
impl Default for ManagerState {
    fn default() -> Self {
        Self {
            fe_state: 0,
            music_418: 0,
            nis_state: 0,
            nis_flag: 0,
            event_type: 0x37,
            movie: 0,
        }
    }
}

const ON: u32 = 32767;

/// 82475D10: tutorial FE state 3 with the 8249B0D8 condition, or state 7.
/// `tutorial_active` is the condition's `+8 -> +38 -> +3C == 1` result.
pub fn fe_query(manager: &ManagerState, tutorial_active: bool) -> bool {
    match manager.fe_state {
        3 => tutorial_active,
        7 => true,
        _ => false,
    }
}

/// 824CF428: NIS one-hot index, 11 (none) when +1C4 is zero.
pub fn nis_index(manager: &ManagerState) -> u32 {
    match manager.nis_state {
        0 => 11,
        4 => match manager.event_type {
            7 | 0x27..=0x29 | 0x2e | 0x30 => 7,
            8 | 0xf | 2 => 6,
            _ => 1,
        },
        3 => 2,
        // 824CF4DC..824CF538: +1CC selects the alternative.
        0x19 => 3 + if manager.nis_flag != 0 { 3 } else { 0 },
        0x17 => 3 + if manager.nis_flag != 0 { 4 } else { 0 },
        0x18 => 3 - if manager.nis_flag != 0 { 2 } else { 0 },
        // 824CF5DC.
        5 | 0x1a | 2 => 3,
        0x10 => 1,
        0x12 | 0xf => 5,
        6 => 0,
        0xc | 0xd => 4,
        0x13 | 0x15 => 8,
        0x14 => 9,
        0x1b => 10,
        _ => 11,
    }
}

/// SFXObj_NIS 824CED08: clears its one-hot slots, sets the selected ones,
/// then publishes the movie flag (slot 1) and FE query (slot 9).
pub fn publish_nis(manager: &ManagerState, fe_query: bool, words: &mut [u32; 16]) {
    for slot in [0, 2, 3, 4, 5, 6, 7, 8, 10, 11, 13, 12] {
        words[slot] = 0;
    }
    let slots: &[usize] = match nis_index(manager) {
        0 => &[2, 12],
        1 => &[3],
        2 => &[4],
        3 => &[5],
        4 => &[6],
        5 => &[0],
        6 => &[7],
        7 => &[8, 12],
        8 => &[10],
        9 => &[11],
        10 => &[13],
        _ => &[],
    };
    for &slot in slots {
        words[slot] = ON;
    }
    words[1] = if manager.movie != 0 { ON } else { 0 };
    words[9] = if fe_query { ON } else { 0 };
}

/// SFXObj_Pause retained byte +1C (1 at construction, 824CF680) and the
/// manager bytes it writes (+35D pause flag; +3B0 cleared on resume).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PauseState {
    pub latch_1c: u8,
    pub manager_35d: u8,
}
impl Default for PauseState {
    fn default() -> Self {
        Self {
            latch_1c: 1,
            manager_35d: 0,
        }
    }
}

/// Queries made by 824CF7D8, resolved by the caller.
pub struct PauseQueries {
    /// `824C2A50(ThePauseMan, 1)`: top pause context mask bit 0.
    pub paused: bool,
    pub fe_query: bool,
    /// 8277ADC0 (provider virtual +10); zero for the ordinary providers.
    pub master_query: bool,
    /// TheAudioCore byte +68, set by stream/movie setup 828B6C50.
    pub movie_core: bool,
}

impl PauseState {
    /// 824CF7D8..824CFA04. Returns whether manager +3B0 must be cleared.
    pub fn publish(
        &mut self,
        manager: &ManagerState,
        queries: &PauseQueries,
        words: &mut [u32; 16],
    ) -> bool {
        let pausing = queries.paused || queries.fe_query;
        let mut duck = false;
        if queries.paused
            && !queries.master_query
            && !queries.movie_core
            && manager.nis_state == 0
            && !queries.fe_query
        {
            duck = true;
        }
        let clear_3b0 = self.manager_35d != 0 && !pausing;
        self.manager_35d = u8::from(pausing);
        let (mut slot0, mut slot2) = (0, 0);
        if duck {
            if manager.music_418 == 1 {
                slot0 = ON;
            } else if self.latch_1c != 0 {
                self.latch_1c = 0;
            } else {
                slot2 = ON;
            }
        } else {
            self.latch_1c = 1;
        }
        words[0] = slot0;
        words[1] = if manager.fe_state == 6 { ON } else { 0 };
        words[2] = slot2;
        clear_3b0
    }
}

/// SFXObj_Challenge slot 5 (824DBF08..824DBF90). The latch is set only by
/// manager virtual +88 from a challenge flow (825FB7A0); a positive timing
/// ratio clears it. Returns the slot value and the new latch.
pub fn challenge_slot5(latch_1c: u8, timing_ratio: Option<f32>) -> (u32, u8) {
    match timing_ratio {
        Some(ratio) if latch_1c != 0 => {
            if ratio > 0.0 {
                (0, 0)
            } else {
                (ON, latch_1c)
            }
        }
        _ => (0, latch_1c),
    }
}

/// SFXObj_Menu FE-state switch 824DC720..824DC874 (slots 1, 3, 4 and 8).
pub fn publish_menu_states(manager: &ManagerState, words: &mut [u32; 16]) {
    let slot = match manager.fe_state {
        4 => Some(1),
        5 => Some(3),
        8 => Some(4),
        9 => Some(8),
        _ => None,
    };
    for s in [1, 3, 4, 8] {
        words[s] = if slot == Some(s) { ON } else { 0 };
    }
}

/// One of the speech manager's two active entries (*(830734FC)+8 +460/+464):
/// type word +04 and active byte +4C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechEntry {
    pub kind: i32,
    pub active: bool,
}

/// SFXObj_Speech slot 1 (824CFC94..824CFD18): an active entry of type
/// 0x4B..0x4D. Slots 0..4 are cleared first; only slot 1 is ported here.
pub fn speech_slot1(entries: &[Option<SpeechEntry>; 2]) -> u32 {
    let speaking = entries
        .iter()
        .flatten()
        .any(|e| e.active && (0x4b..=0x4d).contains(&e.kind));
    if speaking { ON } else { 0 }
}

/// The speech manager's +0C record queried by 824940F0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechRecord {
    /// +426 started, +470 finished, +428 type.
    pub started: bool,
    pub finished: bool,
    pub kind: i32,
}

/// SFXObj_CameraMan slot 0 (824BE690..824BE6FC): `824940F0(speech, 0)`.
pub fn cameraman_slot0(record: Option<SpeechRecord>) -> u32 {
    match record {
        Some(r) if r.started && !r.finished && r.kind == 2 => ON,
        _ => 0,
    }
}

/// SFXObj_Bloom slot 0 (824DB318..824DB41C): the larger of manager float
/// +4A8 (kind-4 visual-transition fade, 827725F8) and, without an NIS, the
/// channel-5 fade amount (+30C0 present, +30C4 value; cMsgFadeAmount),
/// times 32767, truncated and clamped.
pub fn bloom_slot0(manager: &ManagerState, manager_4a8: f32, channel5_fade: Option<f32>) -> u32 {
    let mut level = match channel5_fade {
        Some(fade) if manager.nis_state == 0 => fade,
        _ => 0.0,
    };
    if manager_4a8 > level {
        level = manager_4a8;
    }
    // fctiwz saturates; the clamp below keeps 0..32767 either way.
    ((level * 32767.0) as i32).clamp(0, 32767) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloom_takes_the_larger_fade_and_ignores_channel5_during_nis() {
        let mut manager = ManagerState::default();
        assert_eq!(bloom_slot0(&manager, 0.0, None), 0);
        assert_eq!(bloom_slot0(&manager, 0.25, Some(0.5)), 16383);
        assert_eq!(bloom_slot0(&manager, 0.75, Some(0.5)), 24575);
        manager.nis_state = 3;
        assert_eq!(bloom_slot0(&manager, 0.0, Some(0.5)), 0);
        assert_eq!(bloom_slot0(&manager, 2.0, None), 32767);
    }

    #[test]
    fn constructor_state_publishes_no_nis_menu_or_challenge_controls() {
        let manager = ManagerState::default();
        assert_eq!(nis_index(&manager), 11);
        let mut words = [7; 16];
        publish_nis(&manager, fe_query(&manager, true), &mut words);
        assert_eq!(&words[..14], &[0; 14]);
        assert_eq!(&words[14..], &[7, 7]);
        publish_menu_states(&manager, &mut words);
        assert_eq!(words[8], 0);
        assert_eq!(challenge_slot5(0, Some(0.0)), (0, 0));
    }

    #[test]
    fn nis_index_follows_the_original_state_table() {
        let state = |nis_state, event_type, nis_flag| ManagerState {
            nis_state,
            event_type,
            nis_flag,
            ..ManagerState::default()
        };
        for (s, e, f, index) in [
            (4, 0x29, 0, 7),
            (4, 0xf, 0, 6),
            (4, 0x37, 0, 1),
            (3, 0, 0, 2),
            (0x19, 0, 0, 3),
            (0x19, 0, 1, 6),
            (0x17, 0, 1, 7),
            (0x18, 0, 1, 1),
            (0x18, 0, 0, 3),
            (0x1a, 0, 0, 3),
            (2, 0, 0, 3),
            (6, 0, 0, 0),
            (0x12, 0, 0, 5),
            (0xd, 0, 0, 4),
            (0x15, 0, 0, 8),
            (0x14, 0, 0, 9),
            (0x1b, 0, 0, 10),
            (0x99, 0, 0, 11),
        ] {
            assert_eq!(nis_index(&state(s, e, f)), index, "{s:#x} {e:#x} {f}");
        }
        let mut words = [0; 16];
        publish_nis(&state(0x18, 0, 0), true, &mut words);
        assert_eq!((words[5], words[9], words[12]), (ON, ON, 0));
        publish_nis(&state(4, 2, 0), false, &mut words);
        assert_eq!((words[5], words[7], words[9]), (0, ON, 0));
    }

    #[test]
    fn pause_duck_slot_depends_on_music_state_and_retained_latch() {
        let paused = PauseQueries {
            paused: true,
            fe_query: false,
            master_query: false,
            movie_core: false,
        };
        let mut manager = ManagerState::default();
        let mut state = PauseState::default();
        let mut words = [0; 16];
        assert!(!state.publish(&manager, &paused, &mut words));
        // First paused update only clears the constructed latch.
        assert_eq!((words[0], words[2], state.latch_1c), (0, 0, 0));
        state.publish(&manager, &paused, &mut words);
        assert_eq!((words[0], words[2]), (0, ON));
        manager.music_418 = 1;
        state.publish(&manager, &paused, &mut words);
        assert_eq!((words[0], words[2]), (ON, 0));
        let resumed = PauseQueries {
            paused: false,
            ..paused
        };
        assert!(state.publish(&manager, &resumed, &mut words));
        assert_eq!((words[0], words[2], state.latch_1c), (0, 0, 1));
        manager.nis_state = 3;
        state.publish(&manager, &paused, &mut words);
        assert_eq!((words[0], words[2]), (0, 0));
    }

    #[test]
    fn speech_and_cameraman_need_the_original_entry_types() {
        assert_eq!(speech_slot1(&[None, None]), 0);
        let entry = |kind, active| Some(SpeechEntry { kind, active });
        assert_eq!(speech_slot1(&[entry(0x4c, true), None]), ON);
        assert_eq!(speech_slot1(&[entry(0x4c, false), entry(0x4e, true)]), 0);
        let record = |kind, finished| {
            Some(SpeechRecord {
                started: true,
                finished,
                kind,
            })
        };
        assert_eq!(cameraman_slot0(None), 0);
        assert_eq!(cameraman_slot0(record(2, false)), ON);
        assert_eq!(cameraman_slot0(record(2, true)), 0);
        assert_eq!(cameraman_slot0(record(1, false)), 0);
    }

    #[test]
    fn challenge_latch_publishes_only_while_time_is_stopped() {
        assert_eq!(challenge_slot5(1, Some(0.0)), (ON, 1));
        assert_eq!(challenge_slot5(1, Some(-0.0)), (ON, 1));
        assert_eq!(challenge_slot5(1, Some(0.5)), (0, 0));
        assert_eq!(challenge_slot5(1, None), (0, 1));
    }
}
