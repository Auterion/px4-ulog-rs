#!/bin/bash
#
# Compare px4-ulog-rs streaming parser performance against pyulog.
# Runs both parsers on the same fixture files and prints a side-by-side table.
#
# Prerequisites:
#   - Rust toolchain (cargo)
#   - Python 3 with pyulog: pip install pyulog
#
# Usage:
#   ./scripts/benchmark_compare.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_DIR"

FIXTURES=(
    "tests/fixtures/sample.ulg"
    "tests/fixtures/quadrotor_local.ulg"
    "tests/fixtures/fixed_wing_gps.ulg"
    "tests/fixtures/vtol_demo.ulg"
    "tests/fixtures/truncated_real.ulg"
    "tests/fixtures/sample_appended.ulg"
)

# Check prerequisites
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found. Install Rust via https://rustup.rs"
    exit 1
fi

if ! python3 -c "from pyulog import ULog" 2>/dev/null; then
    echo "Error: pyulog not found. Install with: pip install pyulog"
    exit 1
fi

# Verify fixture files exist
for f in "${FIXTURES[@]}"; do
    if [ ! -f "$f" ]; then
        echo "Error: fixture file not found: $f"
        exit 1
    fi
done

echo "Building px4-ulog-rs (release)..."
cargo build --release --example bench 2>/dev/null

echo ""
echo "=== px4-ulog-rs (Rust, streaming parser, 10 iterations) ==="
echo ""
cargo run --release --example bench 2>/dev/null

echo ""
echo "=== pyulog (Python, 5 iterations) ==="
echo ""

python3 - "${FIXTURES[@]}" <<'PYEOF'
import time, os, sys

files = sys.argv[1:]

print(f"{'File':<45} {'Size':>8} {'Time(ms)':>10} {'MB/s':>12}")
print("-" * 80)

total_bytes = 0
total_time = 0

from pyulog import ULog

for path in files:
    if not os.path.exists(path):
        continue
    size = os.path.getsize(path)
    size_mb = size / 1024 / 1024

    # Warmup
    for _ in range(2):
        ULog(path)

    # Measure
    times = []
    for _ in range(5):
        start = time.perf_counter()
        ULog(path)
        elapsed = time.perf_counter() - start
        times.append(elapsed)

    mean = sum(times) / len(times)
    throughput = size_mb / mean

    print(f"{path:<45} {size_mb:>7.1f}M {mean*1000:>9.2f}ms {throughput:>10.1f} MB/s")
    total_bytes += size
    total_time += mean * 1000

total_mb = total_bytes / 1024 / 1024
print("-" * 80)
print(f"{'TOTAL':<45} {total_mb:>7.1f}M {total_time:>9.2f}ms {total_mb / (total_time / 1000):>10.1f} MB/s")
PYEOF
