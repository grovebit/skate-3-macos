"""Optional native RefPack backend built by play.sh;
without it, callers keep the Python decoder."""
import ctypes
from pathlib import Path

_library=None
_path=Path(__file__).resolve().parents[2]/'target/native/librefpack.dylib'
if _path.is_file():
    _library=ctypes.CDLL(str(_path))
    _library.skate_refpack.argtypes=[ctypes.c_char_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_size_t,ctypes.c_bool]
    _library.skate_refpack.restype=ctypes.c_int

def decode(data,size,start,early=False):
    if _library is None:return None
    output=ctypes.create_string_buffer(size)
    if _library.skate_refpack(data,len(data),output,size,start,early):
        raise ValueError('Malformed RefPack command or declared output size')
    return output.raw
