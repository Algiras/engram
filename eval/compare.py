#!/usr/bin/env python3
"""Compare multiple eval result JSON files."""
import json, sys
from pathlib import Path

files = sorted(sys.argv[1:])
if not files:
    # Auto-discover
    files = sorted(Path("eval/results").glob("run_*.json"))
    files += [
        "eval/domain_results_v1.json",
        "eval/domain_results_ceiling.json",
        "eval/domain_results_v3_full.json",
    ]
    files = [str(f) for f in files if Path(f).exists()]

if not files:
    print("No result files found. Run: python eval/run_eval.sh first")
    sys.exit(1)

results = []
for f in files:
    try:
        data = json.loads(Path(f).read_text())
        name = Path(f).stem
        results.append({
            "name": name[:35],
            "f1": data.get("overall_f1", 0),
            "judge": data.get("overall_judge", 0),
            "not_found": data.get("not_found_rate", 0),
            "label": data.get("label", ""),
        })
    except Exception as e:
        print(f"  skip {f}: {e}")

if not results:
    print("No valid results found.")
    sys.exit(1)

# Sort by judge score
results.sort(key=lambda x: x["judge"])

max_judge = max(r["judge"] for r in results)
ceiling = 14.0

print()
print("╔════════════════════════════════════════════════════════════╗")
print("  engram Eval Comparison")
print(f"  {len(results)} runs  |  ceiling: {ceiling:.1f} judge")
print("╠════════════════════════════════════════════════════════════╣")
print(f"  {'Run':<38} {'F1':>6}  {'Judge':>6}  {'!Found':>6}  chart")
print("  " + "─" * 62)

for r in results:
    bar = "█" * max(1, int(r["judge"] / ceiling * 20))
    marker = " ★" if r["judge"] == max_judge else ""
    print(f"  {r['name']:<38} {r['f1']:6.1f}  {r['judge']:6.1f}  "
          f"{r['not_found']:5.1f}%  {bar}{marker}")

best = results[-1]
worst = results[0]
print()
print(f"  Best: {best['name']}  judge={best['judge']:.1f}  "
      f"({best['judge']/ceiling*100:.0f}% of ceiling)")
if len(results) > 1:
    delta_j = best["judge"] - worst["judge"]
    delta_f1 = best["f1"] - worst["f1"]
    print(f"  Improvement over baseline: +{delta_f1:.1f} F1  +{delta_j:.1f} judge")
print("╚════════════════════════════════════════════════════════════╝")
