"""Generate the original placeholder general sprites (pure stdlib, no deps).

Usage: python3 tools/sprites/generate.py [--preview out.png]
Each sheet is 4 frames of 16x24: idle-0, idle-1 (breath), walk-0, walk-1.
"""
import struct
import sys
import zlib
from pathlib import Path

W, H = 16, 24
OUT = Path(__file__).resolve().parents[2] / "assets" / "sprites"

PALETTE = {
    "K": "2b1b0e", "X": "3a2a22",
    "S": "f4c99b", "s": "d99a6c", "p": "e88a7a",
    "H": "2a2433", "h": "4a4058",
    "W": "f4efe2", "w": "c8c0ae",
    "B": "2f3f73", "b": "4e66a8",
    "G": "3d8b4a", "g": "2a6334",
    "r": "c4413a", "d": "8a2a2a",
    "Y": "e3b53c", "y": "b0812a",
    "P": "e98aa8", "q": "c2607f",
    "U": "6b4a8a", "u": "4a3263",
    "N": "8a5a32", "n": "5e3b1f",
    "L": "8fb8de", "l": "5b86b5",
    "M": "a7adb8", "m": "6d7380",
    "R": "c0503a", "D": "93362a",
    "A": "9a9a9a", "a": "d8d4cc",
}

GENERALS = {
    "kongming": """
................
.....KKKKKK.....
....KBBBBBBK....
...KBbBBBBbBK...
...KBBBBBBBBK...
..KHBBBBBBBBHK..
..KHSSSSSSSSHK..
..KHSKSSSSKSHK..
..KHSKSSSSKSHK..
..KSpSSSSSSpSK..
...KSSSssSSSK.KK
....KKSSSSKK.KWa
....KBWWWWBK.KWa
...KWWBWWBWWKKWK
..KWWWWBBWWWKWaK
..KWwWWBBWWwKaWK
..KWwWWBBWWwWKK.
..KSKWWBBWWKSNK.
...KWWWBBWWWKNK.
...KWWWBBWWWK...
...KwWWBBWWwK...
...KBBBBBBBBK...
....KXXKKXXK....
....KKKKKKKK....
""",
    "guanyu": """
................
......KKKK......
....KKGGGGKK....
...KGGGYYGGGK...
...KGGGGGGGGK...
..KHGGGGGGGGHK..
..KHRRRRRRRRHK..
..KHKKRRRRKKHK..
..KHRKRRRRKRHK..
..KRRRRRRRRRRK..
...KHRRRRRRHK...
...KHHHHHHHHK...
..KGKHHHHHHKGK..
..KGGKHHHHKGGK..
.KGGGGKHHKGGGGK.
.KGgGGYKKYGGgGK.
.KGgGGYGGYGGgGK.
.KRKGGYGGYGGKRK.
..KGGGYGGYGGGK..
..KGGGYYYYGGGK..
..KgGGGGGGGGgK..
..KGGGGGGGGGGK..
...KXXK..KXXK...
...KKKK..KKKK...
""",
    "lvbu": """
.Kr..........rK.
..Kr........rK..
...KrKKKKKKrK...
...KYYYYYYYYK...
..KYYrYYYYrYYK..
..KHYYYYYYYYHK..
..KHSSSSSSSSHK..
..KHKKSSSSKKHK..
..KHSKSSSSKSHK..
..KSSSSSSSSSSK..
...KSSSKKSSSK...
....KKSSSSKK....
...KMKrrrrKMK...
..KMMKrYYrKMMK..
..KMmKrrrrKmMK..
..KrKrdrrdrKrK..
..KrKrrYYrrKrK..
..KSKrdrrdrKSK..
...KYYYYYYYYK...
...KrrdrrdrrK...
...KrrdrrdrrK...
...KddKKKKddK...
...KXXK..KXXK...
...KKKK..KKKK...
""",
    "diaochan": """
................
...KK......KK...
..KHHK.KK.KHHK..
..KHhKKHHKKhHK..
...KKHHHHHHKK...
..KHHHHHHHHHYK..
..KHHSSSSSSHHY..
..KHSKSSSSKSHK..
..KHSKSSSSKSHK..
..KHpSSSSSSpHK..
..KHKSSrrSSKHK..
..KHHKSSSSKHHK..
..KHKPWWWWPKHK..
...KPPPWWPPPK...
..KPPPPPPPPPPK..
..KPqPPYYPPqPK..
.KPPqPPPPPPqPPK.
.KPPKPPPPPPKPPK.
..KKKPPPPPPKKK..
...KPPqPPqPPK...
..KPPPqPPqPPPK..
..KPPPqPPqPPPK..
..KKKKKKKKKKKK..
................
""",
    "pangtong": """
................
....K.KK.KK.....
...KHKHHKHHK....
..KHHHHHHHHHK...
..KHhHHHHHhHHK..
..KHHHHHHHHHHK..
..KHSSSSSSSSHK..
..KHSKKSSKKSHK..
..KHSSKSSKSSHK..
..KSSSSSSSSSSK..
...KSsSKKSsSK...
....KKsssSKK....
...KNKWWWWKNK...
..KNNNKWWKNNNK..
..KNNNNKKNNNNK..
..KNnNNNNNWWWK..
..KNnNNNNKWaWK..
..KSKNNNNKWWWK..
...KNNNNNNSKK...
...KNNyyyyNNK...
...KnNNNNNNnK...
...KNNNNNNNNK...
....KXXKKXXK....
....KKKKKKKK....
""",
    "xushu": """
................
.....KKKKKK.....
....KHHHHHHK....
...KHHHHHHHHK...
...KLLLLLLLLKLK.
..KHlLLLLLLlHKLK
..KHSSSSSSSSHK..
..KHKKSSSSKKHK..
..KHSKSSSSKSHK..
..KSSSSSSSSSSK..
...KSSSssSSSK...
....KKSSSSKK....
...KlKLWWLKlK...
..KLLLKWWKLLLK..
..KLLLLLLLLLLKK.
..KLlLLLLLLlKMK.
..KLlLLLLLLlKMK.
..KSKLLnnLLKSMK.
...KLLLLLLLLKMK.
...KLLLLLLLLKyK.
...KlLLLLLLlKyK.
...KllllllllKK..
....KXXKKXXK....
....KKKKKKKK....
""",
    "zhangzhao": """
................
.....KKKKKK.....
.KK.KHHHHHHK.KK.
KHHKHHHHHHHHKHHK
.KKKHhHHHHhHKKK.
...KHHHHHHHHK...
..KAKKKKKKKKAK..
..KASSSSSSSSAK..
..KASKSSSSKSAK..
..KSSSSSSSSSSK..
...KSaaaaaaSK...
...KaaaaaaaaK...
..KUKaaaaaaKUK..
..KUUKaaaaKUUK..
.KUUUUKaaKUUUUK.
.KUuUUUKKWWUuUK.
.KUuUUUUKWWKuUK.
.KSKUUUUKWWKKSK.
..KUUUUUKWWKUK..
..KUUYYYYYYUUK..
..KuUUUUUUUUuK..
..KUUUUUUUUUUK..
...KXXK..KXXK...
...KKKK..KKKK...
""",
}

