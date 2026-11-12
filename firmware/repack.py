"""Build a COPY of the official updater carrying a patched image.

Usage: python repack.py <patched.bin> <out.exe>

Replaces RCDATA 4000 (the image) and RCDATA 4014 (its checksum, UTF-16
hex text) in a copy of the stock updater through the Windows resource API,
then reads both back to confirm. The stock exe is never modified, and this
script never runs anything.
"""
import ctypes
import shutil
import struct
import sys
from ctypes import wintypes

STOCK_EXE = r"C:\Users\melih\AppData\Local\Temp\claude\ac119611ae06eb26\58eafb19-ce45-4c22-b819-5a1521b446c9\scratchpad\fw-extract"
RT_RCDATA = 10
k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.BeginUpdateResourceW.restype = wintypes.HANDLE
k32.BeginUpdateResourceW.argtypes = [wintypes.LPCWSTR, wintypes.BOOL]
k32.UpdateResourceW.argtypes = [wintypes.HANDLE, ctypes.c_void_p, ctypes.c_void_p,
                                wintypes.WORD, ctypes.c_void_p, wintypes.DWORD]
k32.EndUpdateResourceW.argtypes = [wintypes.HANDLE, wintypes.BOOL]
k32.LoadLibraryExW.restype = wintypes.HMODULE
k32.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
k32.FindResourceW.restype = ctypes.c_void_p
k32.FindResourceW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p]
k32.SizeofResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
k32.LoadResource.restype = ctypes.c_void_p
k32.LoadResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
k32.LockResource.restype = ctypes.c_void_p
k32.LockResource.argtypes = [ctypes.c_void_p]
k32.FreeLibrary.argtypes = [wintypes.HMODULE]


def checksum(img: bytes) -> int:
    return sum(struct.unpack_from("<%dH" % (len(img) // 2), img)) & 0xFFFF


def update(path: str, items: dict) -> None:
    h = k32.BeginUpdateResourceW(path, False)
    if not h:
        raise ctypes.WinError(ctypes.get_last_error())
    for rid, data in items.items():
        buf = ctypes.create_string_buffer(data, len(data))
        if not k32.UpdateResourceW(h, RT_RCDATA, rid, 0, buf, len(data)):
            raise ctypes.WinError(ctypes.get_last_error())
    if not k32.EndUpdateResourceW(h, False):
        raise ctypes.WinError(ctypes.get_last_error())


def read(path: str, rid: int) -> bytes:
    mod = k32.LoadLibraryExW(path, None, 0x2 | 0x20)  # datafile + image resource
    try:
        r = k32.FindResourceW(mod, rid, RT_RCDATA)
        n = k32.SizeofResource(mod, r)
        return ctypes.string_at(k32.LockResource(k32.LoadResource(mod, r)), n)
    finally:
        k32.FreeLibrary(mod)


def main() -> None:
    import glob
    img = open(sys.argv[1], "rb").read()
    out = sys.argv[2]
    src = glob.glob(STOCK_EXE + r"\*.exe")[0]
    assert len(img) == 0x7E000 and img[-4:] == b"\x55\x55\xaa\xaa"
    cs = f"{checksum(img):04X}".encode("utf-16le")
    shutil.copyfile(src, out)
    update(out, {4000: img, 4014: cs})
    assert read(out, 4000) == img and read(out, 4014) == cs
    assert read(src, 4014) == "67E7".encode("utf-16le")  # stock untouched
    print(f"built {out}; image checksum {cs.decode('utf-16le')}; NOT run")


if __name__ == "__main__":
    main()
