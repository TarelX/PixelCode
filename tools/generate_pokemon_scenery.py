"""Original PixelCode grass and research lab exteriors, MIT. Python 3 + Pillow."""
from pathlib import Path
from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parents[1] / "themes/pokemon/tiles"


def garden(size, lab):
    image = Image.new("RGBA", size, "#63904b")
    d = ImageDraw.Draw(image)
    w, h = size
    for y in range(3, h, 9):
        for x in range(3, w, 11):
            d.line((x, y + 2, x, y), fill="#456f40")
            d.point((x + 1, y + 1), fill="#a4b965")
    d.rectangle((w // 2 - 9, 45, w // 2 + 9, h), fill="#c6bc90")
    d.rectangle((10, h // 2, w - 11, h // 2 + 12), fill="#c6bc90")
    x, y, width = lab
    d.rectangle((x, y, x + width, y + 38), fill="#d5dfcf", outline="#334f57", width=2)
    d.rectangle((x - 2, y - 5, x + width + 2, y + 2), fill="#597d91", outline="#334f57", width=2)
    for wx in (x + 7, x + width - 16):
        d.rectangle((wx, y + 9, wx + 9, y + 20), fill="#88bfd0", outline="#334f57")
        d.line((wx + 1, y + 10, wx + 7, y + 10), fill="#d8f0e9")
    d.rectangle((x + width // 2 - 5, y + 21, x + width // 2 + 5, y + 38), fill="#597d91", outline="#334f57")
    d.ellipse((12, h - 42, 54, h - 16), fill="#4b8791", outline="#c1ceab", width=2)
    for tx, ty in [(12, 15), (w - 29, 15), (w - 29, h - 37)]:
        d.rectangle((tx + 7, ty + 14, tx + 10, ty + 26), fill="#6b694b")
        d.rectangle((tx, ty + 4, tx + 19, ty + 19), fill="#355f42")
        d.rectangle((tx + 3, ty, tx + 16, ty + 15), fill="#4c7c48")
        d.line((tx + 4, ty + 3, tx + 13, ty + 3), fill="#86a459", width=2)
    return image


garden((320, 180), (126, 17, 68)).save(OUT / "camp.png")
garden((192, 144), (68, 17, 56)).save(OUT / "orchard.png")
