#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

DEFAULT_WALLET="55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr"
WALLET="${1:-${DEFAULT_WALLET}}"

TRADES_SOURCE="${TRADES_SOURCE:-bitquery}"
KLINE_SOURCE="${KLINE_SOURCE:-gmgn}"
DAYS="${DAYS:-30}"
LIMIT="${LIMIT:-2000}"
PROFIT_SAMPLES="${PROFIT_SAMPLES:-50}"
LOSS_SAMPLES="${LOSS_SAMPLES:-50}"
RESOLUTION="${RESOLUTION:-1m}"
PRE_MINUTES="${PRE_MINUTES:-20}"
POST_MINUTES="${POST_MINUTES:-20}"
MIN_MATCHES="${MIN_MATCHES:-8}"
TOP="${TOP:-25}"
OUT_DIR="${OUT_DIR:-data/strategy_research/wallets/${WALLET}}"

if [[ "${TRADES_SOURCE}" == "bitquery" && -z "${BITQUERY_TOKEN:-}" ]]; then
  echo "BITQUERY_TOKEN is required when TRADES_SOURCE=bitquery." >&2
  echo "Run: export BITQUERY_TOKEN=\"your Bitquery access token\"" >&2
  exit 1
fi

if [[ "${KLINE_SOURCE}" == "gmgn" && -z "${GMGN_API_KEY:-}" ]]; then
  echo "GMGN_API_KEY is required when KLINE_SOURCE=gmgn." >&2
  echo "Run: export GMGN_API_KEY=\"your GMGN API key\"" >&2
  exit 1
fi

echo "== Strategy replication research =="
echo "wallet: ${WALLET}"
echo "out: ${OUT_DIR}"
echo

echo "== Step 1/3: build dataset =="
cargo run -- build-strategy-dataset "${WALLET}" \
  --trades-source "${TRADES_SOURCE}" \
  --kline-source "${KLINE_SOURCE}" \
  --days "${DAYS}" \
  --limit "${LIMIT}" \
  --profit-samples "${PROFIT_SAMPLES}" \
  --loss-samples "${LOSS_SAMPLES}" \
  --resolution "${RESOLUTION}" \
  --pre-minutes "${PRE_MINUTES}" \
  --post-minutes "${POST_MINUTES}" \
  --out "${OUT_DIR}"

echo
echo "== Step 2/3: extract features =="
cargo run -- extract-strategy-features \
  --dataset "${OUT_DIR}"

echo
echo "== Step 3/3: generate rule candidates =="
cargo run -- generate-rule-candidates \
  --dataset "${OUT_DIR}" \
  --min-matches "${MIN_MATCHES}" \
  --top "${TOP}"

echo
echo "== Done =="
echo "Read:"
echo "- ${OUT_DIR}/reports/dataset_summary.md"
echo "- ${OUT_DIR}/reports/feature_comparison.md"
echo "- ${OUT_DIR}/reports/rule_candidates.md"
