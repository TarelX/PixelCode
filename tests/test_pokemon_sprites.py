"""Run with: python3 -m unittest discover -s tests -p 'test_pokemon*.py'."""
from io import BytesIO
from pathlib import Path
import importlib.util
import unittest

from PIL import Image, UnidentifiedImageError

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("pokemon", ROOT / "tools/generate_pokemon.py")
pokemon = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pokemon)


class SpriteBakerTests(unittest.TestCase):
    def test_committed_sheets_have_distinct_unclipped_frames(self):
        for name in pokemon.POKEMON.values():
            with self.subTest(name=name):
                image = Image.open(pokemon.OUT / f"{name}.png")
                self.assertEqual(image.size, (128, 32))
                frames = [image.crop((i * 32, 0, (i + 1) * 32, 32)) for i in range(4)]
                self.assertEqual(len({frame.tobytes() for frame in frames}), 4)
                for frame in frames:
                    x1, y1, x2, y2 = frame.getbbox()
                    self.assertTrue(0 < x1 < x2 < 32 and 0 < y1 < y2 < 32)

    def test_transparent_padding_is_trimmed_and_pixels_are_not_smoothed(self):
        image = Image.new("RGBA", (56, 42))
        image.paste("red", (20, 10, 30, 30))
        raw = BytesIO()
        image.save(raw, format="PNG")
        result = pokemon.sheet(raw.getvalue())
        self.assertEqual(result.crop((0, 0, 32, 32)).getbbox(), (11, 11, 21, 31))
        self.assertEqual({color for _, color in result.getcolors()}, {(0, 0, 0, 0), (255, 0, 0, 255)})

    def test_empty_and_invalid_images_are_rejected(self):
        raw = BytesIO()
        Image.new("RGBA", (56, 42)).save(raw, format="PNG")
        with self.assertRaisesRegex(ValueError, "Empty Pokémon sprite"):
            pokemon.sheet(raw.getvalue())
        with self.assertRaises(UnidentifiedImageError):
            pokemon.sheet(b"invalid PNG")
