"""Miles Mod Manager app icon, in Miles's mod-poster style (same pipeline as Drinkable / ShotgunPlus / ResolutionAdjust:
flat vintage-tinted objects, one ink outline around each object, Alfa Slab One letters with an ink drop shadow,
print speckles). From his sketch: a wooden crate with MILES on it, a wrench, ruler, screwdriver and hammer sticking
out, and his character peeking up from the bottom-left. No poster/circle behind it: the crate is the icon shape.
Usage: python3 miles_icon.py <AlfaSlabOne-Regular.ttf> <character.png> <out prefix> [sun]"""
import sys, random, math
from PIL import Image, ImageDraw, ImageFont, ImageFilter, ImageEnhance, ImageChops

FONT, CHAR, OUT = sys.argv[1:4]
FLAGS = sys.argv[4:]
SUN_ON = "sun" in FLAGS
PAPER = "paper" in FLAGS
GLYPH = "glyph" in FLAGS
K = 2                                   # draw at 2x, scale down at the end
CREAM, INK, SUN = (226, 222, 205), (36, 32, 26), (219, 190, 52)
WOOD, WOOD_DK, WOOD_LT, WOOD_PL = (150, 92, 46), (104, 62, 30), (176, 114, 60), (138, 84, 41)
STEEL, STEEL_LT, STEEL_HI = (84, 88, 92), (128, 134, 136), (176, 180, 178)
RED, RED_DK, RED_HI = (186, 52, 40), (132, 36, 28), (222, 108, 92)
RULER, RULER_DK = (214, 170, 72), (170, 130, 50)


def k(*v): return [int(round(x * K)) for x in v]


def layer(w, h): return Image.new("RGBA", (w * K, h * K), (0, 0, 0, 0))


def vintage(img):
    rgb = img.convert("RGB")
    rgb = ImageEnhance.Color(rgb).enhance(0.85)
    rgb = ImageChops.multiply(rgb, Image.new("RGB", rgb.size, (250, 238, 218)))
    out = rgb.convert("RGBA"); out.putalpha(img.split()[3]); return out


def outlined(img, w):
    w *= K
    pad = Image.new("RGBA", (img.width + 2 * w + 4, img.height + 2 * w + 4), (0, 0, 0, 0))
    pad.alpha_composite(img, (w + 2, w + 2)); img = pad
    a = img.split()[3].point(lambda v: 255 if v > 40 else 0)
    big = a.filter(ImageFilter.MaxFilter(w * 2 + 1))
    o = Image.new("RGBA", img.size, INK + (0,)); o.putalpha(big); o.alpha_composite(img); return o


def bez(p0, p1, p2, p3, n=24):
    return [tuple((1 - t) ** 3 * a + 3 * (1 - t) ** 2 * t * b + 3 * (1 - t) * t ** 2 * c + t ** 3 * d
                  for a, b, c, d in zip(p0, p1, p2, p3)) for t in [i / n for i in range(n + 1)]]


# ---------------- tools ----------------
def wrench():
    L = layer(300, 560); d = ImageDraw.Draw(L)
    d.rounded_rectangle(k(108, 190, 192, 560), radius=30 * K, fill=STEEL_LT)
    d.line(k(130, 230, 130, 540), fill=STEEL_HI, width=12 * K)
    d.ellipse(k(30, 30, 270, 270), fill=STEEL_LT)
    d.chord(k(30, 30, 270, 270), 20, 140, fill=STEEL)                       # shade under the head
    d.arc(k(52, 52, 248, 248), 150, 230, fill=STEEL_HI, width=12 * K)       # shine
    d.rectangle(k(112, 0, 188, 150), fill=(0, 0, 0, 0))                     # open jaw
    d.ellipse(k(112, 112, 188, 188), fill=(0, 0, 0, 0))
    return L


def ruler():
    L = layer(124, 480); d = ImageDraw.Draw(L)
    d.rectangle(k(8, 8, 116, 480), fill=RULER)
    d.rectangle(k(92, 8, 116, 480), fill=RULER_DK)
    for i, y in enumerate(range(40, 470, 30)):
        d.line(k(8, y, 8 + (40 if i % 2 == 0 else 22), y), fill=INK, width=6 * K)
    return L


