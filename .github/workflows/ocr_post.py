"""Post an OpenCodeReview result to the trigger PR and gate on findings.

Reads the JSON emitted by `ocr review --format json`, posts the findings
as a PR review (or a short clean note so review-wait tooling sees a
review), and exits non-zero when findings exist so the check blocks
merge until they are addressed.
"""
import json
import os
import subprocess
import sys


def build_body(result: dict, sha: str) -> str:
    comments = result.get("comments") or []
    if result.get("status") != "success":
        return f"OpenCodeReview @ {sha}: run status `{result.get('status')}`"

    lines: list[str] = []
    if not comments:
        return f"OpenCodeReview @ {sha}: clean, no findings"
    lines = [
        f"OpenCodeReview @ {sha}: {len(comments)} finding(s)", "",
        "| severity | location | finding |", "|---|---|---|",
    ]
    for c in comments:
        loc = c.get("path", "?")
        if c.get("start_line"):
            loc += ':' + str(c.get('start_line'))
        finding = c.get("content", "").replace("\n", " ")
        lines.append(f"| {c.get('severity', '?')} | `{loc}` | {finding} |")
    lines.append("")
    for c in comments:
        suggestion = (c.get("suggestion_code") or "").strip()
        if not suggestion:
            continue
        loc = f"{c.get('path', '?')}:{c.get('start_line', '?')}"
        lines += [f"<details><summary><code>{loc}</code></summary>", "",
                  "```", suggestion, "```", "", "</details>"]
    return "\n".join(lines)


def main() -> int:
    with open(sys.argv[1]) as f:
        result = json.load(f)
    pr = os.environ["PR_NUMBER"]
    sha = os.environ["PR_SHA"][:10]
    body = build_body(result, sha)
    with open("ocr-body.md", "w") as f:
        f.write(body)
    subprocess.run(
        ["gh", "pr", "review", pr, "--comment", "--body-file", "ocr-body.md"],
        check=True,
    )
    return 1 if result.get("comments") else 0


if __name__ == "__main__":
    sys.exit(main())
