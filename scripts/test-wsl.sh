#!/usr/bin/env bash
# ==============================================================================
# Ferrite WSL / Linux Deterministic Toolchain & Verification Script
# Runs 100% offline using ReplayProvider and clang AddressSanitizer/UBSan.
# ==============================================================================

set -eo pipefail

# Ensure ~/.cargo/bin is present in PATH
if [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi
if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================================================${NC}"
echo -e "${BLUE}          FERRITE DETERMINISTIC VERIFICATION & ASAN SUITE               ${NC}"
echo -e "${BLUE}========================================================================${NC}"

# 1. Environment & Toolchain Diagnostics
echo -e "\n${YELLOW}[1/4] Checking Linux / WSL Toolchain...${NC}"
MISSING=0
for tool in cargo rustc clang; do
    if command -v "$tool" >/dev/null 2>&1; then
        echo -e "  [✔] $tool: $(command -v "$tool")"
    else
        echo -e "  ${RED}[✘] Missing required compiler tool: $tool${NC}"
        MISSING=1
    fi
done

# Check optional LLM/LLVM tools
for opt_tool in llvm-config clang++ llvm-symbolizer; do
    if command -v "$opt_tool" >/dev/null 2>&1; then
        echo -e "  [✔] $opt_tool: $(command -v "$opt_tool")"
    else
        echo -e "  [i] $opt_tool: (optional, not found in PATH)"
    fi
done

if [ "$MISSING" -ne 0 ]; then
    echo -e "${RED}Error: Required toolchain binaries missing. Please install rust and clang.${NC}"
    exit 1
fi

echo -e "  Rust target: $(rustc -vV | grep 'host:' | cut -d' ' -f2)"
echo -e "  Clang version: $(clang --version | head -n1)"

# 2. Static Analysis & Clippy Linter Check
echo -e "\n${YELLOW}[2/4] Running Clippy static analysis...${NC}"
cargo clippy --all-targets -- -D warnings
echo -e "${GREEN}  [✔] Clippy checks clean (0 warnings)${NC}"

# 3. Unit and Integration Tests (including Tri-Binary ASan triage)
echo -e "\n${YELLOW}[3/4] Running Unit and Integration Test Suite...${NC}"
cargo test -- --nocapture
echo -e "${GREEN}  [✔] All unit & integration tests passed${NC}"

# 4. Offline Benchmark Suite (Ferrite-Bench Lite across 34 functions)
echo -e "\n${YELLOW}[4/4] Executing Ferrite-Bench Lite Manifest in Replay Mode...${NC}"
cargo run -- bench --manifest bench/manifest.toml --provider replay

echo -e "\n${GREEN}========================================================================${NC}"
echo -e "${GREEN}       ALL WSL DETERMINISTIC VERIFICATION CHECKS COMPLETED SUCCESSFULLY  ${NC}"
echo -e "${GREEN}========================================================================${NC}"
