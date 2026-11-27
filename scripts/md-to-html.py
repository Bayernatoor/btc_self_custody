#!/usr/bin/env python3
"""Render a working doc's markdown to the site's HTML reading style.

House rule: research and working docs ship as a pair, same base name, and the
HTML is generated so the two cannot drift. Never hand-edit both.

    scripts/md-to-html.py notes/whatever.md        # writes notes/whatever.html

Deliberately not a markdown library. This handles the subset these documents
actually use: headings, paragraphs, bullet lists, tables, fenced code, inline
code, bold and italic. Anything it does not know, it passes through as a
paragraph rather than guessing, so an unsupported construct is visible rather
than silently dropped.
"""
import html
import re
import sys
from pathlib import Path

CSS = """
:root{--bg:#0d2137;--fg:#e8eef4;--dim:#9fb3c8;--accent:#f7931a;
--line:rgba(255,255,255,.12);--code:#08192a}
*{box-sizing:border-box}
body{margin:0;padding:3rem 1.5rem;background:var(--bg);color:var(--fg);
font:16px/1.65 -apple-system,BlinkMacSystemFont,"Segoe UI",Inter,system-ui,sans-serif}
main{max-width:58rem;margin:0 auto}
h1{font-size:2rem;margin:0 0 .25rem;letter-spacing:-.02em}
.sub{color:var(--dim);font-size:.9rem;margin:0 0 2.5rem;padding-bottom:1.25rem;
border-bottom:1px solid var(--line)}
h2{font-size:1.35rem;margin:3rem 0 .75rem;padding-bottom:.4rem;
border-bottom:1px solid var(--line)}
h3{font-size:1.05rem;margin:2rem 0 .5rem;color:var(--accent)}
p{margin:.7rem 0}
ul{margin:.7rem 0;padding-left:1.35rem}
li{margin:.4rem 0}
code{background:var(--code);padding:.12em .38em;border-radius:3px;font-size:.86em;
font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:#ffd9a0}
pre{background:var(--code);border:1px solid var(--line);border-left:3px solid var(--accent);
padding:.9rem 1.1rem;overflow-x:auto;border-radius:4px;font-size:.83rem;line-height:1.5;
font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:#cfe3f5}
pre code{background:none;padding:0;color:inherit}
table{border-collapse:collapse;width:100%;margin:1.1rem 0;font-size:.9rem}
th,td{text-align:left;padding:.5rem .7rem;border-bottom:1px solid var(--line);
vertical-align:top}
th{color:var(--accent);font-weight:600;font-size:.8rem;text-transform:uppercase;
letter-spacing:.04em}
td:nth-child(n+2){font-variant-numeric:tabular-nums}
strong{color:#fff}
em{color:var(--dim)}
"""


def inline(s: str) -> str:
    """Escape, then re-introduce the inline markup. Order matters: code first,
    so bold or italic inside a code span is left alone."""
    out, last = [], 0
    for m in re.finditer(r"`([^`]+)`", s):
        out.append(_emphasis(html.escape(s[last:m.start()])))
        out.append(f"<code>{html.escape(m.group(1))}</code>")
        last = m.end()
    out.append(_emphasis(html.escape(s[last:])))
    return "".join(out)


def _emphasis(s: str) -> str:
    s = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", s)
    return re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", s)


def render(md: str) -> tuple[str, str, str]:
    lines = md.split("\n")
    title, subtitle, body, i = "", "", [], 0

    while i < len(lines):
        line = lines[i]

        if line.startswith("```"):
            block, i = [], i + 1
            while i < len(lines) and not lines[i].startswith("```"):
                block.append(lines[i])
                i += 1
            body.append(f"<pre><code>{html.escape(chr(10).join(block))}</code></pre>")

        elif line.startswith("# ") and not title:
            title = line[2:].strip()
            # A lone paragraph under the title becomes the standfirst.
            if i + 2 < len(lines) and lines[i + 1] == "" and lines[i + 2].strip():
                j = i + 2
                sub = []
                while j < len(lines) and lines[j].strip():
                    sub.append(lines[j].strip())
                    j += 1
                subtitle, i = inline(" ".join(sub)), j - 1

        # A second H1. Without this the paragraph branch below takes it, and
        # its loop refuses to consume a line starting with "#", so `i` never
        # advances and render() spins forever. Found 2026-11-22 on a report
        # that embedded another document's title.
        elif line.startswith("# "):
            body.append(f"<h2>{inline(line[2:].strip())}</h2>")

        elif line.startswith("## "):
            body.append(f"<h2>{inline(line[3:].strip())}</h2>")
        elif line.startswith("### "):
            body.append(f"<h3>{inline(line[4:].strip())}</h3>")

        elif line.startswith("|"):
            rows, i = [], i
            while i < len(lines) and lines[i].startswith("|"):
                rows.append(lines[i])
                i += 1
            i -= 1
            cells = [[c.strip() for c in r.strip("|").split("|")] for r in rows]
            head, rest = cells[0], [r for r in cells[2:]]
            t = ["<table><thead><tr>"]
            t += [f"<th>{inline(c)}</th>" for c in head]
            t.append("</tr></thead><tbody>")
            for r in rest:
                t.append("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>")
            t.append("</tbody></table>")
            body.append("".join(t))

        elif line.startswith("- "):
            items, i = [], i
            while i < len(lines) and (lines[i].startswith("- ") or lines[i].startswith("  ")):
                if lines[i].startswith("- "):
                    items.append(lines[i][2:].strip())
                elif items:
                    items[-1] += " " + lines[i].strip()
                i += 1
            i -= 1
            body.append("<ul>" + "".join(f"<li>{inline(x)}</li>" for x in items) + "</ul>")

        elif line.strip():
            para, i = [], i
            while i < len(lines) and lines[i].strip() and not lines[i].startswith(
                ("#", "|", "- ", "```")
            ):
                para.append(lines[i].strip())
                i += 1
            # Backstop against the same hang from any future construct: this
            # branch must always consume the line that entered it.
            if not para:
                para.append(line.strip())
                i += 1
            i -= 1
            body.append(f"<p>{inline(' '.join(para))}</p>")

        i += 1

    return title, subtitle, "\n".join(body)


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    src = Path(sys.argv[1])
    title, subtitle, body = render(src.read_text())
    out = (
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n"
        "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n"
        f"<title>{html.escape(title)}</title>\n<style>{CSS}</style></head>\n<body><main>\n"
        f"<h1>{html.escape(title)}</h1>\n"
        + (f"<p class=\"sub\">{subtitle}</p>\n" if subtitle else "")
        + body
        + "\n</main></body></html>\n"
    )
    dst = src.with_suffix(".html")
    dst.write_text(out)
    print(f"{src} -> {dst}  ({len(out):,} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
