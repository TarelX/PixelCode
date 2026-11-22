"""Bake NightCatSama/pokedex pixels into four-frame sheets (Python 3 + Pillow).

Pokémon © Nintendo/Game Freak/Creatures. Upstream supplies no license.
Run from any directory: python3 tools/generate_pokemon.py
"""
from io import BytesIO
from pathlib import Path
from urllib.request import urlopen

from PIL import Image

REVISION = "fd9135c169271aae5b9f62f97f7dc731cd6346cd"
SOURCE = f"https://raw.githubusercontent.com/NightCatSama/pokedex/{REVISION}"
POKEMON = {25: "pikachu", 376: "metagross", 235: "smeargle", 448: "lucario",
           68: "machamp", 137: "porygon", 65: "alakazam"}
FRAME_WIDTH, FRAME_HEIGHT = 32, 32
OUT = Path(__file__).resolve().parents[1] / "assets/sprites"


def sheet(raw):
    image = Image.open(BytesIO(raw)).convert("RGBA")
    bounds = image.getbbox()
    if not bounds:
        raise ValueError("Empty Pokémon sprite")
    image = image.crop(bounds)
    # Nearest-neighbor only; keep smaller characters at their native size.
    image.thumbnail((FRAME_WIDTH - 4, FRAME_HEIGHT - 4), Image.Resampling.NEAREST)
    result = Image.new("RGBA", (FRAME_WIDTH * 4, FRAME_HEIGHT))
    for frame, (dx, dy) in enumerate(((0, 0), (0, -1), (-1, -2), (1, -1))):
        x = frame * FRAME_WIDTH + (FRAME_WIDTH - image.width) // 2 + dx
        y = FRAME_HEIGHT - image.height - 1 + dy
        result.paste(image, (x, y))
    return result


if __name__ == "__main__":
    for dex, name in POKEMON.items():
        with urlopen(f"{SOURCE}/images/pixel/{dex}.png", timeout=30) as response:
            sheet(response.read()).save(OUT / f"{name}.png")
        print(f"{name}: {FRAME_WIDTH * 4}x{FRAME_HEIGHT}")
