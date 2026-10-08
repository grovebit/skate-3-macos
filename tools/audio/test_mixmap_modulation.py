import struct
import unittest

from tools.owned_game.mixmap_modulation import evaluate_node, linear_to_log, read_log_table


class ModulationTests(unittest.TestCase):
    curves = tuple((i * 197) % 32768 for i in range(513))
    logs = tuple(range(512))
    bounds = ((0, 10), (0, 20), (0, 30), (0, 40))

    def node(self, distance, phase, word=0x88880000, **kwargs):
        return evaluate_node(distance, phase, self.bounds, word, self.curves, self.logs,
                             **kwargs)

    def test_directional_bounds_and_integer_blending(self):
        self.assertEqual(self.node(0, 0)['linear_level'], 32766)
        self.assertEqual(self.node(5, 0)['linear_level'], 16383)
        self.assertEqual(self.node(5, 16384)['linear_level'], 24575)
        self.assertEqual(self.node(5, 49152)['linear_level'], 28671)
        self.assertEqual(self.node(5, 8192)['linear_level'], 20479)

    def test_nonsequential_curve_nibbles(self):
        # Curve 8 complements input; curve 9 is identity. Other nibbles are 15.
        for quadrant, word, expected in [(0, 0x8FFF0000, 16383),
                                          (1, 0xFFF90000, 8190),
                                          (2, 0xF8FF0000, 27305),
                                          (3, 0xFF9F0000, 4094)]:
            self.assertEqual(self.node(5, quadrant << 14, word)['linear_level'], expected)

    def test_disabled_far_and_unsupported_curve(self):
        silence = {'linear_level': 0, 'log_level': -10000}
        self.assertEqual(self.node(0, 0, enabled=False), silence)
        self.assertEqual(self.node(21, 0), silence)
        self.assertEqual(self.node(0, 0, 0xFFFF0000), silence)
        # At the first upper bound the second curve can still contribute.
        self.assertGreater(self.node(10, 8192)['linear_level'], 0)

    def test_log_bins_and_sparse_upper_endpoint(self):
        for level, expected in [(0, -10000), (1, 511 - 9030),
                                 (2, 255 - 8428), (3, 511 - 8428),
                                 (511, 511 - 4214), (512, -3612),
                                 (16384, -602), (32767, 511 - 602)]:
            self.assertEqual(linear_to_log(level, self.logs), expected)

    def test_malformed_inputs(self):
        raw = struct.pack('>512i', *self.logs)
        self.assertEqual(read_log_table(raw), self.logs)
        for data in [b'', raw[:-1], raw + b'\0']:
            with self.assertRaises(ValueError):
                read_log_table(data)
        for distance, phase in [(float('nan'), 0), (float('inf'), 0), (1e40, 0),
                                 (0, -1), (0, 65536)]:
            with self.assertRaises(ValueError):
                self.node(distance, phase)
        with self.assertRaises(ValueError):
            evaluate_node(0, 0, [(1, 1)] * 4, 0, self.curves, self.logs)
        for level, table in [(-1, self.logs), (32768, self.logs), (0, self.logs[:-1]),
                              (0, [602] * 512)]:
            with self.assertRaises(ValueError):
                linear_to_log(level, table)


if __name__ == '__main__':
    unittest.main()