def screwdriver():
    L = layer(170, 540); d = ImageDraw.Draw(L)
    d.rectangle(k(66, 40, 104, 320), fill=STEEL_LT)
    d.polygon(k(66, 42, 74, 6, 96, 6, 104, 42), fill=STEEL_LT)
    d.line(k(78, 30, 78, 300), fill=STEEL_HI, width=10 * K)
    d.rounded_rectangle(k(22, 300, 148, 540), radius=38 * K, fill=RED)
    d.rectangle(k(36, 284, 134, 322), fill=STEEL)
    for x in (62, 86, 110):
        d.line(k(x, 350, x, 520), fill=RED_DK, width=11 * K)
    d.line(k(40, 344, 40, 520), fill=RED_HI, width=11 * K)
    return L


def hammer():
    L = layer(320, 520); d = ImageDraw.Draw(L)
    d.rounded_rectangle(k(124, 90, 180, 520), radius=16 * K, fill=WOOD_LT)
    d.line(k(140, 120, 140, 500), fill=WOOD, width=9 * K)
    d.line(k(162, 160, 160, 460), fill=WOOD_DK, width=5 * K)
    claw = bez((220, 44), (262, 44), (298, 70), (312, 128)) + bez((312, 128), (282, 104), (254, 98), (220, 106))
    d.polygon([tuple(c * K for c in p) for p in claw], fill=STEEL_LT)
    d.rectangle(k(84, 44, 222, 106), fill=STEEL_LT)
    d.rectangle(k(20, 26, 92, 124), fill=STEEL)
    d.line(k(96, 56, 214, 56), fill=STEEL_HI, width=10 * K)
    d.line(k(34, 40, 34, 110), fill=STEEL_LT, width=10 * K)
    return L


def tool(img, angle, outline=12):
    t = outlined(vintage(img), outline)
    return t.rotate(angle, resample=Image.BICUBIC, expand=True)


# ---------------- crate ----------------
CW, CH, RAIL, BOT, POST = 860, 640, 80, 76, 74


def crate(with_bottom=True):
    L = layer(CW, CH); d = ImageDraw.Draw(L); rnd = random.Random(3)
    d.rectangle(k(16, RAIL - 10, CW - 16, CH), fill=WOOD)
    # three planks across the front
    y0, y1 = RAIL, CH - BOT
    h = (y1 - y0) / 3
    for i in range(3):
        a, b = y0 + i * h, y0 + (i + 1) * h
        d.rectangle(k(16, a, CW - 16, b), fill=WOOD if i % 2 == 0 else WOOD_PL)
        d.line(k(16, a + 10, CW - 16, a + 10), fill=WOOD_LT, width=8 * K)        # light top edge
        for _ in range(7):                                                      # grain
            gy = rnd.uniform(a + 26, b - 14); gx = rnd.uniform(110, CW - 320)
            d.line(k(gx, gy, gx + rnd.uniform(120, 260), gy + rnd.uniform(-4, 4)), fill=WOOD_DK, width=5 * K)
        if i:
            d.line(k(16, a, CW - 16, a), fill=INK, width=8 * K)
    # corner posts
    for x0 in (16, CW - 16 - POST):
        d.rectangle(k(x0, RAIL, x0 + POST, CH - BOT), fill=WOOD_DK)
        d.line(k(x0 + 14, RAIL + 10, x0 + 14, CH - BOT - 10), fill=WOOD, width=7 * K)
        d.line(k(x0, RAIL, x0, CH - BOT), fill=INK, width=8 * K)
        d.line(k(x0 + POST, RAIL, x0 + POST, CH - BOT), fill=INK, width=8 * K)
    # top rail (lid)
    d.rectangle(k(0, 0, CW, RAIL), fill=WOOD_LT)
    d.line(k(0, 14, CW, 14), fill=(196, 136, 78), width=10 * K)
    for gx, gy, gl in [(120, 44, 220), (420, 56, 180), (640, 36, 150)]:
        d.line(k(gx, gy, gx + gl, gy), fill=WOOD, width=5 * K)
    d.line(k(0, RAIL, CW, RAIL), fill=INK, width=9 * K)
    for x in (52, CW - 52):                                                       # nails
        d.ellipse(k(x - 8, RAIL / 2 - 8, x + 8, RAIL / 2 + 8), fill=INK)
    if with_bottom:
        bottom_rail(d)
    return L


