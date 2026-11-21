"""Original PixelCode scenery (MIT). Regenerate with Python 3 + Pillow."""
import random
from pathlib import Path
from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parents[1] / "themes/three-kingdoms/tiles"
INK, WOOD, GOLD = "#343c2c", "#775542", "#d7bd88"
GRASS, LIGHT, DARK = "#627650", "#7d8c5e", "#526747"
ROOF, EDGE, TILE = "#a45f4b", "#623f36", "#c98261"


def rect(d, x, y, w, h, c):
    d.rectangle((x, y, x + w - 1, y + h - 1), fill=c)


def tree(d, x, y, pink=False):
    a, b, c = ("#a95c60", "#cf827e", "#e8ac95") if pink else ("#40583e", "#5b764d", "#859561")
    rect(d, x - 10, y + 17, 24, 5, DARK)
    rect(d, x - 2, y + 4, 5, 16, WOOD)
    rect(d, x + 1, y + 7, 2, 12, "#aa7a54")
    d.polygon([(x - 13, y - 2), (x - 8, y - 2), (x - 8, y - 8), (x + 5, y - 8), (x + 5, y - 5), (x + 12, y - 5), (x + 12, y + 1), (x + 16, y + 1), (x + 16, y + 10), (x + 10, y + 10), (x + 10, y + 14), (x - 9, y + 14), (x - 9, y + 10), (x - 15, y + 10), (x - 15, y + 2), (x - 13, y + 2)], fill=a)
    rect(d, x - 10, y - 2, 21, 10, b)
    rect(d, x - 5, y - 6, 10, 4, b)
    rect(d, x - 13, y + 2, 4, 5, b)
    rect(d, x + 9, y + 3, 5, 5, b)
    for dx, dy, w in [(-8, 0, 6), (-3, -4, 5), (5, 1, 4), (-6, 7, 3)]:
        rect(d, x + dx, y + dy, w, 2, c)
    if pink:
        for dx, dy in [(-11, 20), (8, 25), (19, 13), (-4, 28)]:
            rect(d, x + dx, y + dy, 2, 1, b)


