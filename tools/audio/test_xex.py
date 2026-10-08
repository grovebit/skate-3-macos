import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from tools.owned_game import xex


class XexTests(unittest.TestCase):
    def test_load_checks_the_executable_and_its_mapped_image(self):
        executable, image = b'synthetic executable', b'synthetic image'
        with tempfile.TemporaryDirectory() as temp, \
                mock.patch.object(xex, 'XEX_SHA256', hashlib.sha256(executable).hexdigest()), \
                mock.patch.object(xex, 'IMAGE_SHA256', hashlib.sha256(image).hexdigest()), \
                mock.patch.object(xex, 'extract', return_value=image) as extract:
            game = Path(temp)
            (game / 'default.xex').write_bytes(executable)
            self.assertEqual(xex.load(game), image)
            extract.assert_called_once_with(executable)
            extract.return_value = b'other image'
            with self.assertRaisesRegex(ValueError, 'did not decode'):
                xex.load(game)
            (game / 'default.xex').write_bytes(b'other executable')
            with self.assertRaisesRegex(ValueError, 'not the verified'):
                xex.load(game)

    def test_tables_are_sliced_at_mapped_addresses_and_hash_checked(self):
        image = bytes(range(16))
        blob = image[4:8]
        digest = hashlib.sha256(blob).hexdigest()
        self.assertEqual(xex.table(image, 'test', xex.IMAGE_BASE + 4, 4, digest), blob)
        for address in (xex.IMAGE_BASE + 5, xex.IMAGE_BASE + 14, xex.IMAGE_BASE - 4):
            with self.assertRaisesRegex(ValueError, 'test table'):
                xex.table(image, 'test', address, 4, digest)


if __name__ == '__main__':
    unittest.main()
