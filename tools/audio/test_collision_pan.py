import struct
import unittest

from tools.audio.test_impact_audio import control_bank
from tools.owned_game.splc import event_pan_choices, event_layers
from tools.audio.inspect_collision_pan import inspect


class CollisionPanTests(unittest.TestCase):
    def test_duplicate_samples_keep_distinct_pan_choices_and_group_order(self):
        data = control_bank()
        # Leaf 0's second layer has two choices, starting at 300 and 372.
        for offset, angle in [(300, 40.), (372, -40.)]:
            struct.pack_into('>H', data, offset, 0)
            struct.pack_into('>ff', data, offset + 12, 1., angle)
        choices = event_pan_choices(data)
        layer = choices[0][0][1]
        self.assertEqual([(c.sample, c.offset, c.angle_degrees, c.pitch_multiplier) for c in layer],
                         [(1, 300, 40., 1.), (1, 372, -40., 1.)])
        self.assertEqual(choices[2], [choices[0][0], choices[1][0]])
        self.assertEqual(event_layers(data)[0], [[[2], [1]]])
        report = inspect(data, 2)
        self.assertEqual(report['choices'], 4)  # Shared group variants do not count twice.
        self.assertEqual(report['angle_counts'], {-40.: 1, 0.: 2, 40.: 1})
        self.assertEqual(report['variants'][0][1][1]['angle_degrees'], -40.)

    def test_inspection_rejects_nonfinite_parameters_and_outside_events(self):
        for value in [-1, 3]:
            with self.assertRaisesRegex(ValueError, 'outside'):
                inspect(control_bank(), value)
        for field in [12, 16]:
            data = control_bank()
            struct.pack_into('>f', data, 216 + field, float('nan'))
            with self.assertRaisesRegex(ValueError, 'Nonfinite'):
                inspect(data)


if __name__ == '__main__':
    unittest.main()
