"""Unicorn harness for the AK820 MAX Ultra firmware. Nothing here touches hardware.

    venv\\Scripts\\python emu.py <image.bin> [hexframe ...]

1. Boots from the reset vector until main() (0x14bcc) is reached, so the C
   runtime has initialised RAM the way the real chip would.
2. For each frame given, copies it into the 2.4 GHz receive buffer, raises
   the "frame pending" flag and calls the dispatcher FUN_0000ea90 directly.
3. Logs every call to the flash writer (FUN_00005cb8) and stubs it out, so
   "does command X write flash?" gets a definite answer.
"""
import struct
import sys

from unicorn import (UC_ARCH_ARM, UC_HOOK_CODE, UC_HOOK_MEM_UNMAPPED, UC_MODE_MCLASS,
                     UC_MODE_THUMB, Uc, UcError)
from unicorn.arm_const import (UC_ARM_REG_LR, UC_ARM_REG_PC, UC_ARM_REG_R0, UC_ARM_REG_R1,
                               UC_ARM_REG_R2, UC_ARM_REG_SP)

MAIN = 0x14BCC
DISPATCH = 0xEA90
FLASH_WRITE = 0x5CB8
RX_BUF = 0x200052CC        # 2.4 GHz frame buffer (pool value in the dispatcher)
PENDING = 0x2000022B + 4   # "frame pending" byte
STOP = 0x7FFF0             # fake return address inside unused flash


def build(img: bytes) -> Uc:
    uc = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
    uc.mem_map(0x0, 0x80000)
    uc.mem_write(0, img)
    uc.mem_map(0x20000000, 0x10000)          # SRAM
    uc.mem_map(0x40000000, 0x100000)         # APB peripherals: plain RAM, reads 0
    uc.mem_map(0x50000000, 0x100000)
    uc.mem_map(0xE000E000, 0x1000)           # SCS
    uc.mem_write(STOP, b"\x00\xbf\xfe\xe7")  # nop; b .
    return uc


def boot(uc: Uc, img: bytes) -> None:
    sp, reset = struct.unpack_from("<II", img, 0)
    uc.reg_write(UC_ARM_REG_SP, sp)
    seen = {"n": 0}

    def guard(u, addr, size, _):
        seen["n"] += 1
        if addr == MAIN:
            u.emu_stop()

    h = uc.hook_add(UC_HOOK_CODE, guard)
    uc.emu_start(reset | 1, 0, count=5_000_000)
    uc.hook_del(h)
    pc = uc.reg_read(UC_ARM_REG_PC)
    print(f"boot: stopped at {pc:#x} after {seen['n']} instructions "
          f"({'reached main' if pc == MAIN else 'did NOT reach main'})")


WIRED_DISPATCH = 0xF940
WIRED_RAW = 0x20003BFC      # raw report; the handler copies from +1
WIRED_REPLY = 0x20003CBC
HARDFAULT = 0x5880
WATCH = {0x6418: "settings writer 0x6418", 0x1984: "stock apply 0x1984",
         0x7C84: "render setup 0x7C84", 0xE528: "render setup 0xE528",
         0xAF8C: "render setup 0xAF8C"}


def run_frame(uc: Uc, frame: bytes, wired: bool = False) -> list:
    writes = []
    calls = []
    fault = []

    def hook(u, addr, size, _):
        if addr == FLASH_WRITE:
            src, n, dst = (u.reg_read(r) for r in (UC_ARM_REG_R0, UC_ARM_REG_R1, UC_ARM_REG_R2))
            writes.append((dst, n, bytes(u.mem_read(src, min(n, 16)))))
            u.reg_write(UC_ARM_REG_PC, u.reg_read(UC_ARM_REG_LR))  # skip the real write
        elif addr in WATCH:
            calls.append(WATCH[addr])
        elif addr == HARDFAULT:
            fault.append(addr)
            u.emu_stop()

    if wired:
        uc.mem_write(WIRED_RAW, b"\0" + frame.ljust(63, b"\0"))
        entry, reply_at = WIRED_DISPATCH, WIRED_REPLY
    else:
        uc.mem_write(RX_BUF, frame.ljust(64, b"\0"))
        uc.mem_write(PENDING, b"\x01")
        entry, reply_at = DISPATCH, RX_BUF - 0x40
    uc.reg_write(UC_ARM_REG_SP, 0x20008000)
    uc.reg_write(UC_ARM_REG_LR, STOP | 1)
    h = uc.hook_add(UC_HOOK_CODE, hook)
    try:
        uc.emu_start(entry | 1, STOP, count=2_000_000)
    finally:
        uc.hook_del(h)
    pc = uc.reg_read(UC_ARM_REG_PC)
    if fault:
        print("  HARDFAULT reached")
    elif pc != STOP:
        print(f"  did not return (pc {pc:#x}): loop or instruction limit")
    reply = bytes(uc.mem_read(reply_at, 24))
    print(f"  reply {reply.hex(' ')}")
    if calls:
        print(f"  calls: {', '.join(dict.fromkeys(calls))}")
    return writes