BUBBLES = {
    "bubble-wait": """
.KKKKK.
KWWYWWK
KWWYWWK
KWWYWWK
KWWWWWK
KWWYWWK
.KKKKK.
...KK..
...K...
""",
    "bubble-error": """
.KKKKK.
KrrWrrK
KrrWrrK
KrrWrrK
KrrrrrK
KrrWrrK
.KKKKK.
...KK..
...K...
""",
}


def parse(art, width, height):
    rows = [line for line in art.strip("\n").split("\n")]
    assert len(rows) == height, (len(rows), height)
    for row in rows:
        assert len(row) == width, row
        for ch in row:
            assert ch == "." or ch in PALETTE, ch
    return [list(row) for row in rows]


def breathe(grid, waist):
    out = [row[:] for row in grid]
    for y in range(waist, 0, -1):
        out[y] = grid[y - 1][:]
    out[0] = ["."] * W
    return out


def step(grid, lift_left):
    out = [row[:] for row in grid]
    for x in range(0, W // 2) if lift_left else range(W // 2, W):
        if grid[-2][x] != ".":
            out[-3][x] = grid[-2][x]
        out[-2][x] = grid[-1][x]
        out[-1][x] = "."
    return out


def frames_for(grid):
    return [grid, breathe(grid, 13), step(grid, True), step(grid, False)]


def png(path, pixels, width, height):
    rgba = {ch: bytes.fromhex(hex_) + b"\xff" for ch, hex_ in PALETTE.items()} | {".": bytes(4)}
    raw = bytearray()
    for y in range(height):
        raw += b"\x00"
        for x in range(width):
            raw += rgba[pixels[y][x]]

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
                     + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


def sheet(frames):
    return [sum((frame[y] for frame in frames), []) for y in range(H)]


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    sheets = {}
    for name, art in GENERALS.items():
        rows = sheet(frames_for(parse(art, W, H)))
        sheets[name] = rows
        png(OUT / f"{name}.png", rows, W * 4, H)
    for name, art in BUBBLES.items():
        png(OUT / f"{name}.png", parse(art, 7, 9), 7, 9)
    if "--preview" in sys.argv:
        scale, gap = 8, 2
        names = list(sheets)
        width, height = (W * 4 + gap) * scale, (H + gap) * len(names) * scale
        canvas = [["W"] * width for _ in range(height)]
        for i, name in enumerate(names):
            for y in range(H):
                for x in range(W * 4):
                    ch = sheets[name][y][x]
                    if ch == ".":
                        continue
                    for dy in range(scale):
                        row = canvas[((H + gap) * i + y) * scale + dy]
                        for dx in range(scale):
                            row[x * scale + dx] = ch
        png(Path(sys.argv[sys.argv.index("--preview") + 1]), canvas, width, height)


if __name__ == "__main__":
    main()
