# Paired diff of F3 slice-4 audit dumps vs stashed baseline dumps.
import json
import sys


def load(pattern: str) -> tuple[dict, dict]:
    total: dict[str, int] = {}
    ex_keys: dict[str, int] = {}
    import glob

    for f in sorted(glob.glob(pattern)):
        d = json.load(open(f))
        for k, v in d["counters"].items():
            total[k] = total.get(k, 0) + v
        for k, v in d["examples"].items():
            if isinstance(v, dict):
                ex_keys[k] = ex_keys.get(k, 0) + len(v)
            elif isinstance(v, int):
                ex_keys[k] = ex_keys.get(k, 0) + v
    return total, ex_keys


off, exoff = load(sys.argv[1])
on, exon = load(sys.argv[2])
print("examples off:", exoff)
print("examples on:", exon)
print(f"{'counter':40} {'off':>12} {'on':>12} {'delta':>10} {'%':>7}")
for k in sorted(set(off) | set(on)):
    o, n = off.get(k, 0), on.get(k, 0)
    pct = f"{(n - o) / o * 100:+6.2f}%" if o else ("  n/a" if n else "")
    print(f"{k:40} {o:12d} {n:12d} {n - o:10d} {pct}")
