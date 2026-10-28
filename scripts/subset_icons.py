"""Regenerate assets/phosphor-subset.ttf — only the Phosphor glyphs the app uses.

The full Phosphor Regular font is ~480 KB; the app uses a few dozen icons, so we
embed a subset instead (see `ui::theme::install_fonts`). Run after adding an icon:

    pip install fonttools
    python scripts/subset_icons.py

`cargo test` (theme::tests::icon_subset_covers_used_glyphs) fails if a used icon
is missing from the subset, so forgetting this step is caught.
"""
import json
import pathlib
import re
import subprocess
import sys

from fontTools import subset

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "phosphor-subset.ttf"


def phosphor_dir() -> pathlib.Path:
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1"], cwd=ROOT))
    for pkg in meta["packages"]:
        if pkg["name"] == "egui-phosphor":
            return pathlib.Path(pkg["manifest_path"]).parent
    sys.exit("egui-phosphor not found in cargo metadata")


def main() -> None:
    pdir = phosphor_dir()
    consts = dict(re.findall(
        r'pub const (\w+): &str = "\\u\{([0-9A-F]+)\}";',
        (pdir / "src" / "variants" / "regular.rs").read_text(encoding="utf-8")))

    used: set[int] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        for name in re.findall(r"(?:icon|regular)::([A-Z0-9_]+)", text):
            if name in consts:
                used.add(int(consts[name], 16))
        for cp in re.findall(r"\\u\{([0-9A-Fa-f]{4,5})\}", text):
            if 0xE000 <= int(cp, 16) <= 0xF8FF:
                used.add(int(cp, 16))

    opts = subset.Options()
    opts.layout_features = []
    opts.name_IDs = ["*"]  # keep the license / copyright records
    opts.notdef_outline = True
    opts.hinting = False
    font = subset.load_font(str(pdir / "res" / "Phosphor.ttf"), opts)
    sub = subset.Subsetter(opts)
    sub.populate(unicodes=sorted(used))
    sub.subset(font)
    subset.save_font(font, str(OUT), opts)
    print(f"{len(used)} glyphs -> {OUT.relative_to(ROOT)} ({OUT.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
