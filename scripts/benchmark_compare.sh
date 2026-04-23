#!/bin/bash
#
# Compare px4-ulog-rs streaming parser performance against pyulog and
# PX4/ulog_cpp. Runs all three parsers on the same fixture files and prints
# side-by-side tables.
#
# Prerequisites:
#   - Rust toolchain (cargo)
#   - Python 3 with pyulog:  pip install pyulog
#   - A C++17 compiler, CMake >= 3.16, git
#
# The C++ comparison clones and builds PX4/ulog_cpp into .bench/ulog_cpp the
# first time it runs. Subsequent runs reuse the build. Pass --skip-cpp to
# opt out.
#
# Usage:
#   ./scripts/benchmark_compare.sh [--skip-cpp] [--skip-python]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_DIR"

SKIP_CPP=0
SKIP_PY=0
for arg in "$@"; do
    case "$arg" in
        --skip-cpp) SKIP_CPP=1 ;;
        --skip-python) SKIP_PY=1 ;;
        *) echo "unknown flag: $arg" >&2; exit 2 ;;
    esac
done

FIXTURES=(
    "tests/fixtures/sample.ulg"
    "tests/fixtures/quadrotor_local.ulg"
    "tests/fixtures/fixed_wing_gps.ulg"
    "tests/fixtures/vtol_demo.ulg"
    "tests/fixtures/truncated_real.ulg"
    "tests/fixtures/sample_appended.ulg"
)

# Prerequisites: rust is always required.
if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo not found. Install Rust via https://rustup.rs" >&2
    exit 1
fi

if [ "$SKIP_PY" -eq 0 ] && ! python3 -c "from pyulog import ULog" 2>/dev/null; then
    echo "Warning: pyulog not installed; skipping Python benchmark." >&2
    SKIP_PY=1
fi

for f in "${FIXTURES[@]}"; do
    [ -f "$f" ] || { echo "Error: fixture file not found: $f" >&2; exit 1; }
done

echo "Building px4-ulog-rs (release)..."
cargo build --release --example bench >/dev/null

echo
echo "=== px4-ulog-rs (Rust, streaming parser, 10 iterations) ==="
echo
cargo run --release --example bench 2>/dev/null

# ---------------------------------------------------------------------------
# ulog_cpp
# ---------------------------------------------------------------------------
if [ "$SKIP_CPP" -eq 0 ]; then
    ULOG_CPP_REPO="https://github.com/PX4/ulog_cpp.git"
    ULOG_CPP_DIR="$REPO_DIR/.bench/ulog_cpp"
    ULOG_CPP_BUILD="$ULOG_CPP_DIR/build"
    BENCH_BIN="$REPO_DIR/.bench/ulog_cpp_bench"

    if ! command -v cmake >/dev/null 2>&1 || ! command -v git >/dev/null 2>&1; then
        echo "Warning: cmake or git missing; skipping C++ benchmark." >&2
    else
        mkdir -p "$REPO_DIR/.bench"

        if [ ! -d "$ULOG_CPP_DIR/.git" ]; then
            echo "Cloning PX4/ulog_cpp into .bench/ulog_cpp..."
            git clone --depth 1 "$ULOG_CPP_REPO" "$ULOG_CPP_DIR" >/dev/null 2>&1
        fi

        if [ ! -f "$ULOG_CPP_BUILD/ulog_cpp/libulog_cpp.a" ] && \
           [ ! -f "$ULOG_CPP_BUILD/ulog_cpp/libulog_cpp.dylib" ] && \
           [ ! -f "$ULOG_CPP_BUILD/ulog_cpp/libulog_cpp.so" ]; then
            echo "Building ulog_cpp (release)..."
            cmake -S "$ULOG_CPP_DIR" -B "$ULOG_CPP_BUILD" \
                  -DCMAKE_BUILD_TYPE=Release \
                  -DULOG_CPP_BUILD_TESTS=OFF >/dev/null
            cmake --build "$ULOG_CPP_BUILD" --target ulog_cpp -j >/dev/null
        fi

        if [ ! -x "$BENCH_BIN" ] || [ "$SCRIPT_DIR/ulog_cpp_bench.cpp" -nt "$BENCH_BIN" ]; then
            echo "Building ulog_cpp_bench..."
            CXX_BIN="${CXX:-c++}"
            "$CXX_BIN" -std=c++17 -O3 -DNDEBUG \
                -I"$ULOG_CPP_DIR" \
                "$SCRIPT_DIR/ulog_cpp_bench.cpp" \
                "$ULOG_CPP_BUILD"/ulog_cpp/libulog_cpp.* \
                -o "$BENCH_BIN" 2>&1 | head -30
        fi

        if [ -x "$BENCH_BIN" ]; then
            echo
            echo "=== ulog_cpp (C++, streaming reader, 10 iterations) ==="
            echo
            "$BENCH_BIN" "${FIXTURES[@]}"
        else
            echo "Warning: ulog_cpp_bench did not build; skipping C++ results." >&2
        fi
    fi
fi

# ---------------------------------------------------------------------------
# pyulog
# ---------------------------------------------------------------------------
if [ "$SKIP_PY" -eq 0 ]; then
    echo
    echo "=== pyulog (Python, 5 iterations) ==="
    echo
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
fi
