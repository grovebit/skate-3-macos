//! Linear gated type-1 envelope, base-disc 829299F0..82929CE0.
//! Supports descriptor bit 8 set, with either value of bit 10. The caller supplies
//! the original evaluator's milliseconds and resolved trigger. Log conversion,
//! multiplier controls and controller lifetime remain separate dependencies.
use super::curve::CurveTable;
use super::scalar::ScalarTables;
pub mod mxb;

/// Logarithmic output controls for envelopes with descriptor bit 9 clear.
/// Metadata construction: 8292B2F0..8292B5F0. Post-state evaluation:
/// 82928C08..829291CC. Bit 9 set uses another path and is not implemented here.
#[derive(Clone, Copy)]
pub struct LevelControl {
    base: i32,
    depth: i32,
}

impl LevelControl {
    pub fn from_authored(authored: i16, tables: &ScalarTables) -> Self {
        let metadata = tables.declaration(authored);
        Self {
            base: metadata.base,
            depth: metadata.depth,
        }
    }

    /// Linear multipliers are resolved in binding order. A missing array skips
    /// multiplication entirely; a present array executes its low-byte count.
    /// The caller handles the evaluator's idle fast path before this stage.
    pub fn evaluate(
        &self,
        linear: u16,
        multipliers: Option<&[u16]>,
        tables: &ScalarTables,
    ) -> Option<i32> {
        if linear > 32767 {
            return None;
        }
        let scaled = (i32::from(linear) * self.depth) >> 15;
        let shaped = if self.base > 0 {
            // Preserve multiplication before subtraction: rewriting this as
            // (32767-linear)*depth changes integer rounding.
            32767 - self.depth + scaled
        } else {
            32767 - scaled
        };
        let log = self.base + tables.linear_to_log(shaped as u16)?;
        let Some(multipliers) = multipliers else {
            return Some(log);
        };
        let weight = super::control_weight(multipliers)?;
        Some(log.wrapping_mul(weight) >> 15)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle = 0,
    Attack = 1,
    Sustain = 3,
    Release = 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub phase: Phase,
    pub elapsed: f32,
    pub offset: f32,
    pub start_level: i32,
    pub level: i32,
}

#[derive(Clone, Copy)]
pub struct GatedEnvelope {
    attack: f32,
    release: f32,
    attack_curve: u8,
    release_curve: u8,
    retrigger: bool,
    state: State,
}

impl GatedEnvelope {
    pub fn new(attack: f32, release: f32, attack_curve: u8, release_curve: u8) -> Option<Self> {
        let maximum = 4095.0 * f32::from_bits(0x41855557);
        if [attack, release]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0 || *v > maximum)
            || attack_curve > 15
            || release_curve > 15
        {
            return None;
        }
        Some(Self {
            attack,
            release,
            attack_curve,
            release_curve,
            retrigger: false,
            state: State::default(),
        })
    }

