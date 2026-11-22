"""Original PixelCode command room, MIT. Python 3 + Pillow; no external assets."""
from pathlib import Path
from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parents[1] / "themes/three-kingdoms/tiles"
image = Image.new("RGBA", (320, 180), "#1b2029")
d = ImageDraw.Draw(image)


def box(x, y, w, h, color):
    d.rectangle((x, y, x + w - 1, y + h - 1), fill=color)


def floor(x, y, w, h, color):
    box(x, y, w, h, color)
    for row in range(y, y + h, 8):
        box(x, row, w, 1, "#343d4d")
        for col in range(x + (4 if (row // 8) % 2 else 0), x + w, 12):
            box(col, row, 1, min(8, y + h - row), "#343d4d")


def wall(x, y, w):
    box(x, y + 1, w, 12, "#1b2029")
    box(x, y, w, 2, "#687180")
    box(x, y + 3, w, 6, "#424c5d")
    box(x, y + 9, w, 2, "#2a303c")
    box(x, y + 11, w, 2, "#7b6a55")
    for col in range(x + 16, x + w, 16):
        box(col, y + 3, 1, 6, "#333c4b")


def shelf(x, y, w=22):
    box(x + 1, y + 2, w, 18, "#262b35")
    box(x, y, w, 17, "#80705b")
    box(x + 1, y + 1, w - 2, 14, "#333438")
    colors = ["#b5a887", "#8d9b92", "#977e73", "#a7a3a0", "#697f8d"]
    for row in (1, 8):
        for i in range((w - 4) // 3):
            box(x + 2 + i * 3, y + row + 1, 2, 5, colors[(i + row) % 5])
            box(x + 2 + i * 3, y + row + 2, 2, 1, "#d4c9a6")
    for row in (7, 15):
        box(x, y + row, w, 2, "#80705b")


def plant(x, y):
    box(x - 3, y + 10, 8, 2, "#252e33")
    box(x - 2, y + 5, 6, 6, "#958573")
    box(x - 3, y + 5, 8, 2, "#c3b497")
    box(x, y - 7, 1, 13, "#708264")
    for dx, dy in [(-3, -5), (1, -8), (-4, -1), (2, -3), (-2, 2)]:
        box(x + dx, y + dy, 3, 3, "#4a6758")
        box(x + dx, y + dy, 2, 1, "#8aa380")


def window(x, y):
    box(x, y, 23, 16, "#242a35")
    box(x + 1, y + 1, 21, 12, "#778b9a")
    box(x + 2, y + 2, 19, 6, "#a5b3b5")
    box(x + 2, y + 8, 19, 4, "#526d78")
    for col in (7, 14):
        box(x + col, y + 1, 1, 12, "#3a414b")
    box(x + 1, y + 7, 21, 1, "#3a414b")
    box(x - 1, y + 13, 25, 2, "#b3a386")


def lantern(x, y):
    box(x + 2, y, 1, 3, "#776a51")
    box(x, y + 3, 5, 7, "#9d7850")
    box(x + 1, y + 4, 3, 5, "#e8c88b")
    box(x + 2, y + 5, 1, 3, "#fff0bc")
    box(x + 2, y + 10, 1, 2, "#b28a55")


def desk(x, y, wide=False):
    w = 38 if wide else 28
    box(x + 2, y + 15, w, 3, "#29313b")
    box(x + 1, y + 8, w - 2, 8, "#635747")
    box(x + 3, y + 9, w - 6, 5, "#7e6a50")
    box(x, y + 2, w, 8, "#a38a68")
    box(x + 1, y + 3, w - 2, 5, "#b49b75")
    box(x + 3, y + 10, 2, 6, "#b49b75")
    box(x + w - 5, y + 10, 2, 6, "#b49b75")
    # A small terminal, manuscript, inkwell and tea cup.
    box(x + 5, y - 5, 12, 9, "#202d36")
    box(x + 6, y - 4, 10, 6, "#486670")
    box(x + 7, y - 3, 4, 1, "#9db6ac")
    box(x + 7, y - 1, 7, 1, "#81a5a2")
    box(x + 9, y + 3, 4, 1, "#343e43")
    box(x + 17, y + 3, 7, 4, "#d4ccb1")
    box(x + 18, y + 4, 4, 1, "#8b8979")
    box(x + 2, y + 4, 2, 2, "#393f43")
    if wide:
        box(x + 28, y + 2, 4, 4, "#e0d6b7")
        box(x + 29, y + 2, 2, 1, "#786b52")


# Six connected rooms around a central passage; all geometry is whole-pixel.
floor(4, 4, 312, 172, "#414c60")
floor(100, 15, 10, 160, "#303947")
floor(210, 15, 10, 160, "#303947")
floor(4, 82, 312, 18, "#35414f")
for x, width in [(4, 96), (110, 100), (220, 96)]:
    wall(x, 4, width)
    wall(x, 100, width)
    box(x, 16, 3, 66, "#27303c")
    box(x + width - 3, 16, 3, 66, "#27303c")
    box(x, 113, 3, 61, "#27303c")
    box(x + width - 3, 113, 3, 61, "#27303c")
    window(x + 35, 17 if x != 110 else 18)
    shelf(x + 7, 19)
    shelf(x + width - 28, 19)
    lantern(x + 29, 17)
    lantern(x + width - 33, 17)
    plant(x + 12, 66)
    plant(x + width - 14, 66)
    shelf(x + 7, 115)
    window(x + 36, 114)
    plant(x + width - 14, 151)
    # Doorways and blue-grey rugs.
    box(x + 30, 79, 36, 3, "#a09179")
    box(x + 28, 71, 40, 6, "#4c5e6b")
    box(x + 30, 72, 36, 1, "#71808a")
    box(x + 25, 167, 46, 4, "#4c5e6b")
    box(x + 27, 168, 42, 1, "#71808a")

for x, y, wide in [(34, 53, False), (142, 53, True), (258, 53, False),
                    (34, 142, False), (130, 142, True), (226, 142, False), (274, 142, False)]:
    desk(x, y, wide)

# A planning table and board in the commander's room.
box(125, 72, 68, 5, "#786b54")
box(127, 72, 64, 3, "#b5a17a")
for x in (132, 148, 164, 180):
    box(x, 72, 8, 2, "#dbd2af")
box(192, 33, 12, 30, "#252d38")
box(194, 35, 8, 26, "#8b8e7e")
for y in (38, 44, 50):
    box(195, y, 5, 2, "#c8c6a7")
for x in range(100, 221, 8):
    box(x, 89, 3, 1, "#5d6d76")
box(4, 175, 312, 3, "#7b6a55")
image.save(OUT / "command-room.png")
image = Image.new("RGBA", (320, 180))
d = ImageDraw.Draw(image)
for x, y, wide in [(34, 53, False), (142, 53, True), (258, 53, False),
                    (34, 142, False), (130, 142, True), (226, 142, False), (274, 142, False)]:
    desk(x, y, wide)
image.save(OUT / "command-room-desks.png")
