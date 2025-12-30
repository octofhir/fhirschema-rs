#!/bin/bash
#
# Performance profiling scripts for FHIR validation
#
# Dependencies:
#   cargo install flamegraph
#   cargo install cargo-instruments  # for macOS
#
# Usage:
#   ./scripts/profile.sh flamegraph    # create flamegraph
#   ./scripts/profile.sh bench         # run benchmarks
#   ./scripts/profile.sh compare       # compare with baseline

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

case "${1:-bench}" in
    bench)
        echo "=== Running benchmarks ==="
        cargo bench --bench validation_bench
        ;;

    flamegraph)
        echo "=== Creating flamegraph ==="
        echo "Profiling validate_bundle..."

        # macOS requires sudo or dtrace permissions
        if [[ "$OSTYPE" == "darwin"* ]]; then
            echo "macOS: using cargo instruments or dtrace"
            echo "Alternative: cargo flamegraph --bench validation_bench -- --bench validate_bundle"

            # Try cargo-instruments if installed
            if command -v cargo-instruments &> /dev/null; then
                cargo instruments --bench validation_bench -t "Time Profiler" -- --bench validate_bundle
            else
                echo "Install: cargo install cargo-instruments"
                echo "Or use: sudo cargo flamegraph --bench validation_bench"
            fi
        else
            # Linux: use perf
            cargo flamegraph --bench validation_bench -- --bench validate_bundle/100
        fi
        ;;

    compare)
        echo "=== Comparing with baseline ==="
        # Save baseline if it doesn't exist
        if [[ ! -d "target/criterion/baseline" ]]; then
            echo "Creating baseline..."
            cargo bench --bench validation_bench -- --save-baseline baseline
        fi

        # Compare with baseline
        cargo bench --bench validation_bench -- --baseline baseline
        ;;

    quick)
        echo "=== Quick benchmark (patient only) ==="
        cargo bench --bench validation_bench -- validate_patient
        ;;

    throughput)
        echo "=== Measuring throughput ==="
        cargo bench --bench validation_bench -- throughput
        ;;

    report)
        echo "=== Opening Criterion report ==="
        open target/criterion/report/index.html 2>/dev/null || \
        xdg-open target/criterion/report/index.html 2>/dev/null || \
        echo "Report: target/criterion/report/index.html"
        ;;

    *)
        echo "Usage: $0 {bench|flamegraph|compare|quick|throughput|report}"
        echo ""
        echo "  bench      - run full benchmark suite"
        echo "  flamegraph - create flamegraph for profiling"
        echo "  compare    - compare with saved baseline"
        echo "  quick      - run Patient benchmark only"
        echo "  throughput - measure RPS"
        echo "  report     - open HTML report"
        exit 1
        ;;
esac