def roof(d, x, y, w, h):
    for row in range(h):
        inset = max(0, (h - row - 2) // 2)
        rect(d, x + inset, y + row, w - 2 * inset, 1, EDGE if row in (0, h - 1) else ROOF)
    for row in range(3, h - 2, 3):
        inset = max(0, (h - row - 2) // 2)
        rect(d, x + inset + 1, y + row, w - 2 * inset - 2, 1, TILE)
        for col in range(x + inset + 5, x + w - inset - 2, 7):
            rect(d, col, y + row - 1, 1, 3, EDGE)
    rect(d, x + 5, y - 2, w - 10, 2, EDGE)
    rect(d, x + 7, y - 2, w - 14, 1, GOLD)
    rect(d, x - 2, y + h - 3, 4, 2, EDGE)
    rect(d, x + w - 2, y + h - 3, 4, 2, EDGE)


def hall(d, x, y, w, h, large=False):
    rect(d, x + 2, y + h - 3, w, 5, DARK)
    rect(d, x + 4, y + 11, w - 8, h - 12, INK)
    rect(d, x + 6, y + 12, w - 12, h - 16, "#d0bd91")
    for col in (x + 6, x + w - 9):
        rect(d, col, y + 10, 3, h - 12, WOOD)
        rect(d, col + 1, y + 10, 1, h - 13, "#aa7a54")
    rect(d, x + w // 2 - 5, y + h - 17, 10, 13, WOOD)
    rect(d, x + w // 2 - 4, y + h - 15, 8, 11, "#514536")
    for col in [x + 12, x + w - 18]:
        rect(d, col, y + h - 16, 6, 6, WOOD)
        rect(d, col + 1, y + h - 15, 4, 3, GOLD)
        rect(d, col + 3, y + h - 15, 1, 4, WOOD)
    roof(d, x, y, w, 14 if large else 12)
    rect(d, x + 3, y + h - 4, w - 6, 2, "#c3b38e")
    rect(d, x + w // 2 - 9, y + h - 2, 18, 2, "#9a9474")


def fence(d, x, y, w):
    rect(d, x, y + 3, w, 2, WOOD)
    rect(d, x, y + 7, w, 2, WOOD)
    for col in range(x, x + w, 6):
        rect(d, col, y + 1, 2, 10, INK)
        rect(d, col, y, 1, 9, "#ba9a66")


def pond(d, points):
    d.polygon(points, fill=INK)
    inner = [(x + (1 if x < 55 else -1), y + (1 if y < 35 else -1)) for x, y in points]
    d.polygon(inner, fill="#5e8c88")
    for x, y, w in [(36, 31, 10), (48, 23, 6), (44, 39, 12), (63, 34, 5)]:
        rect(d, x, y, w, 1, "#9ab7a0")


def camp():
    image = Image.new("RGB", (320, 180), GRASS)
    d, rng = ImageDraw.Draw(image), random.Random(72)
    for _ in range(1900):
        x, y = rng.randrange(320), rng.randrange(180)
        rect(d, x, y, rng.choice([1, 2, 3]), 1, rng.choice([DARK, LIGHT, GRASS, GRASS]))
    for x, y, w, h in [(85, 45, 154, 14), (87, 96, 154, 14), (85, 151, 222, 13), (86, 47, 13, 116), (218, 48, 13, 116), (152, 43, 16, 112), (66, 85, 160, 12)]:
        rect(d, x - 1, y - 1, w + 2, h + 2, "#888064")
        rect(d, x, y, w, h, "#b9a880")
        for _ in range(w * h // 15):
            rect(d, rng.randrange(x, x + w), rng.randrange(y, y + h), 2, 1, rng.choice(["#cbb993", "#a99873"]))
    fence(d, 9, 5, 302)
    fence(d, 9, 167, 270)
    fence(d, 308, 167, 4)
    for y in range(16, 164, 6):
        rect(d, 9, y, 3, 5, WOOD)
        rect(d, 308, y, 3, 5, WOOD)
    hall(d, 128, 14, 64, 32, True)
    hall(d, 241, 18, 46, 29)
    hall(d, 33, 65, 46, 29)
    hall(d, 241, 65, 46, 29)
    hall(d, 33, 130, 46, 28)
    hall(d, 209, 130, 46, 28)
    pond(d, [(28, 25), (36, 18), (61, 18), (72, 27), (69, 41), (55, 46), (32, 40)])
    rect(d, 47, 30, 27, 6, WOOD)
    for x in range(48, 73, 4):
        rect(d, x, 30, 1, 6, "#caab79")
    tree(d, 28, 23, True)
    tree(d, 78, 23, True)
    rect(d, 128, 65, 64, 14, INK)
    rect(d, 130, 65, 60, 11, "#9c7550")
    for x in range(132, 190, 4):
        rect(d, x, 66, 1, 9, "#c2a170")
    rect(d, 150, 77, 20, 2, GOLD)
    rect(d, 112, 110, 95, 31, "#9e926d")
    rect(d, 114, 112, 91, 27, "#b9a880")
    for x in range(119, 204, 8):
        rect(d, x, 111, 4, 1, GOLD)
        rect(d, x, 139, 4, 1, "#807958")
    for x in [120, 196]:
        rect(d, x, 119, 3, 14, WOOD)
        rect(d, x - 3, 122, 9, 2, WOOD)
        rect(d, x - 2, 114, 7, 6, "#cfae79")
        rect(d, x, 116, 3, 2, EDGE)
    hall(d, 272, 128, 32, 31)
    rect(d, 282, 143, 12, 18, "#363c2d")
    rect(d, 284, 143, 8, 18, "#b9a880")
    for x, y in [(103, 26), (207, 23), (23, 112), (295, 110), (92, 136), (302, 32)]:
        tree(d, x, y)
    for x, y in [(121, 35), (195, 35), (81, 76), (233, 76)]:
        rect(d, x, y - 8, 1, 18, WOOD)
        rect(d, x + 1, y - 7, 7, 8, ROOF)
        rect(d, x + 2, y - 6, 4, 1, GOLD)
    for x, y in [(20, 153), (300, 67), (21, 52), (200, 56), (75, 117)]:
        rect(d, x, y, 5, 3, "#8a9273")
        rect(d, x + 1, y - 1, 3, 1, "#b0ad88")
    image.save(OUT / "camp.png")


def orchard():
    image = Image.new("RGBA", (192, 144), (0, 0, 0, 0))
    d = ImageDraw.Draw(image)
    island = [(13, 58), (26, 58), (26, 41), (43, 41), (43, 30), (84, 30), (84, 25), (132, 25), (132, 34), (160, 34), (160, 50), (176, 50), (176, 98), (163, 98), (163, 113), (140, 113), (140, 123), (52, 123), (52, 116), (29, 116), (29, 103), (13, 103)]
    d.polygon([(x, y + 5) for x, y in island], fill="#293a2c")
    d.polygon(island, fill=DARK)
    d.polygon([(23, 63), (37, 63), (37, 48), (62, 48), (62, 37), (126, 37), (126, 43), (157, 43), (157, 56), (167, 56), (167, 94), (153, 94), (153, 109), (131, 109), (131, 115), (54, 115), (54, 108), (35, 108), (35, 97), (23, 97)], fill=GRASS)
    rect(d, 83, 64, 23, 52, "#b4a079")
    rect(d, 62, 90, 67, 12, "#b4a079")
    rng = random.Random(8)
    for _ in range(100):
        x, y = rng.randrange(36, 157), rng.randrange(47, 108)
        if image.getpixel((x, y))[:3] == (98, 118, 80):
            rect(d, x, y, 2, 1, LIGHT)
    fence(d, 35, 52, 35)
    fence(d, 119, 52, 38)
    hall(d, 73, 44, 43, 32, True)
    rect(d, 85, 58, 19, 19, "#444932")
    rect(d, 88, 59, 13, 19, "#909664")
    roof(d, 73, 44, 43, 13)
    tree(d, 48, 45, True)
    tree(d, 134, 43, True)
    tree(d, 31, 71, True)
    tree(d, 152, 69, True)
    tree(d, 60, 85, True)
    for x, y in [(42, 26), (121, 19), (162, 44), (65, 115), (142, 104), (24, 99), (112, 84)]:
        rect(d, x, y, 2, 2, "#d98f83")
        rect(d, x + 3, y + 2, 1, 1, "#e7b599")
    d.polygon([(124, 91), (143, 88), (164, 95), (164, 102), (149, 112), (122, 108), (114, 100)], fill=INK)
    d.polygon([(124, 94), (143, 91), (160, 96), (160, 101), (147, 108), (124, 105), (118, 100)], fill="#5e8c88")
    rect(d, 126, 97, 13, 1, "#a0b6a2")
    rect(d, 143, 102, 8, 1, "#a0b6a2")
    for x, y in [(92, 84), (90, 99), (95, 109)]:
        rect(d, x, y, 7, 3, "#d4bc93")
    image.save(OUT / "orchard.png")


if __name__ == "__main__":
    camp()
    orchard()
