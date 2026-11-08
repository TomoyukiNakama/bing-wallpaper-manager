#!/usr/bin/env python3
"""使用しているライブラリの一覧を生成する。

Linux 向けビルドに実際に含まれる依存（通常の依存とビルド依存。開発用の依存は除く）を
`cargo metadata` からたどり、次の 2 つを書き出す。

  src/third_party.tsv        アプリの「このアプリについて」で表示する一覧（名前・バージョン・ライセンス・URL）
  THIRD_PARTY_LICENSES.md    同じ一覧を人が読める形にしたもの

依存を追加・更新したら実行し直す:  python3 scripts/gen-third-party.py

ビルド済みのバイナリを配るときは、各ライブラリのライセンス本文も同梱する:
  python3 scripts/gen-third-party.py --texts OUT.txt
各クレートに含まれる LICENSE / LICENCE / COPYING / NOTICE ファイルの本文をまとめて OUT.txt に書く。
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-unknown-linux-gnu"


def main() -> None:
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]

    # 開発用の依存を除いて、ルートからたどれるパッケージを集める
    seen, stack = set(), [root]
    while stack:
        node = nodes[stack.pop()]
        for dep in node["deps"]:
            kinds = {k["kind"] for k in dep["dep_kinds"]}
            if kinds <= {"dev"}:
                continue
            if dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                stack.append(dep["pkg"])

    if "--texts" in sys.argv:
        write_texts(Path(sys.argv[sys.argv.index("--texts") + 1]), [packages[pid] for pid in seen])
        return

    rows = []
    for pid in seen:
        p = packages[pid]
        url = p.get("repository") or p.get("homepage") or f"https://crates.io/crates/{p['name']}"
        rows.append((p["name"], p["version"], p.get("license") or "unknown", url))
    rows.sort(key=lambda r: (r[0].lower(), r[1]))

    tsv = "".join("\t".join(r) + "\n" for r in rows)
    (ROOT / "src" / "third_party.tsv").write_text(tsv)

    md = [
        "# Third-party libraries",
        "",
        f"This application uses the following {len(rows)} Rust crates.",
        "Each crate is distributed under the license shown; see its repository for the full license text.",
        "",
        "| Crate | Version | License | Repository |",
        "| --- | --- | --- | --- |",
    ]
    md += [f"| {n} | {v} | {l} | {u} |" for n, v, l, u in rows]
    (ROOT / "THIRD_PARTY_LICENSES.md").write_text("\n".join(md) + "\n")
    print(f"{len(rows)} crates", file=sys.stderr)


MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)", re.IGNORECASE)


def write_texts(out: Path, packages: list) -> None:
    """各クレートのライセンスファイルの本文を 1 つのテキストにまとめる。"""
    packages = sorted(packages, key=lambda p: (p["name"].lower(), p["version"]))
    parts = [
        "Third-party license texts for Bing Wallpaper Manager",
        "=" * 52,
        "",
        "This file contains the license files shipped with each Rust crate compiled into this program.",
        "",
    ]
    missing = 0
    for p in packages:
        crate_dir = Path(p["manifest_path"]).parent
        files = sorted(f for f in crate_dir.iterdir() if f.is_file() and LICENSE_FILE.match(f.name))
        parts += ["", "-" * 78, f"{p['name']} {p['version']} ({p.get('license') or 'unknown'})", "-" * 78]
        if not files:
            missing += 1
            parts.append("(This crate does not include a license file; see its license expression above.)")
        for f in files:
            parts += ["", f"--- {f.name} ---", f.read_text(errors="replace").rstrip()]
    # ライセンスファイルを含まないクレート（MIT / Apache-2.0 のもの）のために、標準の本文を付ける
    parts += [
        "",
        "=" * 78,
        "Standard license texts",
        "=" * 78,
        "",
        "Crates above that do not include a license file are distributed under the MIT License and/or",
        "the Apache License 2.0; the standard texts of both licenses follow.",
        "",
        "--- MIT License ---",
        MIT_TEXT,
        "",
        "--- Apache License 2.0 ---",
        (ROOT / "LICENSE-APACHE").read_text().rstrip(),
    ]
    out.write_text("\n".join(parts) + "\n")
    print(f"{len(packages)} crates, {missing} without license files -> {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