def bottom_rail(d):
    d.rectangle(k(16, CH - BOT, CW - 16, CH), fill=WOOD)
    d.line(k(16, CH - BOT + 12, CW - 16, CH - BOT + 12), fill=WOOD_LT, width=8 * K)
    d.line(k(16, CH - BOT, CW - 16, CH - BOT), fill=INK, width=9 * K)
    for x in (52, CW - 52):
        d.ellipse(k(x - 8, CH - BOT / 2 - 8, x + 8, CH - BOT / 2 + 8), fill=INK)


def word(img, text, cx, cy, size, maxw):
    f = ImageFont.truetype(FONT, size * K)
    while f.getlength(text) + 40 * K > maxw * K:
        size -= 4; f = ImageFont.truetype(FONT, size * K)
    ld = ImageDraw.Draw(img)
    ld.text(k(cx + 13, cy + 15), text, font=f, fill=INK, anchor="mm", stroke_width=20 * K, stroke_fill=INK)
    ld.text(k(cx, cy), text, font=f, fill=CREAM, anchor="mm", stroke_width=12 * K, stroke_fill=INK)
    return f.getbbox(text, anchor="mm")


# ---------------- compose (unrotated) ----------------
W, H = 1300, 1300
img = Image.new("RGBA", (W * K, H * K), (0, 0, 0, 0))
CX, CY = 220, 520                                   # crate top-left on the canvas

if SUN_ON:
    ImageDraw.Draw(img).ellipse(k(CX + 300, CY - 330, CX + 760, CY + 130), fill=SUN)

placements = [  # (layer, angle, x of bottom centre, y of bottom)
    (wrench(), 9, CX + 175, CY + 215),
    (ruler(), 3, CX + 335, CY + 200),
    (screwdriver(), -6, CX + 520, CY + 190),
    (hammer(), -11, CX + 690, CY + 215),
]
for lay, ang, bx, by in placements:
    t = tool(lay, ang)
    img.alpha_composite(t, (int(bx * K - t.width / 2), int(by * K - t.height)))

tools_a = img.split()[3].point(lambda v: 255 if v > 100 else 0)
if SUN_ON:
    tools_a = None
body = crate(with_bottom=False)
full = outlined(crate(with_bottom=True), 14)
o = 14 * K + 2
img.alpha_composite(full, (CX * K - o, CY * K - o))          # outline + whole crate
img.alpha_composite(body, (CX * K, CY * K))
crate_a = Image.new("L", img.size, 0); crate_a.paste(full.split()[3], (CX * K - o, CY * K - o))
crate_a = crate_a.point(lambda v: 255 if v > 100 else 0)

# MILES across the top plank band, between the posts
inner = CW - 2 * (16 + POST) - 30
def paper_label(img):
    pw, ph = 560, 196
    P = layer(pw + 60, ph + 60); d = ImageDraw.Draw(P)
    d.rectangle(k(30, 30, 30 + pw, 30 + ph), fill=(234, 226, 200))
    d.rectangle(k(30, 30 + ph - 26, 30 + pw, 30 + ph), fill=(214, 204, 174))          # bottom shade
    d.polygon(k(30 + pw - 46, 30 + ph, 30 + pw, 30 + ph - 46, 30 + pw, 30 + ph), fill=(196, 184, 150))  # curled corner
    f = ImageFont.truetype(FONT, 150 * K)
    while f.getlength("MILES") > (pw - 80) * K:
        f = ImageFont.truetype(FONT, int(f.size - 4 * K))
    d.text(k(30 + pw / 2, 30 + ph / 2 - 4), "MILES", font=f, fill=INK, anchor="mm")
    P = outlined(vintage(P), 9).rotate(-3, resample=Image.BICUBIC, expand=True)
    x = int((CX + CW / 2) * K - P.width / 2); y = int((CY + RAIL + 112) * K - P.height / 2)
    img.alpha_composite(P, (x, y))
    T = Image.new("RGBA", img.size, (0, 0, 0, 0)); td = ImageDraw.Draw(T)          # tape on the top corners
    for tx, ty, a in [(CX + CW / 2 - pw / 2 + 10, CY + RAIL + 12, -30), (CX + CW / 2 + pw / 2 - 10, CY + RAIL + 2, 28)]:
        tape = Image.new("RGBA", k(130, 46), (222, 214, 178, 210))
        tape = tape.rotate(a, resample=Image.BICUBIC, expand=True)
        T.alpha_composite(tape, (int(tx * K - tape.width / 2), int(ty * K - tape.height / 2)))
    img.alpha_composite(T)


