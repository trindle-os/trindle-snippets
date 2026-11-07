"""Draws docs/images/banner.png (1280x360) and how-it-works.png (1280x360). Run from the repository root: python3 docs/images/sources/make_images.py"""
from PIL import Image, ImageDraw, ImageFont

FONT = "/usr/share/fonts/truetype/trindle-sans/TrindleSans-Variable.ttf"
BG, CARD, TEXT, MUTED, YELLOW = "#0a111e", "#14223a", "#f3f5fa", "#a9b4c8", "#fbc711"

def font(size, weight=400):
    f = ImageFont.truetype(FONT, size)
    try:
        f.set_variation_by_axes([weight])
    except (OSError, AttributeError):
        pass
    return f

def banner():
    im = Image.new("RGB", (1280, 360), BG)
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((72, 105, 222, 255), 34, fill=YELLOW)
    d.rounded_rectangle((113, 128, 181, 232), 10, fill="#fdfdf8")
    for i, w in enumerate((46, 34, 40)):
        d.rounded_rectangle((123, 148 + i * 24, 123 + w, 156 + i * 24), 4, fill="#b8890a")
    d.text((278, 92), "Snippets", font=font(84, 600), fill=TEXT)
    d.text((280, 196), "Short codes for the words you type again and again.", font=font(26), fill=MUTED)
    x = 280
    for label, on in (("Trindle plugin", True), ("Espanso files", False), ("Works offline", False)):
        w = d.textlength(label, font=font(16, 600)) + 32
        d.rounded_rectangle((x, 252, x + w, 288), 18, fill="#4b3c05" if on else CARD, outline="#7a6410" if on else "#2b3a58")
        d.text((x + 16, 260), label, font=font(16, 600), fill="#fde694" if on else TEXT)
        x += w + 10
    im.save("docs/images/banner.png")

def steps():
    im = Image.new("RGB", (1280, 360), BG)
    d = ImageDraw.Draw(im)
    items = (
        ("1", "You type a short code", ":tm"),
        ("2", "Trindle sees the code", "Snippets gives it the list"),
        ("3", "Your words appear", "Typed by Snippets"),
    )
    for k, (n, title, sub) in enumerate(items):
        x = 60 + k * 400
        d.rounded_rectangle((x, 60, x + 360, 300), 24, fill=CARD)
        d.ellipse((x + 28, 84, x + 76, 132), fill=YELLOW)
        d.text((x + 52, 108), n, font=font(28, 700), fill="#1b1500", anchor="mm")
        d.text((x + 28, 156), title, font=font(26, 600), fill=TEXT)
        d.text((x + 28, 214), sub, font=font(22), fill=MUTED)
        if k < 2:
            d.polygon([(x + 372, 170), (x + 392, 180), (x + 372, 190)], fill=YELLOW)
    im.save("docs/images/how-it-works.png")

banner()
steps()
