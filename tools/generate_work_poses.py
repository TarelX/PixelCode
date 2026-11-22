"""Bake desk poses from the existing art; no downloads or changes to travel sprites."""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets/work-sprites"
INK = "#313131"


def smeargle_arms(tap, reading=False):
    image = Image.new("RGBA", (32, 32))
    d = ImageDraw.Draw(image)
    for x, down in [(13, tap == 1), (22, tap == 2)]:
        y = 28 + int(down)
        d.polygon([(x, 22), (x + 2, 23), (x + 2, 25), (x, 27), (x, y + 1), (x - 3, y + 1), (x - 3, 27), (x - 1, 25), (x - 1, 23)], fill=INK)
        d.polygon([(x, 23), (x + 1, 24), (x + 1, 25), (x - 1, 27), (x - 1, y), (x - 2, y), (x - 2, 27), (x, 25)], fill="#f6f6bd")
        d.line([(x - 2, y), (x, y)], fill="#cdbd94")
    return image


def smeargle(tap, reading=False):
    source = Image.open(ROOT / "assets/sprites/smeargle.png").convert("RGBA")
    image = Image.new("RGBA", (32, 32))
    d = ImageDraw.Draw(image)
    # Brush-tail is resting behind the seat, not held in a third working hand.
    d.line([(24, 27), (28, 28), (29, 25)], fill=INK, width=3)
    d.line([(24, 27), (28, 27), (29, 24)], fill="#a48b73", width=1)
    d.rectangle((28, 22, 30, 24), fill="#5a9c31")
    d.point((29, 22), fill="#8bbd41")
    d.polygon([(16, 21), (21, 21), (23, 25), (21, 29), (16, 29), (14, 25)], fill=INK)
    d.polygon([(16, 22), (21, 22), (22, 25), (20, 28), (17, 28), (15, 25)], fill="#a48b73")
    d.rectangle((17, 24, 20, 27), fill="#f6f6bd")
    d.rectangle((16, 22, 21, 23), fill="#9c5239")
    # Two short sleeves and broad paws are part of the seated body silhouette.
    image.alpha_composite(smeargle_arms(tap))
    # Preserve the original face/hat/ears; remove the standing arms and raised brush pose.
    head = source.crop((12, 9, 27, 22))
    image.alpha_composite(head, (12, 9 + int(reading)))
    return image


def metagross(tap, reading=False):
    source = Image.open(ROOT / "assets/sprites/metagross.png").convert("RGBA").crop((0, 0, 32, 32))
    image = source.copy()
    d = ImageDraw.Draw(image)
    d.rectangle((9, 24, 25, 31), fill=(0, 0, 0, 0))
    # Retain the supporting rear legs. Replace the dangling foreground limb with
    # two weight-bearing, six-pixel-wide forelegs whose claws meet the keys.
    for x, down in [(11, tap == 1), (20, tap == 2)]:
        y = 28 + int(down)
        d.polygon([(x - 2, 21), (x + 3, 21), (x + 4, y), (x + 2, y + 2), (x - 3, y + 2), (x - 4, y - 1)], fill=INK)
        d.polygon([(x - 1, 22), (x + 2, 22), (x + 3, y), (x + 1, y + 1), (x - 2, y + 1), (x - 3, y - 1)], fill="#4a6aac")
        d.polygon([(x, 23), (x + 2, 23), (x + 2, y - 1), (x, y), (x - 2, y), (x - 2, y - 2)], fill="#83b4ff")
        d.rectangle((x - 2, y + 1, x - 1, y + 1), fill="#bdbdbd")
        d.rectangle((x + 1, y + 1, x + 2, y + 1), fill="#bdbdbd")
    face = source.crop((9, 13, 24, 24))
    image.alpha_composite(face, (9, 13 + int(reading)))
    return image


def sheet(draw):
    image = Image.new("RGBA", (128, 32))
    for index, frame in enumerate([draw(0), draw(1), draw(2), draw(0, True)]):
        image.alpha_composite(frame, (index * 32, 0))
    return image


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for name, draw in [("smeargle", smeargle), ("metagross", metagross)]:
        sheet(draw).save(OUT / f"{name}.png")
    # The working paws cross the foreground desk lip. Reuse their exact pixels
    # above the keyboard, not a second pair of generic hands.
    for name, draw in [("smeargle", smeargle_arms), ("metagross", metagross)]:
        contact = Image.new("RGBA", (128, 32))
        poses = sheet(draw)
        for index in range(4):
            contact.alpha_composite(poses.crop((index * 32 + 7, 24, index * 32 + 25, 32)), (index * 32 + 7, 24))
        contact.save(OUT / f"{name}-hands.png")
