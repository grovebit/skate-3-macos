import unittest

from tools.owned_game.mixmap_envelope import GatedEnvelope, Phase


class MixMapEnvelopeTests(unittest.TestCase):
    table = tuple((i * 197) % 32768 for i in range(513))

    def envelope(self, attack=100.0, release=50.0):
        # Linear selectors isolate state transitions from authored table values.
        return GatedEnvelope(attack, release, 9, 9, self.table)

    def test_attack_sustain_release_and_idle(self):
        env = self.envelope()
        self.assertEqual(env.advance(20, False), 0)
        self.assertEqual(env.advance(50, True), 16383)
        self.assertEqual(env.advance(50, True), 32767)
        self.assertEqual(env.phase, Phase.SUSTAIN)
        self.assertEqual(env.advance(1000, True), 32767)
        self.assertEqual(env.elapsed, 0)
        # The delta on the trigger-off invocation is discarded at transition.
        self.assertEqual(env.advance(1000, False), 32767)
        self.assertEqual(env.phase, Phase.RELEASE)
        self.assertEqual(env.advance(25, False), 16383)
        self.assertEqual(env.advance(25, False), 0)
        self.assertEqual((env.phase, env.elapsed, env.offset, env.start_level),
                         (Phase.IDLE, 0, 0, 0))

    def test_interrupted_attack_preserves_level_and_remaps_time(self):
        env = self.envelope()
        self.assertEqual(env.advance(50, True), 16383)
        self.assertEqual(env.advance(0, False), 16383)
        self.assertEqual((env.phase, env.elapsed, env.offset, env.start_level),
                         (Phase.RELEASE, 25, 25, 16383))
        self.assertEqual(env.advance(12.5, False), 8192)
        self.assertEqual(env.advance(12.5, False), 0)

    def test_retrigger_does_not_interrupt_release_or_restart_at_completion(self):
        env = self.envelope()
        env.advance(100, True)
        env.advance(0, False)
        self.assertEqual(env.advance(25, True), 16383)
        self.assertEqual(env.phase, Phase.RELEASE)
        self.assertEqual(env.advance(25, True), 0)
        self.assertEqual(env.phase, Phase.IDLE)
        self.assertEqual(env.advance(50, True), 16383)
        self.assertEqual(env.phase, Phase.ATTACK)

    def test_abort_precedes_attack_completion(self):
        env = self.envelope()
        env.advance(50, True)
        # Trigger loss is handled before the now-overdue attack completes.
        self.assertEqual(env.advance(100, False), 16383)
        self.assertEqual((env.elapsed, env.offset), (-25, -25))
        self.assertEqual(env.advance(75, False), 0)

    def test_short_attack_abort_and_reset(self):
        env = self.envelope(attack=10)
        env.advance(5, True)
        self.assertEqual(env.advance(0, False), 0)
        self.assertEqual(env.phase, Phase.IDLE)
        env.advance(5, True)
        env.reset()
        self.assertEqual((env.phase, env.elapsed, env.offset, env.start_level, env.level),
                         (Phase.IDLE, 0, 0, 0, 0))

    def test_rejects_unsupported_inputs_before_changing_state(self):
        for duration in [0, -1, float('inf'), float('nan'), 100000]:
            with self.assertRaises(ValueError):
                self.envelope(attack=duration)
        env = self.envelope()
        for delta, trigger in [(-1, True), (float('nan'), True),
                               (float('inf'), True), (1, 32767)]:
            with self.assertRaises(ValueError):
                env.advance(delta, trigger)
            self.assertEqual((env.phase, env.elapsed, env.level), (Phase.IDLE, 0, 0))

    def test_release_remapping_overflow_leaves_state_unchanged(self):
        env = self.envelope(attack=20, release=60000)
        env.advance(1, True)
        before = vars(env).copy()
        with self.assertRaisesRegex(ValueError, 'Release remapping'):
            env.advance(3.4028234663852886e38, False)
        self.assertEqual(vars(env), before)


if __name__ == '__main__':
    unittest.main()
