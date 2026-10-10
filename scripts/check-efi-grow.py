#!/usr/bin/env python3
"""EFI grow: ingle kindles bud after the fire; a 10-byte page becomes 600.

Happy BOOTX64. CAIRN has ingle + gleam + bud + pane + leaf. FAT32 at LBA 2048 is
KINDLING `85C7-AA81` with LFN `page` still wax (10). WRITE(10) is refused
on any other serial. Boot1 greps `the leaf grew` and the dirent is 600.
Boot2 greps `kept` and never `the leaf grew`. Pane takes a `q` before leaf.
Does not touch the live Databar.
"""
from __future__ import annotations

import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QEMU = os.environ.get("QEMU", "qemu-system-x86_64")
ESP = Path(f"/tmp/kindling-efi-grow-esp-{os.getpid()}")
EFI_SRC = ROOT / "kernel" / "BOOTX64.EFI"
EFI = ESP / "EFI" / "BOOT" / "BOOTX64.EFI"
CAIRN = ESP / "EFI" / "BOOT" / "CAIRN"
MON = f"/tmp/kindling-efi-grow-{os.getpid()}.mon"
SER = f"/tmp/kindling-efi-grow-{os.getpid()}.ser"
VARS = f"/tmp/kindling-efi-grow-{os.getpid()}.vars"
ERR = f"/tmp/kindling-efi-grow-{os.getpid()}.err"
IMG = f"/tmp/kindling-efi-grow-{os.getpid()}.img"

OVMF_CODE = [
    Path("/usr/share/edk2/x64/OVMF_CODE.4m.fd"),
    Path("/usr/share/edk2/x64/OVMF_CODE.fd"),
    Path("/usr/share/edk2-ovmf/x64/OVMF_CODE.fd"),
    Path("/usr/share/OVMF/OVMF_CODE.fd"),
]
OVMF_VARS = [
    Path("/usr/share/edk2/x64/OVMF_VARS.4m.fd"),
    Path("/usr/share/edk2/x64/OVMF_VARS.fd"),
    Path("/usr/share/edk2-ovmf/x64/OVMF_VARS.fd"),
    Path("/usr/share/OVMF/OVMF_VARS.fd"),
]

WAX = b"wax waits\n"
INK_HEAD = b"a longer leaf\n"
INK_LEN = 600
PAGE_83 = b"PAGE       "
KINDLING_VOL = 0x85C7AA81
PART_LBA = 2048


def first_file(cands: list[Path]) -> Path | None:
    for p in cands:
        if p.is_file():
            return p
    return None


def short_cksum(name11: bytes) -> int:
    s = 0
    for b in name11:
        s = (((s & 1) << 7) + (s >> 1) + b) & 0xFF
    return s


def lfn_entry(long_name: str, name11: bytes) -> bytes:
    """One last-and-first LFN slot (≤13 ASCII chars)."""
    chars = [ord(c) for c in long_name] + [0]
    while len(chars) < 13:
        chars.append(0xFFFF)
    ent = bytearray(32)
    ent[0] = 0x41
    ck = short_cksum(name11)

    def put(start: int, chunk: list[int]) -> None:
        i = 0
        for ch in chunk:
            ent[start + i] = ch & 0xFF
            ent[start + i + 1] = (ch >> 8) & 0xFF
            i += 2

    put(1, chars[0:5])
    ent[11] = 0x0F
    ent[13] = ck
    put(14, chars[5:11])
    put(28, chars[11:13])
    return bytes(ent)