if GLYPH:
    pass
elif PAPER:
    paper_label(img)
else:
    word(img, "MILES", CX + CW / 2, CY + RAIL + 112, 200, inner)

if not GLYPH:
  pass
# Miles peeking up from inside the crate, bottom-left, tucked behind the bottom rail
ch = Image.open(CHAR).convert("RGBA")
size = 450
ch = ch.resize((size * K, size * K), Image.LANCZOS)
ch = outlined(vintage(ch), 9)
cx0, cy0 = CX + 86, CY + 262
cut = (CY + CH) - cy0                                         # cut flush with the crate's bottom edge
ch.paste((0, 0, 0, 0), (0, int((cut + 9) * K + 2), ch.width, ch.height))
rail_first = True
rail = Image.new("RGBA", body.size, (0, 0, 0, 0)); bottom_rail(ImageDraw.Draw(rail))
img.alpha_composite(rail, (CX * K, CY * K))
if not GLYPH:
    img.alpha_composite(ch, (int(cx0 * K - 9 * K - 2), int(cy0 * K - 9 * K - 2)))

# print speckles, only on the artwork
rnd = random.Random(7)
g = Image.new("RGBA", img.size, (0, 0, 0, 0)); gd = ImageDraw.Draw(g)
for _ in range(420):
    x, y, r = rnd.randrange(W * K), rnd.randrange(H * K), rnd.choice([2, 3, 3, 4]) * K
    gd.ellipse([x - r, y - r, x + r, y + r], fill=(255, 255, 240, rnd.randrange(60, 140)))
ga = ImageChops.multiply(g.split()[3], img.split()[3].point(lambda v: 255 if v > 200 else 0)); g.putalpha(ga)
if not GLYPH:
    img.alpha_composite(g)

# tilt like the sketch, fit into a square with a small margin
def fit(im, box=None):
    im = im.rotate(6, resample=Image.BICUBIC, expand=True)
    box = box or im.getbbox()
    im = im.crop(box)
    S, M = 1024, 20
    sc = (S - 2 * M) / max(im.size)
    im = im.resize((round(im.width * sc), round(im.height * sc)), Image.LANCZOS)
    out = Image.new(im.mode, (S, S), 0 if im.mode == "L" else (0, 0, 0, 0))
    out.paste(im, ((S - im.width) // 2, (S - im.height) // 2))
    return out, box

out, box = fit(img)
out.save(OUT + ".png")
if GLYPH:
    # detailed menu glyph: every ink line of the drawing becomes a gap, so planks, posts, nails and tool parts show
    import numpy as np
    a = np.array(img)
    ink = (np.abs(a[..., :3].astype(int) - np.array(INK)).sum(-1) < 60) | (a[..., 3] < 128)
    inkim = Image.fromarray((ink * 255).astype("uint8")).filter(ImageFilter.MaxFilter(2 * 7 * K + 1))
    solid = ImageChops.invert(inkim)
    gm, _ = fit(solid)
    w = Image.new("RGBA", gm.size, (255, 255, 255, 0)); w.putalpha(gm); w.save(OUT + "-glyph.png")
    sys.exit()
if tools_a is not None:
    # menu glyph: one-colour silhouette (the app tints it with the accent colour)
    gap = 26 * K
    grown = crate_a.filter(ImageFilter.MaxFilter(gap * 2 + 1))
    g = ImageChops.subtract(tools_a, grown)
    g = ImageChops.lighter(g, crate_a)
    d = ImageDraw.Draw(g)                                   # cut the lid off the box so it reads as a crate
    d.rectangle(k(CX - 30, CY + RAIL - 6, CX + CW + 30, CY + RAIL + 22), fill=0)
    gm, _ = fit(g, box)
    w = Image.new("RGBA", gm.size, (255, 255, 255, 0)); w.putalpha(gm)
    w.save(OUT + "-glyph.png")
print("ok")
