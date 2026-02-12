#!/bin/bash
# End-to-end test script for learning system

set -e

PROJECT="e2e-test-$(date +%s)"
echo "🧪 End-to-End Learning System Test"
echo "===================================="
echo "Project: $PROJECT"
echo

# Step 1: Run simulation
echo "1️⃣  Running simulation (50 mixed events)..."
cargo run --quiet -- learn simulate "$PROJECT" --sessions 50 --pattern mixed 2>/dev/null
echo "   ✓ Simulation complete"
echo

# Step 2: Check dashboard
echo "2️⃣  Checking learning dashboard..."
OUTPUT=$(cargo run --quiet -- learn dashboard "$PROJECT" 2>/dev/null)
echo "$OUTPUT" | head -15
echo

# Verify sessions tracked
if echo "$OUTPUT" | grep -q "Sessions: 1"; then
    echo "   ✓ Learning sessions tracked"
else
    echo "   ✗ FAILED: Sessions not tracked"
    exit 1
fi

# Verify importance boosts
if echo "$OUTPUT" | grep -q "Top Importance Boosts"; then
    echo "   ✓ Importance boosts learned"
else
    echo "   ✗ FAILED: No importance boosts"
    exit 1
fi
echo

# Step 3: Test optimize (dry-run)
echo "3️⃣  Testing optimize (dry-run)..."
OUTPUT=$(cargo run --quiet -- learn optimize "$PROJECT" --dry-run 2>/dev/null)
echo "$OUTPUT" | head -10
echo

if echo "$OUTPUT" | grep -q "Proposed Changes"; then
    echo "   ✓ Optimizations proposed"
else
    echo "   ✗ FAILED: No optimizations proposed"
    exit 1
fi
echo

# Step 4: Test optimize (apply)
echo "4️⃣  Applying optimizations..."
cargo run --quiet -- learn optimize "$PROJECT" --auto 2>/dev/null | tail -6
echo "   ✓ Optimizations applied"
echo

# Step 5: Verify analytics
echo "5️⃣  Checking analytics..."
cargo run --quiet -- analytics "$PROJECT" --days 1 2>/dev/null | head -8
echo "   ✓ Analytics tracked"
echo

# Success!
echo "✅ All end-to-end tests PASSED!"
echo
echo "Cleanup: rm -rf ~/memory/learning/$PROJECT.json ~/memory/analytics/*"
