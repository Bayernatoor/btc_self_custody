#!/usr/bin/env python3
"""Generate the chart copy review package from registry.rs.

    python3 scripts/chart-copy-review.py
    python3 notes/render-spec.py notes/chart-copy-review.md

Everything a human needs to judge one chart's prose, on one screen, without
opening the codebase or running a query. The source of truth is `registry.rs`,
so regenerating this after an edit is how you see what changed.

Why this exists rather than reading registry.rs directly: a chart's copy is
spread across `desc_per_block`, `desc_daily`, `About.definition`,
`About.technical` and one `population` per measurement, and they are 200 lines
apart in a 5,000-line file. Nobody catches "this says size and that says
weight" reading it in that order. Three such disagreements shipped.

Lives in `scripts/` rather than beside its output, because `notes/` is
gitignored: a generator referenced from a working doc has to sit somewhere
tracked or it is dead on a fresh clone. Same reason as `probe-endpoints.sh`.

Risk ordering, highest first, because a review that runs out of attention
should run out of it on the safe charts:

  1. figures with nothing watching them  (KNOWN_UNGUARDED)
  2. heuristic detectors, where the copy carries the caveat or nothing does
  3. charts whose two arms declare different methods
  4. everything else
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
REG = (ROOT / "src/stats/charts/registry.rs").read_text()


def field(block, name):
    """A `name: "..."` string field, with Rust escapes resolved."""
    m = re.search(rf'{name}: "((?:[^"\\]|\\.)*)"', block)
    if not m:
        return None
    return m.group(1).replace('\\"', '"').replace("\\n", "\n").replace("\\\\", "\\")


def enum_field(block, name):
    """A `name: Enum::Variant` field.

    Anchored on a non-word character before the name, because `per_block`
    is a substring of `method_per_block` and `daily` of `method_daily`: an
    unanchored match reported every chart's aggregation as its method, which
    is exactly the kind of quietly-wrong output this package exists to stop.
    """
    m = re.search(rf"(?<![A-Za-z0-9_]){name}: (?:\w+::)?(\w+)", block)
    return m.group(1) if m else None


def chart_blocks():
    """Split registry.rs on ChartMeta boundaries, keeping only real entries."""
    out = []
    for b in re.split(r"\n    ChartMeta \{", REG):
        slug = field(b, "slug")
        if slug and field(b, "title"):
            out.append((slug, b))
    return out


def measurements(block):
    """Each Measurement's declared fields, in order."""
    out = []
    for m in re.finditer(r"Measurement \{", block):
        # Brace-match from the opening brace so nested braces do not truncate.
        i = block.index("{", m.start())
        depth, j = 0, i
        while j < len(block):
            if block[j] == "{":
                depth += 1
            elif block[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        seg = block[i : j + 1]
        out.append(
            {
                "series": field(seg, "series") or "",
                "series_daily": field(seg, "series_daily") or "",
                "quantity": field(seg, "quantity") or "",
                "population": field(seg, "population") or "",
                "method_per_block": enum_field(seg, "method_per_block"),
                "method_daily": enum_field(seg, "method_daily"),
                "per_block": enum_field(seg, "per_block"),
                "daily": enum_field(seg, "daily"),
            }
        )
    return out


def guarded_claims():
    """(slug, claim, lo, hi) from the drift table."""
    start = REG.find("let checks: &[(&str, &str, &str, f64, f64)]")
    if start < 0:
        return {}
    seg = REG[start : REG.find("\n        ];", start)]
    out = {}
    for m in re.finditer(
        r'\(\s*"([^"]+)",\s*"((?:[^"\\]|\\.)*)",\s*"(?:[^"\\]|\\.)*"\s*,\s*'
        r"(-?[\d_.e]+),\s*(-?[\d_.e]+),?\s*\)",
        seg,
        re.S,
    ):
        out.setdefault(m.group(1), []).append(
            (m.group(2), m.group(3), m.group(4))
        )
    return out


def known_unguarded():
    start = REG.find("const KNOWN_UNGUARDED")
    if start < 0:
        return {}
    seg = REG[start : REG.find("\n    ];", start)]
    out = {}
    for m in re.finditer(r'\("([^"]+)",\s*"([^"]+)"\)', seg):
        out.setdefault(m.group(1), []).append(m.group(2))
    return out


GUARDED = guarded_claims()
UNGUARDED = known_unguarded()

# Charts whose copy was rewritten or verified in the 2026-10 basis work, so a
# reviewer can skip them or spot-check rather than read cold.
RECENTLY_REWORKED = {
    "op-block-share": "numerator and denominator both reworked 2026-10-21; declaration corrected 2026-10-23",
    "all-embedded-share": "both bands moved to weight; declaration corrected 2026-10-23",
    "inscription-share": "daily arm was dividing by size until 2026-10-23; copy and declaration rewritten",
    "unified-volume": "basis stated in copy 2026-10-21",
    "coinbase-payouts": "new chart, written 2026-10-21",
    "address-types-pct": "P2A promoted to its own band 2026-10-18",
    "utxo-flow": "copy corrected 2026-10-18",
}


def risk(slug, ms):
    score = 0
    if slug in UNGUARDED:
        score += 10 * len(UNGUARDED[slug])
    if any(m["method_per_block"] == "HeuristicallyDetected" for m in ms):
        score += 5
    if any(m["method_per_block"] != m["method_daily"] for m in ms):
        score += 3
    if slug not in GUARDED:
        score += 2
    return score


def registry_count():
    """How many entries the `CHARTS` array itself declares.

    The parser above splits the whole file, which also catches `ChartMeta`
    literals in the test module. A review package that silently covers 61 of
    64 charts is worse than none, so the two counts have to agree.
    """
    i = REG.index("pub const CHARTS")
    j = REG.index("\n];", i)
    return REG[i:j].count("ChartMeta {")


def main():
    charts = chart_blocks()
    want = registry_count()
    if len(charts) != want:
        sys.exit(
            f"parsed {len(charts)} charts but CHARTS declares {want}. The "
            "parser has drifted from the file; fix it rather than shipping a "
            "review that quietly omits charts."
        )
    rows = []
    for slug, b in charts:
        ms = measurements(b)
        rows.append(
            {
                "slug": slug,
                "title": field(b, "title"),
                "category": enum_field(b, "category"),
                "unit": enum_field(b, "unit"),
                "shape": enum_field(b, "shape"),
                "desc_per_block": field(b, "desc_per_block"),
                "desc_daily": field(b, "desc_daily"),
                "definition": field(b, "definition"),
                "technical": field(b, "technical"),
                "measurements": ms,
                "risk": risk(slug, ms),
            }
        )
    # A note naming a slug that no longer exists is silent debt: it simply
    # stops appearing, and the reader never learns the annotation went stale.
    slugs = {r["slug"] for r in rows}
    stale = sorted(set(RECENTLY_REWORKED) - slugs)
    if stale:
        sys.exit(
            "RECENTLY_REWORKED names charts that are not in the registry: "
            + ", ".join(stale)
        )

    rows.sort(key=lambda r: (-r["risk"], r["slug"]))

    L = []
    w = L.append
    w("# Chart copy review\n")
    w(
        f"Generated from `registry.rs` by `scripts/chart-copy-review.py`. "
        f"**{len(rows)} charts.** Regenerate after any copy edit; do not hand-edit "
        "this file.\n"
    )
    w(
        "Ordered by risk, highest first, so a review that runs out of attention "
        "runs out on the safe charts. The score counts unwatched figures most "
        "heavily, then heuristic detection, then arms that declare different "
        "methods.\n"
    )
    w("## How to read one entry\n")
    w(
        "- **description, per-block / daily** are what a reader sees on the card "
        "and above the chart. They must agree with each other and with the "
        "builder. Three shipped disagreeing.\n"
        "- **definition** and **technical** are the drawer copy.\n"
        "- **measurement** is the declared contract: what is counted "
        "(`quantity`), over what (`population`), how it is obtained "
        "(`method`) and how a day is condensed (`aggregation`).\n"
        "- **watched** lists figures the drift gate re-measures, with the "
        "bounds it allows. **UNWATCHED** lists figures in the copy that "
        "nothing checks.\n"
    )
    w("Three questions per chart, in this order:\n")
    w(
        "1. Does the description say what the measurement declares? A mismatch "
        "here is the defect that shipped three times.\n"
        "2. Would a reader take the right quantity off the title alone?\n"
        "3. Does any caveat that matters sit where it will be read, rather than "
        "only in `technical`?\n"
    )

    w("\n## Index\n")
    w("| # | chart | risk | why |")
    w("|---|---|---|---|")
    for i, r in enumerate(rows, 1):
        why = []
        if r["slug"] in UNGUARDED:
            why.append(f"{len(UNGUARDED[r['slug']])} unwatched")
        if any(m["method_per_block"] == "HeuristicallyDetected" for m in r["measurements"]):
            why.append("heuristic")
        if any(m["method_per_block"] != m["method_daily"] for m in r["measurements"]):
            why.append("arms differ")
        if r["slug"] in RECENTLY_REWORKED:
            why.append("**recently reworked**")
        w(f"| {i} | `{r['slug']}` | {r['risk']} | {', '.join(why) or 'routine'} |")

    for i, r in enumerate(rows, 1):
        w(f"\n---\n\n## {i}. {r['title']}\n")
        w(
            f"`{r['slug']}` &middot; {r['category']} &middot; {r['unit']} "
            f"&middot; {r['shape']} &middot; risk {r['risk']}\n"
        )
        if r["slug"] in RECENTLY_REWORKED:
            w(f"> **Recently reworked.** {RECENTLY_REWORKED[r['slug']]}\n")
        w(f"**description, per-block**\n\n> {r['desc_per_block']}\n")
        if r["desc_daily"] and r["desc_daily"] != r["desc_per_block"]:
            w(f"**description, daily**\n\n> {r['desc_daily']}\n")
        elif r["desc_daily"]:
            w("**description, daily** &mdash; identical to per-block.\n")
        if r["definition"]:
            w(f"**definition**\n\n> {r['definition']}\n")
        if r["technical"]:
            w(f"**technical**\n\n> {r['technical']}\n")
        for m in r["measurements"]:
            label = m["series"] or "(the only series)"
            w(f"**measurement: {label}**\n")
            w(f"- quantity: {m['quantity']}")
            w(f"- population: {m['population']}")
            w(
                f"- method: {m['method_per_block']} per block, "
                f"{m['method_daily']} daily"
            )
            w(
                f"- aggregation: {m['per_block']} per block, "
                f"{m['daily']} daily\n"
            )
        if r["slug"] in GUARDED:
            w("**watched by the drift gate**\n")
            for claim, lo, hi in GUARDED[r["slug"]]:
                w(f'- "{claim}" &mdash; must stay within {lo} to {hi}')
            w("")
        if r["slug"] in UNGUARDED:
            w("**UNWATCHED figures** (in the copy, nothing re-measures them)\n")
            for f in UNGUARDED[r["slug"]]:
                w(f"- `{f}`")
            w("")
        w("- [ ] reviewed")

    out = ROOT / "notes/chart-copy-review.md"
    out.write_text("\n".join(L) + "\n")
    print(f"wrote {out} ({len(rows)} charts)")

    j = ROOT / "notes/chart-copy-review.json"
    j.write_text(json.dumps(rows, indent=1))
    print(f"wrote {j}")


if __name__ == "__main__":
    sys.exit(main())
