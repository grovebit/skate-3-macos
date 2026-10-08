import json
import struct
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import wave

from tools.audio import prepare_skating_audio as audio
from tools.owned_game.big import BigEntry
from tools.owned_game.splc import streams


def pcm(path, frames=16):
    with wave.open(str(path), 'wb') as wav:
        wav.setparams((1, 2, 48000, frames, 'NONE', 'not compressed'))
        wav.writeframes(b'\0\0' * frames)


INFO = {'version': 'test', 'encoding': 'PCM', 'sampleRate': 48000,
        'channels': 1, 'numberOfSamples': 16, 'streamInfo': {'total': 2},
        'loopingInfo': {'start': 0, 'end': 16}}


class SplcTests(unittest.TestCase):
    @staticmethod
    def bank():
        header = bytearray(0x3c)
        header[:8] = b'SPLC\0\0\0\3'
        struct.pack_into('>I', header, 0x18, 2)
        sample = b'\x03\x00\xbb\x80' + bytes(28)
        table = struct.pack('>IIIIII', 0, 24, 123, 32, 56, 456)
        return bytes(header) + table + sample + sample

    def test_reads_full_embedded_streams_and_preserves_hashes(self):
        data = self.bank()
        clips = streams(data)
        self.assertEqual([(s.offset, s.size, s.source_hash) for s in clips],
                         [(84, 32, 123), (116, 32, 456)])

    def test_rejects_truncated_bad_codec_and_overlapping_records(self):
        original = self.bank()
        bad_codec = bytearray(original); bad_codec[84] = 0xff
        backwards = bytearray(original); struct.pack_into('>I', backwards, 0x3c + 12, 8)
        too_many = bytearray(original); struct.pack_into('>I', too_many, 0x18, 0xffffffff)
        for data in [b'', original[:50], original[:-25], bad_codec, backwards, too_many]:
            with self.assertRaises(ValueError): streams(data)


class AudioPreparationTests(unittest.TestCase):
    def test_rejects_truncated_or_mismatched_wav(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'sound.wav'
            pcm(path)
            self.assertEqual(audio.validate_wav(path, INFO)['frames'], 16)
            with self.assertRaises(ValueError):
                audio.validate_wav(path, dict(INFO, sampleRate=44100))
            path.write_bytes(path.read_bytes()[:-2])
            with self.assertRaises(ValueError):
                audio.validate_wav(path, INFO)
            for invalid in (b'', b'not a wave file'):
                path.write_bytes(invalid)
                with self.assertRaisesRegex(ValueError, 'invalid WAV'):
                    audio.validate_wav(path, INFO)

    def test_publishes_complete_library_and_preserves_existing_output(self):
        self.run_preparation(fail=False)

    def test_decoder_failure_does_not_publish_partial_library(self):
        self.run_preparation(fail=True)

    def run_preparation(self, fail):
        with tempfile.TemporaryDirectory() as temp:
            game = Path(temp) / 'game'
            game.mkdir()
            (game / 'default.xex').touch()
            output = Path(temp) / 'library'
            entry = BigEntry(0, 'data/audio/GRINDS.abk', 0, 4, 4, 0)

            def decode(decoder, source, index=1, output=None):
                if output is not None:
                    if fail and index == 2:
                        raise ValueError('Bad stream')
                    pcm(output)
                return INFO

            with patch.object(audio, 'BigArchive') as archive, patch.object(audio, 'decoder_info', decode):
                archive.return_value.entries = [entry]
                archive.return_value.read.return_value = b'bank'
                if fail:
                    with self.assertRaisesRegex(ValueError, 'Bad stream'):
                        audio.prepare(game, output, 'decoder', banks=('GRINDS.abk',))
                    self.assertFalse(output.exists())
                else:
                    manifest = audio.prepare(game, output, 'decoder', banks=('GRINDS.abk',))
                    self.assertEqual(len(manifest['clips']), 2)
                    self.assertEqual(json.loads((output / 'manifest.json').read_text()), manifest)
                    self.assertEqual(manifest['clips'][1]['subsong'], 2)
                    self.assertEqual(manifest['clips'][0]['loop'], INFO['loopingInfo'])
                    self.assertTrue((output / manifest['clips'][0]['file']).is_file())
                    self.assertTrue((output / 'index.html').is_file())
                    with self.assertRaises(FileExistsError):
                        audio.prepare(game, output, 'decoder', banks=('GRINDS.abk',))
                self.assertEqual(list(Path(temp).glob('.skating-audio-*')), [])

    def test_audition_page_escapes_source_labels(self):
        page = audio.audition_page([{'bank': '<bank>', 'subsong': 1, 'seconds': 1,
                                    'loop': None, 'file': 'clips/0001.wav'}])
        self.assertIn('&lt;bank&gt;', page)
        self.assertNotIn('<bank>', page)
        self.assertIn('preload="none"', page)


if __name__ == '__main__':
    unittest.main()
