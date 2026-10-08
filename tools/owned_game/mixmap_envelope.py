"""Gated, non-retriggering type-1 envelope from base-disc 829299F0.

This is the flag combination used by collision control B100304C: bit 8 set,
bit 10 clear. Inputs are explicit evaluator clock deltas, not host frame times.
Only the linear envelope state is modeled; logarithmic conversion, multiplier
controls, ownership, input publication, and other flag combinations are not.
"""
from dataclasses import dataclass, field
from enum import IntEnum
import math

from tools.owned_game.mixmap_curve import _f32, evaluate_curve, evaluate_float_curve


class Phase(IntEnum):
    IDLE = 0
    ATTACK = 1
    SUSTAIN = 3
    RELEASE = 4


@dataclass
class GatedEnvelope:
    attack_duration: float
    release_duration: float
    attack_curve: int
    release_curve: int
    table: tuple
    phase: Phase = field(default=Phase.IDLE, init=False)
    elapsed: float = field(default=0.0, init=False)
    offset: float = field(default=0.0, init=False)
    start_level: int = field(default=0, init=False)
    level: int = field(default=0, init=False)

    def __post_init__(self):
        for name in ('attack_duration', 'release_duration'):
            value = getattr(self, name)
            maximum = _f32(0xFFF * 16.666669845581055)  # 822F3BA4
            if not math.isfinite(value) or not 0 < value <= maximum:
                raise ValueError('Duration must be positive and within the authored clock range')
            value = _f32(value)
            if value == 0:
                raise ValueError('Duration must remain positive in binary32')
            setattr(self, name, value)
        for curve in (self.attack_curve, self.release_curve):
            if not isinstance(curve, int) or not 0 <= curve <= 15:
                raise ValueError('Envelope curve must be a four-bit selector')
        self.table = tuple(self.table)
        evaluate_curve(0, 8, self.table)  # Validate once before mutating state.

    def reset(self):
        """Clear the fields reset by native idle/completion, not owner lifetime."""
        self.phase = Phase.IDLE
        self.elapsed = self.offset = 0.0
        self.start_level = self.level = 0

    def advance(self, delta, trigger):
        """Return the linear level after one actual evaluator invocation.

        Caller must supply a bool trigger and finite nonnegative binary32 delta.
        Transition ordering and discarded overshoot match the original helper.
        """
        if not isinstance(trigger, bool):
            raise ValueError('Trigger must be a boolean')
        if not math.isfinite(delta) or not 0 <= delta <= 3.4028234663852886e38:
            raise ValueError('Delta must be a finite nonnegative binary32 value')
        delta = _f32(delta)
        if self.phase == Phase.IDLE and not trigger:
            # 82928B54..82928B84 skips the helper and clears idle state.
            self.reset()
            return self.level
        elapsed = self.elapsed + delta
        if elapsed > 3.4028234663852886e38:
            raise ValueError('Accumulated envelope time exceeds binary32 range')
        elapsed = _f32(elapsed)
        remapped = self.release_duration
        if self.phase == Phase.ATTACK and not trigger:
            # 822F377C, bits 418553F8. Short attacks complete release
            # immediately; longer ones remap elapsed time into release.
            if self.attack_duration >= 16.666000366210938:
                remaining = _f32(self.attack_duration - elapsed)
                fraction = _f32(remaining / self.attack_duration)
                try:
                    remapped = _f32(fraction * self.release_duration)
                except OverflowError as error:
                    raise ValueError('Release remapping exceeds binary32 range') from error
        self.elapsed = elapsed
        if self.phase == Phase.IDLE:
            self.phase = Phase.ATTACK

        if self.phase == Phase.ATTACK:
            if not trigger:
                self.phase = Phase.RELEASE
                self.start_level = self.level
                self.elapsed = self.offset = remapped
            elif self.elapsed >= self.attack_duration:
                self.phase = Phase.SUSTAIN
                self.elapsed = self.offset = 0.0
                self.start_level = 32767
            else:
                duration = _f32(self.attack_duration - self.offset)
                position = _f32(self.elapsed - self.offset)
                if duration > 0:
                    position = _f32(position / duration)
                curve = evaluate_float_curve(position, self.attack_curve, self.table)
                self.level = self.start_level + int(_f32(curve * (32767 - self.start_level)))
                return self.level

        if self.phase == Phase.SUSTAIN:
            if trigger:
                self.level = 32767
                self.elapsed = 0.0
                return self.level
            self.phase = Phase.RELEASE
            self.elapsed = self.offset = 0.0
            self.start_level = 32767

        if self.elapsed >= self.release_duration:
            # Do not restart even if trigger is true on this invocation.
            self.reset()
            return self.level
        duration = _f32(self.release_duration - self.offset)
        position = _f32(self.elapsed - self.offset)
        if duration > 0:
            position = _f32(position / duration)
        curve = evaluate_float_curve(_f32(1.0 - position), self.release_curve, self.table)
        removed = int(_f32(_f32(1.0 - curve) * self.start_level))
        self.level = self.start_level - removed
        return self.level