def plant_fat32(img: bytearray, part_lba: int = PART_LBA, part_secs: int = 2048) -> None:
    """Tiny FAT32: reserved 32, 2×FATSz32 16, root cluster 2, LFN page in cluster 3."""
    fat = part_lba * 512
    img[fat : fat + 3] = b"\xeb\x58\x90"
    img[fat + 3 : fat + 11] = b"MSDOS5.0"
    img[fat + 11 : fat + 13] = (512).to_bytes(2, "little")
    img[fat + 13] = 1
    img[fat + 14 : fat + 16] = (32).to_bytes(2, "little")
    img[fat + 16] = 2
    img[fat + 17 : fat + 19] = (0).to_bytes(2, "little")
    img[fat + 19 : fat + 21] = (0).to_bytes(2, "little")
    img[fat + 21] = 0xF8
    img[fat + 22 : fat + 24] = (0).to_bytes(2, "little")
    img[fat + 24 : fat + 26] = (32).to_bytes(2, "little")
    img[fat + 26 : fat + 28] = (2).to_bytes(2, "little")
    img[fat + 28 : fat + 32] = part_lba.to_bytes(4, "little")
    img[fat + 32 : fat + 36] = part_secs.to_bytes(4, "little")
    img[fat + 36 : fat + 40] = (16).to_bytes(4, "little")
    img[fat + 44 : fat + 48] = (2).to_bytes(4, "little")
    img[fat + 48 : fat + 50] = (1).to_bytes(2, "little")
    img[fat + 50 : fat + 52] = (6).to_bytes(2, "little")
    img[fat + 66] = 0x29
    img[fat + 67 : fat + 71] = KINDLING_VOL.to_bytes(4, "little")
    img[fat + 71 : fat + 82] = b"KINDLING   "
    img[fat + 82 : fat + 90] = b"FAT32   "
    img[fat + 510] = 0x55
    img[fat + 511] = 0xAA
    fat1 = fat + 32 * 512
    for clus, val in ((0, 0x0FFFFFF8), (1, 0x0FFFFFFF), (2, 0x0FFFFFFF), (3, 0x0FFFFFFF)):
        off = fat1 + clus * 4
        img[off : off + 4] = (val & 0x0FFFFFFF).to_bytes(4, "little")
    fat2 = fat + (32 + 16) * 512
    img[fat2 : fat2 + 16 * 512] = img[fat1 : fat1 + 16 * 512]
    data = fat + 64 * 512
    img[data : data + 32] = lfn_entry("page", PAGE_83)
    img[data + 32 : data + 43] = PAGE_83
    img[data + 43] = 0x20
    img[data + 58 : data + 60] = (3).to_bytes(2, "little")
    img[data + 60 : data + 64] = len(WAX).to_bytes(4, "little")
    img[data + 512 : data + 512 + len(WAX)] = WAX


def page_dirent_size(img: bytes) -> int:
    data = PART_LBA * 512 + 64 * 512
    return int.from_bytes(img[data + 60 : data + 64], "little")


def page_head(img: bytes) -> bytes:
    return img[PART_LBA * 512 + 64 * 512 + 512 : PART_LBA * 512 + 64 * 512 + 512 + len(INK_HEAD)]


def read_until(sock: socket.socket, mark: str, timeout: float) -> str:
    buf = ""
    end = time.time() + timeout
    sock.settimeout(0.2)
    while time.time() < end:
        try:
            chunk = sock.recv(4096)
        except TimeoutError:
            chunk = b""
        except socket.timeout:
            chunk = b""
        if chunk:
            buf += chunk.decode(errors="replace")
        if mark in buf:
            return buf
    return buf


def serial_has(mark: str, timeout: float) -> str:
    end = time.time() + timeout
    text = ""
    while time.time() < end:
        if Path(SER).is_file():
            text = Path(SER).read_bytes().decode(errors="replace").replace("\r", "")
            if mark in text:
                return text
        time.sleep(0.05)
    return text


