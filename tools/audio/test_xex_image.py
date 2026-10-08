import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from tools.native_replay import xex_image


class XexImageTests(unittest.TestCase):
    def test_load_checks_the_executable_and_its_mapped_image(self):
        xex, image = b'synthetic executable', b'synthetic image'
        with tempfile.TemporaryDirectory() as temp, \
                mock.patch.object(xex_image, 'XEX_SHA256', hashlib.sha256(xex).hexdigest()), \
                mock.patch.object(xex_image, 'IMAGE_SHA256', hashlib.sha256(image).hexdigest()), \
                mock.patch.object(xex_image, 'extract', return_value=image) as extract:
            game = Path(temp)
            (game / 'default.xex').write_bytes(xex)
            self.assertEqual(xex_image.load(game), image)
            extract.assert_called_once_with(xex)
            extract.return_value = b'other image'
            with self.assertRaisesRegex(ValueError, 'did not decode'):
                xex_image.load(game)
            (game / 'default.xex').write_bytes(b'other executable')
            with self.assertRaisesRegex(ValueError, 'not the verified'):
                xex_image.load(game)


if __name__ == '__main__':
    unittest.main()
