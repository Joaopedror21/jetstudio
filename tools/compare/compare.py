"""Compare a JetStudio capture with a reference capture of the same size.

Usage:
    python -I tools/compare/compare.py REF.png OURS.png REGIONS.json OUT_DIR

Writes to OUT_DIR:
    side_by_side.png   reference | ours
    overlay.png        50/50 blend
    diff.png           per-pixel max-channel absolute difference, amplified 4x
    report.json        per-region metrics
    report.md          the same as a table

Metrics per region (reference pixels): mean absolute error (0-255), 95th percentile of the
per-pixel max-channel difference, fraction of pixels whose difference exceeds 24, and the mean
colour of the region in each image (to separate palette shifts from geometry/content changes).

The reference captures stay local (reference/ is git-ignored); this script never copies them into
the repository. Content that only exists in one image (user samples, plugin windows, online
content) makes pixel metrics pessimistic; read them per region, together with the overlay.
"""

import json
import os
import sys

from PIL import Image, ImageChops


def region_metrics(ref, ours, box):
    a = ref.crop(box)
    b = ours.crop(box)
    diff = ImageChops.difference(a, b)
    r, g, bl = diff.split()
    maxc = ImageChops.lighter(ImageChops.lighter(r, g), bl)
    hist = maxc.histogram()
    total = sum(hist)
    mae = sum(v * c for v, c in enumerate(hist)) / total
    acc, p95 = 0, 255
    for v, c in enumerate(hist):
        acc += c
        if acc >= total * 0.95:
            p95 = v
            break
    over = sum(hist[25:]) / total

    def mean(img):
        stat = [sum(v * c for v, c in enumerate(ch.histogram())) / total for ch in img.split()]
        return "#%02X%02X%02X" % tuple(int(round(x)) for x in stat)

    return {
        "box": box,
        "mae": round(mae, 2),
        "p95": p95,
        "frac_over_24": round(over, 4),
        "mean_ref": mean(a),
        "mean_ours": mean(b),
    }


def main():
    ref_path, ours_path, regions_path, out_dir = sys.argv[1:5]
    ref = Image.open(ref_path).convert("RGB")
    ours = Image.open(ours_path).convert("RGB")
    if ref.size != ours.size:
        sys.exit(f"size mismatch: reference {ref.size} vs ours {ours.size}")
    os.makedirs(out_dir, exist_ok=True)
    w, h = ref.size
    side = Image.new("RGB", (w * 2 + 8, h), (255, 0, 255))
    side.paste(ref, (0, 0))
    side.paste(ours, (w + 8, 0))
    side.save(os.path.join(out_dir, "side_by_side.png"))
    Image.blend(ref, ours, 0.5).save(os.path.join(out_dir, "overlay.png"))
    diff = ImageChops.difference(ref, ours)
    r, g, b = diff.split()
    maxc = ImageChops.lighter(ImageChops.lighter(r, g), b).point(lambda v: min(255, v * 4))
    maxc.save(os.path.join(out_dir, "diff.png"))

    regions = json.load(open(regions_path, encoding="utf-8"))
    report = {name: region_metrics(ref, ours, tuple(box)) for name, box in regions.items()}
    report["_full"] = region_metrics(ref, ours, (0, 0, w, h))
    with open(os.path.join(out_dir, "report.json"), "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
    lines = ["| Region | MAE | p95 | >24 | mean ref | mean ours |", "|---|---:|---:|---:|---|---|"]
    for name, m in report.items():
        lines.append(
            f"| {name} | {m['mae']} | {m['p95']} | {m['frac_over_24']:.1%} | {m['mean_ref']} | {m['mean_ours']} |"
        )
    with open(os.path.join(out_dir, "report.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