def boot_fire(code: Path, want: str) -> tuple[int, str]:
    for p in (SER, MON, ERR):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    errf = open(ERR, "ab")
    proc = subprocess.Popen(
        [
            QEMU,
            "-M",
            "q35",
            "-m",
            "256M",
            "-display",
            "none",
            "-serial",
            f"file:{SER}",
            "-no-reboot",
            "-no-shutdown",
            "-drive",
            f"if=pflash,format=raw,unit=0,readonly=on,file={code}",
            "-drive",
            f"if=pflash,format=raw,unit=1,file={VARS}",
            "-drive",
            f"format=raw,file=fat:rw:{ESP}",
            "-drive",
            f"if=none,id=kindstick,format=raw,cache=writethrough,file={IMG}",
            "-device",
            "qemu-xhci,id=xhci",
            "-device",
            "usb-storage,bus=xhci.0,port=1,drive=kindstick",
            "-monitor",
            f"unix:{MON},server,nowait",
        ],
        stdout=subprocess.DEVNULL,
        stderr=errf,
        cwd=ROOT,
        start_new_session=True,
    )
    try:
        serial = serial_has("kindling: ingle ok\ningle\n", 20)
        t = serial.replace("\r", "")
        if "kindling: ingle ok\ningle\n" not in t:
            print("FAIL efi-grow (spark never wrote its name)")
            print(serial[-600:])
            return 1, t
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL efi-grow (no monitor socket)")
            return 1, t
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL efi-grow (no monitor prompt)")
            print(banner)
            sock.close()
            return 1, t
        sock.sendall(b"sendkey ret\n")
        read_until(sock, "(qemu)", 2)
        serial = serial_has(want, 25)
        t = serial.replace("\r", "")
        if want not in t:
            sock.close()
            return 1, t
        if "the pane is open" not in t:
            serial = serial_has("the pane is open", 8)
            t = serial.replace("\r", "")
        if "the pane is open" not in t:
            sock.close()
            return 1, t
        if "the spark went out" in t:
            sock.close()
            return 1, t
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        if "the pane is shut" not in t:
            serial = serial_has("the pane is shut", 8)
            t = serial.replace("\r", "")
        if "the pane is shut" not in t or "the spark went out" in t:
            sock.close()
            return 1, t
        if "the volume speaks" not in t:
            serial = serial_has("the volume speaks", 8)
            t = serial.replace("\r", "")
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        serial = serial_has("the light remains", 8)
        t = serial.replace("\r", "")
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        sock.close()
        serial = serial_has("kindling: gleam exit 0", 8)
        return 0, serial.replace("\r", "")
    finally:
        errf.close()
        try:
            os.killpg(proc.pid, 9)
        except OSError:
            proc.kill()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.send_signal(9)


