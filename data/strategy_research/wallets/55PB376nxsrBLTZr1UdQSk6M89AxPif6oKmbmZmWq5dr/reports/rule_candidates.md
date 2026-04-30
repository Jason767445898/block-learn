# Strategy Rule Candidates

## Dataset

- positions: 100
- baseline win rate in balanced sample: 50.00%
- minimum matches per rule: 8

## Top Candidate Entry Rules

| rank | rule | matches | win rate | avg ROI | avg PnL SOL | median hold | avg runup | avg drawdown | exit efficiency |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `pre_5m_return >= 0.5000 AND entry_range_position >= 0.7500` | 10 | 90.00% | 36.48% | 0.2080 | 95 | 46.85% | -48.06% | 0.9320 |
| 2 | `pre_5m_return >= 0.1000 AND pre_5m_volume_spike >= 1.5000 AND entry_range_position >= 0.7500` | 10 | 90.00% | 33.11% | 0.1816 | 89 | 40.87% | -48.50% | 0.9342 |
| 3 | `pre_5m_return >= 0.2500 AND pre_5m_volume_spike >= 1.5000 AND entry_range_position >= 0.7500` | 10 | 90.00% | 33.11% | 0.1816 | 89 | 40.87% | -48.50% | 0.9342 |
| 4 | `pre_5m_return >= 0.2500 AND entry_range_position >= 0.7500` | 18 | 88.89% | 34.33% | 0.1977 | 81 | 41.71% | -39.13% | 0.9203 |
| 5 | `pre_5m_return >= 0.2500 AND entry_range_position >= 0.8500` | 16 | 87.50% | 27.30% | 0.1619 | 81 | 36.65% | -37.83% | 0.9232 |
| 6 | `pre_5m_return >= 0.5000 AND pre_5m_volume_spike >= 1.5000 AND entry_range_position >= 0.7500` | 8 | 87.50% | 40.82% | 0.2231 | 95 | 47.41% | -55.01% | 0.9354 |
| 7 | `pre_5m_return >= 1.0000 AND entry_range_position >= 0.7500` | 8 | 87.50% | 40.63% | 0.2221 | 89 | 50.10% | -54.56% | 0.9382 |
| 8 | `pre_5m_return >= 0.1000 AND pre_5m_volume_spike >= 2.0000 AND entry_range_position >= 0.7500` | 8 | 87.50% | 33.59% | 0.1761 | 89 | 47.34% | -50.26% | 0.9366 |
| 9 | `pre_5m_return >= 0.2500 AND pre_5m_volume_spike >= 2.0000 AND entry_range_position >= 0.7500` | 8 | 87.50% | 33.59% | 0.1761 | 89 | 47.34% | -50.26% | 0.9366 |
| 10 | `pre_5m_return >= 0.5000 AND entry_range_position >= 0.8500` | 8 | 87.50% | 22.97% | 0.1391 | 95 | 38.03% | -47.69% | 0.9409 |
| 11 | `pre_5m_volume_spike >= 1.0000 AND entry_range_position >= 0.7500` | 14 | 85.71% | 25.44% | 0.1387 | 76 | 31.94% | -41.16% | 0.9140 |
| 12 | `pre_5m_return >= 0.1000 AND entry_range_position >= 0.7500` | 19 | 84.21% | 32.45% | 0.1870 | 76 | 39.51% | -37.67% | 0.9214 |
| 13 | `pre_5m_return >= 0.5000 AND pre_5m_volume_spike >= 1.0000` | 12 | 83.33% | 28.92% | 0.1596 | 89 | 50.70% | -52.24% | 0.8850 |
| 14 | `pre_5m_volume_spike >= 1.5000 AND entry_range_position >= 0.7500` | 12 | 83.33% | 27.56% | 0.1512 | 76 | 34.20% | -44.53% | 0.9087 |
| 15 | `pre_5m_return >= 0.0000 AND pre_5m_volume_spike >= 1.5000 AND entry_range_position >= 0.7500` | 12 | 83.33% | 27.56% | 0.1512 | 76 | 34.20% | -44.53% | 0.9087 |
| 16 | `pre_5m_return >= 0.1000 AND entry_range_position >= 0.8500` | 17 | 82.35% | 25.62% | 0.1520 | 76 | 34.50% | -36.27% | 0.9244 |
| 17 | `pre_5m_return >= 0.5000 AND entry_range_position >= 0.6500` | 11 | 81.82% | 32.80% | 0.1873 | 89 | 46.26% | -48.32% | 0.9121 |
| 18 | `pre_5m_return >= 0.5000 AND pre_5m_volume_spike >= 1.5000` | 11 | 81.82% | 29.98% | 0.1663 | 89 | 52.25% | -54.30% | 0.8817 |
| 19 | `pre_5m_volume_spike >= 1.0000 AND entry_range_position >= 0.8500` | 11 | 81.82% | 15.83% | 0.0881 | 76 | 25.55% | -39.51% | 0.9482 |
| 20 | `pre_5m_return >= 0.2500 AND entry_range_position >= 0.6500` | 20 | 80.00% | 29.92% | 0.1651 | 81 | 42.91% | -40.28% | 0.9080 |
| 21 | `pre_5m_return >= 1.0000` | 10 | 80.00% | 32.58% | 0.1809 | 89 | 54.88% | -54.50% | 0.8863 |
| 22 | `pre_5m_return >= 1.0000 AND pre_5m_volume_spike >= 1.0000` | 10 | 80.00% | 32.58% | 0.1809 | 89 | 54.88% | -54.50% | 0.8863 |
| 23 | `pre_5m_return >= 0.5000 AND pre_5m_volume_spike >= 1.5000 AND entry_range_position >= 0.5000` | 10 | 80.00% | 32.50% | 0.1777 | 89 | 46.72% | -53.98% | 0.9054 |
| 24 | `pre_5m_return >= 0.2500 AND entry_range_position >= 0.4000` | 24 | 79.17% | 25.88% | 0.1491 | 76 | 41.50% | -39.35% | 0.9025 |
| 25 | `pre_5m_return >= 0.5000` | 14 | 78.57% | 26.03% | 0.1505 | 89 | 47.61% | -47.44% | 0.8929 |

## Entry Label Distribution

| entry label | profit | loss | win rate |
|---|---:|---:|---:|
| `breakout_volume_entry` | 13 | 9 | 59.09% |
| `dip_or_rebound_entry` | 2 | 4 | 33.33% |
| `high_range_entry` | 23 | 15 | 60.53% |
| `unclear_entry` | 12 | 22 | 35.29% |

## Important Notes

- These are candidate filters, not a finished trading system.
- The rules intentionally use entry-side fields only: `pre_5m_return`, `pre_20m_return`, `pre_5m_volume_spike`, and `entry_range_position`.
- Metrics such as max runup and exit efficiency are included only to understand what happened after entry; do not use them as live entry conditions.
- Because this is a balanced 50 profit / 50 loss sample, win rate here measures separation power, not the wallet's real-world win rate.
- Next step: test the best few rules on out-of-sample positions that were not used in this 100-sample dataset.
