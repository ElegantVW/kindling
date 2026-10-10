#!/usr/bin/env python3
"""Boot a held hall, r — prove recast from the keeper menu (-no-reboot).

Needs kindling-ingle-keeper.img after check-ingle-keeper (gil is listed).
"""
from __future__ import annotations

import os
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QEMU = os.environ.get("QEMU", "qemu-system-x86_64")
FW = ROOT / "fw" / "cerne-fw.bin"
IMG = Path(os.environ.get("INGLE_KEEPER_IMG", ROOT / "kindling-ingle-keeper.img"))
MON = f"/tmp/kindling-ingle-hall-cast-{os.getpid()}.mon"
SER = f"/tmp/kindling-ingle-hall-cast-{os.getpid()}.ser"
CPU = "qemu64,+aes,+rdrand"


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


def main() -> int:
    for p in (MON, SER):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    if not FW.is_file() or not IMG.is_file():
        print("FAIL ingle-hall-cast (missing fw or image)", file=sys.stderr)
        return 1
    proc = subprocess.Popen(
        [
            QEMU,
            "-M",
            "pc",
            "-cpu",
            CPU,
            "-m",
            "256M",
            "-bios",
            str(FW),
            "-drive",
            f"if=ide,format=raw,file={IMG}",
            "-display",
            "none",
            "-serial",
            f"file:{SER}",
            "-no-reboot",
            "-monitor",
            f"unix:{MON},server,nowait",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        serial = serial_has("who keeps this fire", 8)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            print("FAIL ingle-hall-cast (no hall)")
            print(serial[-400:])
            return 1
        if "1 gil" not in t:
            print("FAIL ingle-hall-cast (hall has no keeper; run ingle-keeper first)")
            print(serial[-400:])
            return 1
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL ingle-hall-cast (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL ingle-hall-cast (no monitor prompt)")
            print(banner)
            return 1
        sock.sendall(b"sendkey r\n")
        read_until(sock, "(qemu)", 2)
        sock.close()
        serial = serial_has("the fire is recast", 4)
        t = serial.replace("\r", "")
        if "the fire is recast" not in t:
            print("FAIL ingle-hall-cast (no recast)")
            print(serial[-400:])
            return 1
        try:
            proc.wait(timeout=4)
        except subprocess.TimeoutExpired:
            print("FAIL ingle-hall-cast (qemu still running)")
            print(serial[-400:])
            return 1
        print("ok   ingle-hall-cast")
        return 0
    finally:
        proc.kill()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.send_signal(9)
        for p in (MON, SER):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