def main() -> int:
    for p in (MON, SER, VARS, ERR, IMG):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    code = first_file(OVMF_CODE)
    vars_src = first_file(OVMF_VARS)
    if code is None or vars_src is None:
        print("FAIL efi-grow (no OVMF)", file=sys.stderr)
        return 1
    if not EFI_SRC.is_file():
        print("FAIL efi-grow (missing BOOTX64.EFI)", file=sys.stderr)
        return 1
    ld = ROOT / "ld" / "cerne-ld.bin"
    kern = ROOT / "kernel" / "kernel.fw.bin"
    if not ld.is_file() or not kern.is_file():
        print("FAIL efi-grow (missing loader or kernel)", file=sys.stderr)
        return 1
    subprocess.check_call(
        ["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "ingle.bin"), str(ROOT / "spark" / "ingle.asm")],
        cwd=ROOT,
    )
    subprocess.check_call(
        ["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "bud.bin"), str(ROOT / "spark" / "bud.asm")],
        cwd=ROOT,
    )
    subprocess.check_call(
        ["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "leaf.bin"), str(ROOT / "spark" / "leaf.asm")],
        cwd=ROOT,
    )
    subprocess.check_call(["make", "spark/gleam.elf"], cwd=ROOT)
    subprocess.check_call(["make", "spark/pane.elf"], cwd=ROOT)
    cairn_blob = ROOT / "spark" / "cairn-grow.bin"
    subprocess.check_call(
        [
            "python3",
            str(ROOT / "scripts" / "mkimg.py"),
            "--loader",
            str(ld),
            "--kernel",
            str(kern),
            "--out",
            str(ROOT / "kindling-efi-grow-pack.img"),
            "--spark",
            f"ingle={ROOT / 'spark' / 'ingle.bin'}",
            "--spark",
            f"gleam={ROOT / 'spark' / 'gleam.elf'}",
            "--spark",
            f"bud={ROOT / 'spark' / 'bud.bin'}",
            "--spark",
            f"pane={ROOT / 'spark' / 'pane.elf'}",
            "--spark",
            f"leaf={ROOT / 'spark' / 'leaf.bin'}",
            "--leaf",
            f"LEAF={ROOT / 'spark' / 'LEAF.txt'}",
            "--cairn-out",
            str(cairn_blob),
        ],
        cwd=ROOT,
    )
    ESP.joinpath("EFI/BOOT").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(EFI_SRC, EFI)
    shutil.copyfile(cairn_blob, CAIRN)
    img = bytearray(2 * 1024 * 1024)
    img[510] = 0x55
    img[511] = 0xAA
    img[446 + 4] = 0x0C
    img[446 + 8 : 446 + 12] = PART_LBA.to_bytes(4, "little")
    img[446 + 12 : 446 + 16] = (2048).to_bytes(4, "little")
    plant_fat32(img)
    fat = PART_LBA * 512
    if img[fat + 82 : fat + 87] != b"FAT32" or img[fat + 510] != 0x55:
        print("FAIL efi-grow (planted boot sector is not FAT32)", file=sys.stderr)
        return 1
    if int.from_bytes(img[fat + 67 : fat + 71], "little") != KINDLING_VOL:
        print("FAIL efi-grow (planted volume serial is not KINDLING)", file=sys.stderr)
        return 1
    data = fat + 64 * 512
    if img[data + 32 : data + 43] != PAGE_83 or img[data + 512 : data + 512 + len(WAX)] != WAX:
        print("FAIL efi-grow (planted page is missing)", file=sys.stderr)
        return 1
    Path(IMG).write_bytes(img)
    shutil.copyfile(vars_src, VARS)

    rc, serial = boot_fire(code, "the leaf grew")
    if rc != 0:
        print("FAIL efi-grow (no the leaf grew on first boot)")
        print(serial[-800:])
        return 1
    t = serial
    if "kindling: fat" not in t:
        print("FAIL efi-grow (no fat)")
        print(serial[-800:])
        return 1
    if "the rust is lit" not in t:
        print("FAIL efi-grow (gleam never wrote the rust)")
        print(serial[-800:])
        return 1
    if "the leaf grew" not in t:
        print("FAIL efi-grow (no the leaf grew on first boot)")
        print(serial[-800:])
        return 1
    if "kept" in t:
        print("FAIL efi-grow (kept on first boot — grow never ran)")
        print(serial[-800:])
        return 1
    if "kindling: gleam exit 0" not in t:
        print("FAIL efi-grow (no gleam exit 0 on first boot)")
        print(serial[-800:])
        return 1
    landed = Path(IMG).read_bytes()
    if page_dirent_size(landed) != INK_LEN:
        print("FAIL efi-grow (dirent size is not 600)")
        return 1
    if page_head(landed) != INK_HEAD:
        print("FAIL efi-grow (WRITE(10) never landed the longer leaf)")
        return 1

    rc, serial = boot_fire(code, "kept")
    if rc != 0:
        print("FAIL efi-grow (no kept on second boot)")
        print(serial[-800:])
        return 1
    t = serial
    if "kept" not in t:
        print("FAIL efi-grow (no kept on second boot)")
        print(serial[-800:])
        return 1
    if "the leaf grew" in t:
        print("FAIL efi-grow (re-grew on second boot — ink never landed)")
        print(serial[-800:])
        return 1
    if "kindling: gleam exit 0" not in t:
        print("FAIL efi-grow (no gleam exit 0 on second boot)")
        print(serial[-800:])
        return 1
    print("ok   efi-grow")
    return 0


if __name__ == "__main__":
    try:
        rc = main()
    finally:
        for p in (MON, SER, VARS, ERR, IMG):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass
        try:
            os.unlink(ROOT / "kindling-efi-grow-pack.img")
        except FileNotFoundError:
            pass
        shutil.rmtree(ESP, ignore_errors=True)
    sys.exit(rc)
