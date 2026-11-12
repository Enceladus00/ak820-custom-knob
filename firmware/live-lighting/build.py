"""Build the live-lighting patch: compile, extract sections, verify the hook
sites, write patches.json, and run fwpatch.py. Produces live-v2.bin.

All outputs go to OUT, outside the repo: the patched image contains AJAZZ's
firmware and must not be committed. Only these sources belong here.
"""
import json
import os
import subprocess
import sys

TOOLS = r"C:\Users\melih\tools"
B = os.path.join(TOOLS, r"xpack-arm-none-eabi-gcc-15.2.1-1.1\bin\arm-none-eabi-")
STOCK_PATH = os.path.join(TOOLS, r"ak820-fw\ak820maxultra_v1.36.bin")
OUT = os.path.join(TOOLS, r"ak820-fw\live")
HERE = os.path.dirname(os.path.abspath(__file__))
ELF = os.path.join(OUT, "live.elf")

# Original bytes at the hook sites: movs rX,#6 ; cmp.w rX, rCMD, lsr #4
EXPECT = {0xED36: "0621b1eb101f", 0xFC54: "0623b3eb111f",
          0x1054E: "04f01ffb"}  # bl 0x14B90 (LED push)


def run(*args):
    subprocess.run(args, check=True, cwd=HERE)


def main():
    os.makedirs(OUT, exist_ok=True)
    stock = open(STOCK_PATH, "rb").read()
    run(B + "gcc", "-mcpu=cortex-m3", "-mthumb", "-Os", "-ffreestanding", "-nostdlib", "-fno-tree-loop-distribute-patterns",
        "-fno-common", "-Wall", "-Wextra", "-Werror", "-T", "live.ld",
        "-Wl,--no-warn-rwx-segments", "-o", ELF, "live.c", "effects.c", "hooks.S")

    patches = []
    for sec, addr in ((".hook24", 0xED36), (".hookw", 0xFC54), (".hookframe", 0x1054E), (".text", 0x17400)):
        out = os.path.join(OUT, sec.strip(".") + ".bin")
        run(B + "objcopy", "-O", "binary", "-j", sec, ELF, out)
        data = open(out, "rb").read()
        if addr in EXPECT:
            size = len(EXPECT[addr]) // 2
            got = stock[addr:addr + size].hex()
            if got != EXPECT[addr]:
                sys.exit(f"hook site {addr:#x} holds {got}, expected {EXPECT[addr]}")
            assert len(data) == size
        elif stock[addr:addr + len(data)] != bytes(len(data)):
            sys.exit("the free area is not empty")
        patches.append({"addr": hex(addr), "hex": data.hex(), "why": sec})

    spec = {"patches": patches,
            "allow": [["0xED36", "0xED3C"], ["0xFC54", "0xFC5A"], ["0x1054E", "0x10552"],
                      ["0x17400", "0x19400"]]}
    spec_path = os.path.join(OUT, "patches.json")
    json.dump(spec, open(spec_path, "w"), indent=1)
    run(sys.executable, "-I", os.path.join(HERE, "..", "fwpatch.py"),
        spec_path, os.path.join(OUT, "live-v2.bin"))


if __name__ == "__main__":
    main()
