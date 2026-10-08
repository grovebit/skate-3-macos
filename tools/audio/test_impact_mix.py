from pathlib import Path
import struct
import tempfile
import unittest
import wave

from tools.owned_game.impact_mix import render


def write(path, samples, rate=48000):
    with wave.open(str(path), 'wb') as wav:
        wav.setparams((1, 2, rate, len(samples), 'NONE', 'not compressed'))
        wav.writeframes(struct.pack('<' + 'h' * len(samples), *samples))


def read(path):
    with wave.open(str(path), 'rb') as wav:
        return struct.unpack('<' + 'h' * wav.getnframes(), wav.readframes(wav.getnframes()))


class ImpactMixTests(unittest.TestCase):
    def test_layers_sum_without_clipping_and_preserve_the_longest_tail(self):
        with tempfile.TemporaryDirectory() as temp:
            a, b, out = [Path(temp) / name for name in ['a.wav', 'b.wav', 'mix.wav']]
            write(a, [30000, 30000])
            write(b, [30000, -30000, 10000])
            info = render([a, b], out)
            self.assertEqual(read(out), (32767, 0, 5461))
            self.assertEqual(info['layer_count'], 2)
            self.assertEqual(info['frames'], 3)

    def test_single_layer_preserves_pcm_including_negative_full_scale(self):
        with tempfile.TemporaryDirectory() as temp:
            a, out = Path(temp) / 'a.wav', Path(temp) / 'out.wav'
            values = [-32768, -1000, 0, 1000, 32767]
            write(a, values)
            self.assertEqual(render([a], out)['gain'], 1)
            self.assertEqual(read(out), tuple(values))

    def test_attack_trim_attenuates_loud_clips_without_boosting_quiet_ones(self):
        with tempfile.TemporaryDirectory() as temp:
            source, out = Path(temp) / 'source.wav', Path(temp) / 'out.wav'
            write(source, [24000] * 2400 + [0] * 24000)
            loud = render([source], out)
            self.assertAlmostEqual(loud['attack_rms'] * loud['playback_trim'], 0.18)
            # Adding a long quiet tail must not change the attack measurement.
            write(source, [24000] * 2400)
            self.assertAlmostEqual(render([source], out)['playback_trim'], loud['playback_trim'])
            write(source, [1000] * 2400)
            self.assertEqual(render([source], out)['playback_trim'], 1.)

    def test_rejects_rate_mismatch_truncation_and_unbounded_layers(self):
        with tempfile.TemporaryDirectory() as temp:
            a, b, out = [Path(temp) / name for name in ['a.wav', 'b.wav', 'mix.wav']]
            write(a, [10, 20])
            write(b, [10, 20], 44100)
            for sources in [[], [a] * 9, [a, b]]:
                with self.assertRaises(ValueError):
                    render(sources, out)
            a.write_bytes(a.read_bytes()[:-2])
            with self.assertRaises(ValueError):
                render([a], out)
            self.assertFalse(out.exists())


if __name__ == '__main__':
    unittest.main()
