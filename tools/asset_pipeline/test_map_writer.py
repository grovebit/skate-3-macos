import io
import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

import numpy as np
from PIL import Image
from .map_writer import write, write_textures


class MapWriterTests(unittest.TestCase):
    def test_parallel_textures_match_serial_format_exactly(self):
        with tempfile.TemporaryDirectory() as work:
            root=Path(work);textures={};expected=io.BytesIO()
            rng=np.random.default_rng(23)
            for name, cube, png in [('z-cube', True, False), ('a-rgba', False, False), ('m-png', False, True), ('b-flat', False, False)]:
                pixels=rng.integers(0,256,(24 if cube else 4,4,4),dtype=np.uint8)
                if name=='b-flat':pixels[:]=0
                path=root/(name+('.png' if png else '.rgba'))
                if png:Image.fromarray(pixels).save(path)
                else:path.write_bytes(pixels.tobytes())
                textures[name]=dict(width=4,height=len(pixels),cube_faces=6 if cube else 1,
                                    **{'png' if png else 'rgba':path.name})
            for name,entry in sorted(textures.items()):
                if 'png' in entry:
                    with Image.open(root/entry['png']) as image:pixels=np.array(image.convert('RGBA'))
                else:pixels=np.frombuffer((root/entry['rgba']).read_bytes(),dtype=np.uint8).reshape(entry['height'],4,4)
                raw=(pixels if entry['cube_faces']==6 else pixels[::-1]).tobytes()
                packed=zlib.compress(raw,1);method=1
                if len(packed)>=len(raw):packed=raw;method=0
                encoded=name.encode()
                expected.write(struct.pack('<I',len(encoded))+encoded)
                expected.write(struct.pack('<5I',4,entry['height'],1,method,len(packed))+packed)
            actual=io.BytesIO();write_textures(actual,root,textures)
            self.assertEqual(actual.getvalue(),expected.getvalue())
            empty=io.BytesIO();write_textures(empty,root,{})
            self.assertEqual(empty.getvalue(),b'')

    def test_texture_failure_is_propagated(self):
        with tempfile.TemporaryDirectory() as work:
            with self.assertRaises(FileNotFoundError):
                write_textures(io.BytesIO(),Path(work),{'missing':dict(rgba='missing',width=4,height=4)})

    def test_header_carries_the_authored_spawn_and_heading(self):
        with tempfile.TemporaryDirectory() as work:
            root=Path(work)
            manifest=root/'manifest.json';collision=root/'collision.rwcmset'
            manifest.write_text(json.dumps(dict(map_name='Park',textures={},models=[],grind_splines=[],
                normal_texture_policy=dict(excluded_texture_ids=[]),other_presentation_assets=[])))
            collision.write_bytes(b'RWCM')
            def header(path):
                data=path.read_bytes();self.assertEqual(data[:12],b'SKATE14\0'+struct.pack('<I',0x12345678))
                self.assertEqual(data[12:20],struct.pack('<I',4)+b'Park')
                return struct.unpack_from('<4f',data,20)
            write(manifest,root/'park.skate',collision,spawn=(-64.25,0.75,0.),heading=1.5)
            self.assertEqual(header(root/'park.skate'),(-64.25,0.75,0.,1.5))
            write(manifest,root/'backdrop.skate',None,render_only=True,spawn=(1.,2.,3.),heading=1.5)
            self.assertEqual(header(root/'backdrop.skate'),(0.,0.,0.,0.))
            with self.assertRaisesRegex(ValueError,'requires its authored spawn'):
                write(manifest,root/'missing.skate',collision)


if __name__=='__main__':unittest.main()
