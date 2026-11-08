#!/usr/bin/env python3
"""使用しているライブラリの一覧を生成する。

Linux 向けビルドに実際に含まれる依存（通常の依存とビルド依存。開発用の依存は除く）を
`cargo metadata` からたどり、次の 2 つを書き出す。

  src/third_party.tsv        アプリの「このアプリについて」で表示する一覧（名前・バージョン・ライセンス・URL）
  THIRD_PARTY_LICENSES.md    同じ一覧を人が読める形にしたもの

依存を追加・更新したら実行し直す:  python3 scripts/gen-third-party.py
"""

import json
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


if __name__ == "__main__":
    main()
