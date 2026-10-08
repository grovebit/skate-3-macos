import struct
import unittest

from tools.audio.inspect_collision_start import inspect
from tools.owned_game.splc import event_controls, event_layers, event_pan_choices
from tools.audio.test_impact_audio import control_bank
from tools.audio.verify_aems_selection import rng_step, select


class AemsStartTests(unittest.TestCase):
    def test_decodes_controls_without_changing_legacy_views(self):
        data = control_bank()
        old_samples = event_layers(data)
        struct.pack_into('>3f', data, 60 + 8, .5, 1., -.25)
        values = (.25, .5, 1., 40., .125, .0625, 2., .25, 1.5)
        struct.pack_into('>9f', data, 216 + 4, *values)
        data[216 + 0x28] = 0x34
        struct.pack_into('>3f', data, 216 + 0x2c, .5, -.125, .25)
        struct.pack_into('>f', data, 216 + 0x40, .75)
        data[216 + 0x44] = 1
        data[216 + 3] = 255
        data[216 + 0x3c] = 254
        events = event_controls(data)
        leaf = events[0].variants[0]
        self.assertEqual((leaf.gain, leaf.source_rate, leaf.source_rate_range), (.5, 1., -.25))
        layer = leaf.layers[0]
        self.assertEqual((layer.selection_mode, layer.initial_state), (1, 0x10000))
        c = layer.choices[0]
        self.assertEqual((c.sample, c.offset, c.gain, c.source_rate), (2, 216, .25, .5))
        self.assertEqual((c.delay, c.source_start, c.source_end, c.attack_end, c.release_start),
                         (.125, .0625, 2., .25, 1.5))
        self.assertEqual((c.envelope_curve, c.gain_randomization, c.source_rate_range,
                          c.delay_range, c.probability, c.routing_flag),
                         (4, .5, -.125, .25, .75, 1))
        self.assertEqual((c.effect_index, c.priority), (255, -2))
        self.assertEqual(event_layers(data), old_samples)
        self.assertEqual(event_pan_choices(data)[0][0][0][0].angle_degrees, 40.)
        self.assertEqual((events[2].selection_mode, events[2].initial_state), (2, 0x10000))
        self.assertIs(events[2].variants[0], leaf)

    def test_audit_deduplicates_shared_leaves_and_choices(self):
        data = control_bank()
        report = inspect(data, [0, 2, 2])
        self.assertEqual((report['events'], report['unique_leaves'], report['unique_layers'],
                          report['unique_choices']), (2, 2, 3, 4))
        self.assertEqual(report['event_modes'], {'alternating_half_shuffle': 1, 'leaf': 1})
        self.assertEqual(report['leaf_layer_counts'], {'1': 1, '2': 1})
        self.assertEqual(report['layer_modes'], {'sequential': 3})
        self.assertEqual(report['choice_fields']['delay'], {'0.0': 4})
        for event in (-1, 3):
            with self.assertRaisesRegex(ValueError, 'outside'):
                inspect(data, [event])

    def test_random_width_and_boundaries(self):
        self.assertEqual(rng_step(1), (2745024, 41))
        self.assertEqual(rng_step(0xffffffff), (2316998, 35))
        for count in range(2, 33):
            self.assertEqual(select(count, 0, 0x10000, 0), (0, 0x10000))
            self.assertEqual(select(count, 0, 0x10000, 32767), (count - 1, 0x10000))
        for mode in range(3):
            self.assertEqual(select(1, mode, 0x12345678, 32767), (0, 0x12345678))

    def test_sequential_increments_before_return_and_wraps(self):
        self.assertEqual(select(3, 1, 0x10000, 0), (2, 2))
        self.assertEqual(select(3, 1, 2, 0), (0, 0))
        self.assertEqual(select(3, 1, 0, 0), (1, 1))
        self.assertEqual(select(3, 1, 0xffffffff, 0), (0, 0))
        self.assertEqual(select(3, 1, 0x7fffffff, 0), (254, 0xfffffffe))

    def test_shuffle_initial_partial_half_and_refill(self):
        state = 0x10000
        indices = []
        for _ in range(11):
            index, state = select(5, 2, state, 0)
            indices.append(index)
        self.assertEqual(indices, [0, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1])
        self.assertEqual(state, 0x70001)
        self.assertEqual(select(5, 2, 0, 0), (0, 0))  # No available mask bits.
        self.assertEqual(select(5, 2, 0x70001, 32767), (2, 0x60001))


if __name__ == '__main__':
    unittest.main()
