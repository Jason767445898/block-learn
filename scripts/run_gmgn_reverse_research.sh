#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

WALLET="${1:-sample-gmgn-wallet}"
OUT_DIR="${OUT_DIR:-data/gmgn_reverse/wallets/${WALLET}}"
WALLET_TRADES_SOURCE="${WALLET_TRADES_SOURCE:-sample}"
ACTIVITY_SOURCE="${ACTIVITY_SOURCE:-sample}"
DAYS="${DAYS:-30}"
PROFIT_SAMPLES="${PROFIT_SAMPLES:-50}"
LOSS_SAMPLES="${LOSS_SAMPLES:-50}"
PRE_MINUTES="${PRE_MINUTES:-60}"
POST_MINUTES="${POST_MINUTES:-60}"
MIN_MATCHES="${MIN_MATCHES:-6}"
TOP="${TOP:-25}"
LARGE_TRADE_SOL_THRESHOLD="${LARGE_TRADE_SOL_THRESHOLD:-2.0}"

BUILD_ARGS=(
  "${WALLET}"
  --wallet-trades-source "${WALLET_TRADES_SOURCE}"
  --activity-source "${ACTIVITY_SOURCE}"
  --days "${DAYS}"
  --profit-samples "${PROFIT_SAMPLES}"
  --loss-samples "${LOSS_SAMPLES}"
  --pre-minutes "${PRE_MINUTES}"
  --post-minutes "${POST_MINUTES}"
  --large-trade-sol-threshold "${LARGE_TRADE_SOL_THRESHOLD}"
  --out "${OUT_DIR}"
)

if [[ -n "${WALLET_TRADES_FILE:-}" ]]; then
  BUILD_ARGS+=(--wallet-trades-file "${WALLET_TRADES_FILE}")
fi

if [[ -n "${ACTIVITY_DIR:-}" ]]; then
  BUILD_ARGS+=(--activity-dir "${ACTIVITY_DIR}")
fi

echo "== GMGN reverse research =="
echo "wallet: ${WALLET}"
echo "out: ${OUT_DIR}"
echo "wallet trades source: ${WALLET_TRADES_SOURCE}"
echo "activity source: ${ACTIVITY_SOURCE}"
echo "Bitquery API: not used"
echo

echo "== Step 1/3: build dataset =="
cargo run -- gmgn-reverse build "${BUILD_ARGS[@]}"

echo
echo "== Step 2/3: extract activity features =="
cargo run -- gmgn-reverse extract --dataset "${OUT_DIR}"

echo
echo "== Step 3/3: generate activity rules =="
cargo run -- gmgn-reverse rules --dataset "${OUT_DIR}" --min-matches "${MIN_MATCHES}" --top "${TOP}"

echo
echo "== Done =="
echo "Read:"
echo "- ${OUT_DIR}/reports/dataset_summary.md"
echo "- ${OUT_DIR}/reports/activity_feature_comparison.md"
echo "- ${OUT_DIR}/reports/activity_rule_candidates.md"
echo "- ${OUT_DIR}/reports/gmgn_reverse_analysis_report.md"