    /// Decode the two authored duration/curve words for a gated record.
    /// Constructor 8292B3A0..8292B3E4 replaces zero durations with one tick;
    /// binding 82927C78..82927D28 then scales ticks into evaluator milliseconds.
    pub fn from_authored(attack_word: u32, release_word: u32, retrigger: bool) -> Self {
        let duration = |word: u32| (word & 4095).max(1) as f32 * f32::from_bits(0x41855557);
        let mut envelope = Self::new(
            duration(attack_word),
            duration(release_word),
            ((attack_word >> 12) & 15) as u8,
            ((release_word >> 12) & 15) as u8,
        )
        .unwrap();
        envelope.retrigger = retrigger;
        envelope
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// Clear the native idle/completion fields, without resetting owner state.
    pub fn reset(&mut self) {
        self.state = State::default();
    }

    /// Advance once per original evaluator invocation. Unsupported diagnostic
    /// inputs leave state unchanged; this does not define native NaN behavior.
    pub fn advance(&mut self, delta: f32, trigger: bool, table: &CurveTable) -> Option<i32> {
        if !delta.is_finite() || delta < 0.0 {
            return None;
        }
        let mut next = self.state;
        let level = self.advance_state(&mut next, delta, trigger, table)?;
        self.state = next;
        Some(level)
    }

    fn advance_state(
        &self,
        state: &mut State,
        delta: f32,
        trigger: bool,
        table: &CurveTable,
    ) -> Option<i32> {
        if state.phase == Phase::Idle && !trigger {
            *state = State::default();
            return Some(0);
        }
        let elapsed = state.elapsed + delta;
        if !elapsed.is_finite() {
            return None;
        }
        let mut remapped = self.release;
        if state.phase == Phase::Attack && !trigger && self.attack >= f32::from_bits(0x418553f8) {
            remapped = ((self.attack - elapsed) / self.attack) * self.release;
            if !remapped.is_finite() {
                return None;
            }
        }
        state.elapsed = elapsed;
        if state.phase == Phase::Release {
            // Completion wins over a renewed trigger (82929A84..82929AA8).
            if elapsed >= self.release {
                *state = State::default();
                return Some(0);
            }
            if trigger && self.retrigger {
                let offset = if self.release >= f32::from_bits(0x418553f8) {
                    ((self.release - elapsed) / self.release) * self.attack
                } else {
                    self.attack
                };
                if !offset.is_finite() {
                    return None;
                }
                state.phase = Phase::Attack;
                state.start_level = state.level;
                state.elapsed = offset;
                state.offset = offset;
            }
        }
        if state.phase == Phase::Idle {
            state.phase = Phase::Attack;
        }
        if state.phase == Phase::Attack {
            if !trigger {
                state.phase = Phase::Release;
                state.start_level = state.level;
                state.elapsed = remapped;
                state.offset = remapped;
            } else if state.elapsed >= self.attack {
                state.phase = Phase::Sustain;
                state.elapsed = 0.0;
                state.offset = 0.0;
                state.start_level = 32767;
            } else {
                let duration = self.attack - state.offset;
                let mut position = state.elapsed - state.offset;
                if duration > 0.0 {
                    position /= duration;
                }
                let curve = table.evaluate_float(position, u32::from(self.attack_curve))?;
                state.level =
                    state.start_level + (curve * (32767 - state.start_level) as f32) as i32;
                return Some(state.level);
            }
        }
        if state.phase == Phase::Sustain {
            if trigger {
                state.level = 32767;
                state.elapsed = 0.0;
                return Some(state.level);
            }
            state.phase = Phase::Release;
            state.elapsed = 0.0;
            state.offset = 0.0;
            state.start_level = 32767;
        }
        if state.elapsed >= self.release {
            *state = State::default();
            return Some(0);
        }
        let duration = self.release - state.offset;
        let mut position = state.elapsed - state.offset;
        if duration > 0.0 {
            position /= duration;
        }
        let curve = table.evaluate_float(1.0 - position, u32::from(self.release_curve))?;
        state.level = state.start_level - ((1.0 - curve) * state.start_level as f32) as i32;
        Some(state.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table() -> CurveTable {
        CurveTable::from_be_bytes(&vec![0; 513 * 4]).unwrap()
    }

    #[test]
    fn log_shaping_preserves_sign_branch_and_null_multiplier_array() {
        let logs: Vec<u8> = (0_i32..512).flat_map(i32::to_be_bytes).collect();
        let tables = ScalarTables::from_be_bytes(&logs, &vec![0; 602 * 4]).unwrap();
        let positive = LevelControl::from_authored(100, &tables);
        let negative = LevelControl::from_authored(-1100, &tables);
        assert_eq!(positive.evaluate(1, None, &tables), Some(-9900));
        assert_eq!(negative.evaluate(1, None, &tables), Some(-91));
        assert_eq!(positive.evaluate(32767, None, &tables), Some(9));
        assert_eq!(positive.evaluate(32767, Some(&[]), &tables), Some(8));
        assert_eq!(positive.evaluate(32767, Some(&[0]), &tables), Some(0));
        assert_eq!(positive.evaluate(32767, Some(&[32768]), &tables), None);
    }

    #[test]
    fn sustain_release_discards_delta_and_does_not_retrigger() {
        let mut env = GatedEnvelope::new(100.0, 50.0, 8, 8).unwrap();
        let table = table();
        assert_eq!(env.advance(200.0, true, &table), Some(32767));
        assert_eq!(env.state().phase, Phase::Sustain);
        assert_eq!(env.advance(200.0, false, &table), Some(0));
        assert_eq!(env.state().phase, Phase::Release);
        assert_eq!(env.state().elapsed, 0.0);
        assert_eq!(env.advance(50.0, true, &table), Some(0));
        assert_eq!(env.state(), State::default());
        env.advance(1.0, true, &table).unwrap();
        assert_eq!(env.state().phase, Phase::Attack);
    }

    #[test]
    fn aborted_attack_keeps_negative_offset_and_rejects_overflow_atomically() {
        let table = table();
        let mut env = GatedEnvelope::new(100.0, 50.0, 8, 8).unwrap();
        env.advance(10.0, true, &table).unwrap();
        env.advance(200.0, false, &table).unwrap();
        assert_eq!(env.state().offset, -55.0);
        let before = env.state();
        assert_eq!(env.advance(f32::NAN, true, &table), None);
        assert_eq!(env.state(), before);
        let mut env = GatedEnvelope::new(17.0, 68000.0, 8, 8).unwrap();
        env.advance(1.0, true, &table).unwrap();
        let before = env.state();
        assert_eq!(env.advance(f32::MAX, false, &table), None);
        assert_eq!(env.state(), before);
    }

    #[test]
    fn authored_zero_attack_is_one_tick_and_release_completion_wins_retrigger() {
        let table = table();
        let mut env = GatedEnvelope::from_authored(0x9000, 0x9003, true);
        let tick = f32::from_bits(0x41855557);
        assert_eq!(env.attack, tick);
        env.advance(tick, true, &table).unwrap();
        env.advance(0.0, false, &table).unwrap();
        env.advance(tick, true, &table).unwrap();
        assert_eq!(env.state().phase, Phase::Attack);
        assert_eq!(env.state().start_level, 32767);
        env.advance(tick, true, &table).unwrap();
        env.advance(0.0, false, &table).unwrap();
        env.advance(env.release, true, &table).unwrap();
        assert_eq!(env.state(), State::default());
    }
}
