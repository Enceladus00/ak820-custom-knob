"""Apply byte patches to the stock AK820 MAX Ultra v1.36 image, safely.

Usage: python fwpatch.py <patches.json> <out.bin>

patches.json: {"patches": [{"addr": "0x17400", "hex": "...", "why": "..."}],
               "allow": [["0x17200", "0x7dff0"], ...]}

Guarantees, or it refuses to write:
* the input is the known stock image (sha256 check),
* every changed byte lies inside an "allow" range,
* the vector table (0x000-0x13f), the settings pages (0x8000-0xbfff) and
  the end marker (last 4 bytes, 55 55 AA AA) are never changed,
* the 16-bit word checksum the updater checks is recomputed and printed.
"""
import hashlib
import json
import struct
import sys

STOCK = r"C:\Users\melih\tools\ak820-fw\ak820maxultra_v1.36.bin"
STOCK_SHA256 = "58db8a73ea8be7b8"  # prefix, see firmware-image notes
SIZE = 0x7E000
PROTECTED = [(0x0, 0x140), (0x8000, 0xC000), (SIZE - 4, SIZE)]


def checksum(img: bytes) -> int:
    return sum(struct.unpack_from("<%dH" % (len(img) // 2), img)) & 0xFFFF


def main() -> None:
    spec = json.load(open(sys.argv[1]))
    out = sys.argv[2]
    stock = open(STOCK, "rb").read()
    if not hashlib.sha256(stock).hexdigest().startswith(STOCK_SHA256) or len(stock) != SIZE:
        sys.exit("refusing: input is not the known stock v1.36 image")
    assert checksum(stock) == 0x67E7 and stock[-4:] == b"\x55\x55\xaa\xaa"

    allow = [(int(a, 0), int(b, 0)) for a, b in spec["allow"]]
    img = bytearray(stock)
    for p in spec["patches"]:
        addr, data = int(p["addr"], 0), bytes.fromhex(p["hex"])
        img[addr:addr + len(data)] = data
        print(f"patch {addr:#07x} +{len(data):4d}  {p.get('why', '')}")

    changed = [i for i in range(SIZE) if img[i] != stock[i]]
    for i in changed:
        if any(a <= i < b for a, b in PROTECTED):
            sys.exit(f"refusing: byte {i:#x} is in a protected range")
        if not any(a <= i < b for a, b in allow):
            sys.exit(f"refusing: byte {i:#x} is outside the allowed ranges")
    assert len(img) == SIZE and img[-4:] == b"\x55\x55\xaa\xaa"

    cs = checksum(bytes(img))
    open(out, "wb").write(img)
    print(f"{len(changed)} bytes changed; checksum {cs:04X} (stock 67E7)")
    print(f"sha256 {hashlib.sha256(img).hexdigest()}")


if __name__ == "__main__":
    main()
