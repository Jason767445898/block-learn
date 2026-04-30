# GMGN Rule Replay Against Wallet Trades

## Scope

- wallet: `55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr`
- positions replayed: 100
- data: actual wallet positions from CSV + 1m OHLCV klines
- entry replay uses `position_features.csv`.
- sell replay uses the entry candle close as simulated buy price, because wallet average buy price and kline price are in different units/scales.
- limitation: the sell simulation is minute-level. It cannot exactly reproduce 15s/30s decisions, intra-candle order, multiple buys/sells, or re-entry chains.

## Entry Rule Replay

| rule | matches | coverage | wins | losses | win rate | avg ROI | median hold |
|---|---:|---:|---:|---:|---:|---:|---:|
| base: pre_5m_return >= 25% AND range >= 0.75 | 18 | 18.0% | 16 | 2 | 88.9% | 34.33% | 78s |
| with volume: base AND volume_spike >= 1.5x | 10 | 10.0% | 9 | 1 | 90.0% | 33.11% | 82s |

- baseline over all 100 positions: win rate 50.0%
- Interpretation: the entry filter captures only a minority of the wallet buys, but it selects a much stronger subset than the full wallet history.

## Sell Rule Replay

The sell replay starts from the wallet actual first buy time, then applies the playbook exit rules to 1m candles.

| subset | count | same P/L sign | within 60s | within 120s | median abs time diff | avg abs ROI diff | avg ROI diff |
|---|---:|---:|---:|---:|---:|---:|---:|
| all positions | 100 | 47/100 | 67 | 89 | 40s | 43.18% | 17.16% |
| entry base matches | 18 | 13/18 | 10 | 15 | 52s | 34.30% | -9.31% |
| entry + volume matches | 10 | 7/10 | 5 | 7 | 61s | 33.53% | -23.36% |

### Simulated Exit Reasons

| reason | all positions | entry base matches |
|---|---:|---:|
| `explosive_runner_trail` | 4 | 2 |
| `failed_close_1m` | 19 | 3 |
| `normal_trailing_stop` | 23 | 6 |
| `strong_hard_tp_60pct` | 27 | 0 |
| `strong_trailing_stop` | 14 | 4 |
| `time_300s` | 1 | 0 |
| `weak_no_15pct_by_1m` | 12 | 3 |

## Biggest Time Mismatches

| mint | actual exit | simulated exit | diff | actual ROI | simulated ROI | reason | entry match |
|---|---|---|---:|---:|---:|---|---:|
| `4eoAmGt4...` | 2026-04-29T09:51:07+00:00 | 2026-04-29T09:44:00+00:00 | -427s | -6.37% | 17.16% | `normal_trailing_stop` | false |
| `G76fNn6Q...` | 2026-04-29T01:44:49+00:00 | 2026-04-29T01:39:00+00:00 | -349s | 135.70% | 15.00% | `normal_trailing_stop` | true |
| `DZQKJPbk...` | 2026-04-29T09:21:39+00:00 | 2026-04-29T09:25:58+00:00 | 259s | -1.25% | -1.06% | `time_300s` | false |
| `2uRdYXZc...` | 2026-04-29T04:24:09+00:00 | 2026-04-29T04:20:00+00:00 | -249s | 11.13% | 12.91% | `weak_no_15pct_by_1m` | false |
| `HS1Vh4K1...` | 2026-04-29T09:30:19+00:00 | 2026-04-29T09:27:00+00:00 | -199s | 54.39% | 31.75% | `strong_trailing_stop` | true |
| `81n9oxHu...` | 2026-04-29T04:00:15+00:00 | 2026-04-29T03:57:00+00:00 | -195s | -3.57% | 23.48% | `normal_trailing_stop` | false |
| `3H3kqqdF...` | 2026-04-29T03:12:36+00:00 | 2026-04-29T03:10:00+00:00 | -156s | -19.30% | 60.00% | `strong_hard_tp_60pct` | false |
| `6sN3E4Dk...` | 2026-04-29T01:56:25+00:00 | 2026-04-29T01:54:00+00:00 | -145s | 4.16% | -1.62% | `weak_no_15pct_by_1m` | false |
| `8TwGaAgq...` | 2026-04-29T03:33:23+00:00 | 2026-04-29T03:31:00+00:00 | -143s | 4.71% | 60.00% | `strong_hard_tp_60pct` | false |
| `9ch1JqNx...` | 2026-04-29T02:26:43+00:00 | 2026-04-29T02:29:00+00:00 | 137s | 2.01% | 1.10% | `weak_no_15pct_by_1m` | false |

## Biggest ROI Mismatches

| mint | actual ROI | simulated ROI | diff | actual hold | simulated reason | entry match |
|---|---:|---:|---:|---:|---|---:|
| `3pbggCjS...` | 167.30% | -79.48% | -246.78% | 71s | `failed_close_1m` | false |
| `BP37KtWx...` | 182.37% | -19.26% | -201.63% | 76s | `failed_close_1m` | false |
| `2Ku7NnBc...` | 1.30% | 146.23% | 144.93% | 15s | `explosive_runner_trail` | true |
| `G76fNn6Q...` | 135.70% | 15.00% | -120.70% | 358s | `normal_trailing_stop` | true |
| `7wKewyb1...` | -52.29% | 60.00% | 112.29% | 57s | `strong_hard_tp_60pct` | false |
| `5xL9HT3f...` | -46.49% | 60.00% | 106.49% | 16s | `strong_hard_tp_60pct` | false |
| `6i7VSyA3...` | 47.64% | -53.88% | -101.52% | 41s | `failed_close_1m` | false |
| `BX4eMWGB...` | -40.43% | 60.00% | 100.43% | 22s | `strong_hard_tp_60pct` | false |
| `3Ky1XiMU...` | -34.96% | 60.00% | 94.96% | 8s | `strong_hard_tp_60pct` | false |
| `JEGXcUid...` | -34.03% | 60.00% | 94.03% | 12s | `strong_hard_tp_60pct` | false |

## Good Trades Missed By Entry Rule

These were real wallet trades with ROI >= 50% but did not satisfy the base entry rule.

| mint | ROI | pre 5m return | range position | volume spike | entry label |
|---|---:|---:|---:|---:|---|
| `BP37KtWx...` | 182.37% |  | 0.904 |  | `high_range_entry` |
| `BDDumgWw...` | 180.78% |  | 0.725 |  | `unclear_entry` |
| `3pbggCjS...` | 167.30% |  | 0.898 |  | `high_range_entry` |
| `524zXrkb...` | 98.06% |  | 1.000 |  | `high_range_entry` |
| `9EZW54MX...` | 58.36% |  | 0.702 |  | `unclear_entry` |
| `H6SBg5q5...` | 50.40% |  | 0.996 |  | `high_range_entry` |

## Bad Trades Passed By Entry Rule

These passed the base entry filter but still lost more than 20%.

| mint | ROI | pre 5m return | range position | volume spike | entry label |
|---|---:|---:|---:|---:|---|

## Conclusion

- The entry rule reverse-engineers a high-quality subset of this wallet behavior, not the whole wallet behavior.
- The base entry rule is selective: it catches about one fifth of actual wallet buys, but that subset has much higher win rate and ROI than the full sample.
- The sell playbook is only partially validated on 1m candles. It is close enough to test directionally, but not enough to claim exact reverse engineering of the wallet exits.
- The biggest remaining gap is data granularity: to validate the sell rules properly, collect trade-level wallet swaps with exact buy/sell timestamps, side, token amount, SOL amount, and execution price.