KEY_LED = 0x15B8A


def run_frames(uc: Uc, fn: int, n: int) -> int:
    """Calls the patched frame function n times; returns flash writes seen."""
    if fn is None:
        raise SystemExit("pass --frame=ADDR before F<n>")
    count = [0]
    steps = [0]

    def hook(u, addr, size, _):
        steps[0] += 1
        if addr == FLASH_WRITE:
            count[0] += 1
            u.reg_write(UC_ARM_REG_PC, u.reg_read(UC_ARM_REG_LR))
        elif addr == HARDFAULT:
            print("  HARDFAULT in frame")
            u.emu_stop()

    h = uc.hook_add(UC_HOOK_CODE, hook)
    try:
        for _ in range(n):
            uc.reg_write(UC_ARM_REG_SP, 0x20006D00)
            uc.reg_write(UC_ARM_REG_LR, STOP | 1)
            uc.emu_start(fn | 1, STOP, count=3_000_000)
    finally:
        uc.hook_del(h)
    print(f"  ~{steps[0] // max(n, 1)} instructions per frame (incl. the stock LED push)")
    return count[0]


def leds(uc: Uc, keys=(0, 1, 2, 13, 30, 81)) -> str:
    """RGB of a few keys (key order, mapped through the firmware's LED table)."""
    table = bytes(uc.mem_read(KEY_LED, 82))
    r = bytes(uc.mem_read(0x20004984, 126))
    g = bytes(uc.mem_read(0x20004A02, 126))
    b = bytes(uc.mem_read(0x20004A80, 126))
    return " ".join(f"k{k}:{r[table[k]]:02x}{g[table[k]]:02x}{b[table[k]]:02x}" for k in keys)


def engine(uc: Uc) -> str:
    s = bytes(uc.mem_read(0x20006E00, 20))
    magic, frame, start, us = struct.unpack_from("<IIII", s)
    return f"engine magic {magic:#x} frame {frame} start {start} frame_us {us} active {s[16]} restore {s[17]}"


def state(uc: Uc) -> str:
    """The lighting variables the renderer reads."""
    r = lambda a: uc.mem_read(a, 1)[0]
    s = bytes(uc.mem_read(0x200001CC, 8))
    return (f"state {s.hex(' ')} | effect_idx {r(0x200001A1)} brightness {r(0x200001B0)} "
            f"speed {r(0x200001B2)} dir_flag {r(0x200001B4)}")


def main() -> None:
    img = open(sys.argv[1], "rb").read()
    uc = build(img)

    def unmapped(u, access, addr, size, value, _):
        print(f"  unmapped access {addr:#x} at pc {u.reg_read(UC_ARM_REG_PC):#x}")
        return False

    uc.hook_add(UC_HOOK_MEM_UNMAPPED, unmapped)
    boot(uc, img)
    total = 0
    frame_fn = None
    for arg in sys.argv[2:]:
        if arg.startswith("--frame="):
            frame_fn = int(arg.split("=", 1)[1], 0)
            continue
        if arg.startswith("F"):
            n = int(arg[1:])
            writes = run_frames(uc, frame_fn, n)
            total += writes
            print(f"after {n} frames: {leds(uc)}  | flash writes {writes}")
            print(f"  {engine(uc)}")
            continue
        wired = arg.startswith("w:")
        hx = arg[2:] if wired else arg
        print(f"frame {'(wired) ' if wired else ''}{hx}")
        try:
            for dst, n, head in run_frame(uc, bytes.fromhex(hx), wired):
                total += 1
                print(f"  FLASH WRITE -> {dst:#07x} len {n}  data {head.hex(' ')}")
        except UcError as e:
            print(f"  emulation error: {e} at pc {uc.reg_read(UC_ARM_REG_PC):#x}")
        print(f"  {state(uc)}")
    print(f"TOTAL FLASH WRITES: {total}")


if __name__ == "__main__":
    main()
