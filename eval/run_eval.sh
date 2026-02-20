#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="eval/results"
OUTPUT="$RESULTS_DIR/run_${TIMESTAMP}.json"
PREV=$(ls "$RESULTS_DIR"/run_*.json 2>/dev/null | sort | tail -1 || true)

mkdir -p "$RESULTS_DIR"

echo "━━━ engram eval run $TIMESTAMP ━━━"
echo ""

# Build
echo "Building release binary..."
cargo build --release --quiet
echo "  ✓ Binary built"
echo ""

# Rebuild embedding index (pick up latest knowledge)
echo "Rebuilding embedding index..."
./target/release/engram embed Personal --provider gemini 2>/dev/null | grep "Total chunks"
echo ""

# Run eval
echo "Running domain eval..."
PREV_ARG=""
if [ -n "$PREV" ]; then
  PREV_ARG="--prev $PREV"
fi

python3 eval/engram_eval.py \
  --project Personal \
  --dataset eval/qa_dataset.json \
  --engram ./target/release/engram \
  --use-judge \
  --output "$OUTPUT" \
  $PREV_ARG

echo ""
echo "Results saved: $OUTPUT"

# CI gate: fail if judge drops > 2.5 vs previous
# Note: binary judge (0/1 per question) has ~2.3 pts natural run-to-run variance;
# gate of 2.5 catches real regressions while ignoring noise.
if [ -n "$PREV" ]; then
  PREV_JUDGE=$(python3 -c "import json; print(json.load(open('$PREV'))['overall_judge'])" 2>/dev/null || echo "0")
  CURR_JUDGE=$(python3 -c "import json; print(json.load(open('$OUTPUT'))['overall_judge'])" 2>/dev/null || echo "0")
  DELTA=$(python3 -c "print(f'{float(\"$CURR_JUDGE\") - float(\"$PREV_JUDGE\"):.2f}')" 2>/dev/null || echo "0")
  echo ""
  echo "Judge delta vs previous: $DELTA"
  REGRESSION=$(python3 -c "print('yes' if float('$CURR_JUDGE') < float('$PREV_JUDGE') - 2.5 else 'no')" 2>/dev/null || echo "no")
  if [ "$REGRESSION" = "yes" ]; then
    echo "❌ REGRESSION: judge dropped more than 2.5 vs previous run"
    exit 1
  fi
  echo "✓ No regression detected"
fi
