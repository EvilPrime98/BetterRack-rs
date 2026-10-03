"""Build the shared test library used by the parity harness (both servers read the same folder).

    python make_test_library.py <out_dir> [<BetterRack rar fixtures dir>]

Deterministic (fixed zip timestamps, solid-colour PNG pages) so page hashes are stable.
uid_pairs.txt is captured with out_dir = C:\\br-test-lib, because the uid hashes the absolute path.
"""
import shutil
import struct
import sys
import zlib
import zipfile
from pathlib import Path


def png(rgb, size=8):
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    row = b"\x00" + bytes(rgb) * size
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(row * size, 9))
        + chunk(b"IEND", b"")
    )


COMIC_INFO = """<?xml version="1.0" encoding="utf-8"?>
<ComicInfo>
  <Series>Test Series</Series>
  <Number>1</Number>
  <Title>Tagged Issue</Title>
  <Year>2020</Year>
  <Publisher>Test Pub</Publisher>
  <PageCount>3</PageCount>
</ComicInfo>
"""


def cbz(path, pages, comic_info=False):
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_STORED) as z:
        for name, rgb in pages:
            zi = zipfile.ZipInfo(name, (2020, 1, 1, 0, 0, 0))
            z.writestr(zi, png(rgb))
        if comic_info:
            z.writestr(zipfile.ZipInfo("ComicInfo.xml", (2020, 1, 1, 0, 0, 0)), COMIC_INFO)


def main():
    out = Path(sys.argv[1])
    rar_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    if out.exists():
        shutil.rmtree(out)
    # Natural ordering matters: page 2 must sort before page 10.
    natural = [(f"page{n}.png", (n * 20 % 256, 40, 90)) for n in (1, 2, 3, 10, 11)]
    cbz(out / "Alpha" / "Alpha 001.cbz", natural)
    cbz(out / "Alpha" / "Alpha 002.cbz", natural[:3])
    cbz(out / "Beta" / "Nested" / "Beta Vol 1.cbz", natural[:2])
    cbz(out / "Tagged 001.cbz", [("1.png", (200, 0, 0)), ("2.png", (0, 200, 0)), ("3.png", (0, 0, 200))], comic_info=True)
    cbz(out / "Unicode ñandú #1.cbz", natural[:2])
    (out / "Empty folder").mkdir(parents=True)
    (out / "notes.txt").write_text("not a comic, must be ignored by the scan\n")
    if rar_dir and rar_dir.is_dir():
        (out / "Rar").mkdir()
        for f in sorted(rar_dir.glob("*.cbr")):
            shutil.copy(f, out / "Rar" / f.name)
    print(f"test library written to {out}")


if __name__ == "__main__":
    main()
