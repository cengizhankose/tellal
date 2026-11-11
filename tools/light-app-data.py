#!/usr/bin/env python3
"""Makes a private copy of the T3 desktop app's theme settings with the appearance set to light.

Usage: tools/light-app-data.py <dest-dir>     then run the panel with T3P_T3_APP_DATA=<dest-dir>

Copies `<T3 userData>/Local Storage/leveldb` (default ~/Library/Application Support/t3code-v2,
or $T3P_T3_APP_DATA) into <dest-dir> and appends one leveldb write-ahead log whose batch sets
localStorage `t3code:theme-appearance-mode` = `light` with a sequence number above every existing
record, so it wins (crates/core/src/theme/leveldb.rs). The T3 files are only read.
"""

import os
import shutil
import struct
import sys
from pathlib import Path

ORIGIN = "t3code://app"
SEQ = 1 << 55
BLOCK = 32768


def varint(n: int) -> bytes:
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def crc32c(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82F63B78 if crc & 1 else 0)
    crc ^= 0xFFFFFFFF
    # leveldb stores a masked CRC.
    return (((crc >> 15) | (crc << 17)) + 0xA282EAD8) & 0xFFFFFFFF


def put_batch(items: dict[str, str]) -> bytes:
    body = bytearray(struct.pack("<QI", SEQ, len(items)))
    for key, value in items.items():
        # Chromium localStorage: key `_<origin>\0<enc><key>`, value `<enc><bytes>`, enc 1 = Latin-1.
        k = b"_" + ORIGIN.encode() + b"\x00\x01" + key.encode("latin-1")
        v = b"\x01" + value.encode("latin-1")
        body += b"\x01" + varint(len(k)) + k + varint(len(v)) + v
    payload = bytes(body)
    assert len(payload) + 7 <= BLOCK
    record_type = 1  # FULL
    crc = crc32c(bytes([record_type]) + payload)
    return struct.pack("<IHB", crc, len(payload), record_type) + payload


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    src_root = Path(os.environ.get("T3P_T3_APP_DATA") or Path.home() / "Library/Application Support/t3code-v2")
    src = src_root / "Local Storage/leveldb"
    dest = Path(sys.argv[1]) / "Local Storage/leveldb"
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    for f in src.iterdir():
        if f.suffix in (".ldb", ".log"):
            shutil.copyfile(f, dest / f.name)
    (dest / "999999.log").write_bytes(put_batch({"t3code:theme-appearance-mode": "light"}))
    print(dest.parent.parent)


if __name__ == "__main__":
    main()
