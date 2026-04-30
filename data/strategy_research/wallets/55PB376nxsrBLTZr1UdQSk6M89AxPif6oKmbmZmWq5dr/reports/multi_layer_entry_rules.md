# Multi-layer Entry Rule Expansion

## Goal

Increase coverage if the goal is to simulate more of the wallet behavior. The important distinction: simulation can use broader layers, but live copy trading should size lower-confidence layers smaller.

## Baseline

- positions: 100
- baseline win rate: 50.0%

## Entry Label Distribution

| label | matches | wins | losses | win rate | avg ROI |
|---|---:|---:|---:|---:|---:|
| `breakout_volume_entry` | 22 | 13 | 9 | 59.1% | 8.9% |
| `dip_or_rebound_entry` | 6 | 2 | 4 | 33.3% | -8.3% |
| `high_range_entry` | 38 | 23 | 15 | 60.5% | 21.1% |
| `unclear_entry` | 34 | 12 | 22 | 35.3% | -1.3% |

## Proposed Layered Rules

Rules are applied in order. `New matches` means positions not already captured by earlier layers.

| layer | purpose | rule | new matches | wins | losses | new win rate | new avg ROI | standalone matches | standalone win rate |
|---:|---|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | Safest momentum/high-range follow mode | `pre_5m_return >= 0.25 AND entry_range_position >= 0.75` | 18 | 16 | 2 | 88.9% | 34.3% | 18 | 88.9% |
| 2 | Adds very-high-range near-high buys, including some missing pre-5m cases | `entry_range_position >= 0.85 AND distance_to_20m_high <= -0.05` | 4 | 4 | 0 | 100.0% | 96.1% | 7 | 100.0% |
| 3 | Adds high-range labeled fallback | `entry_label = high_range_entry AND entry_range_position >= 0.85` | 15 | 7 | 8 | 46.7% | 13.0% | 27 | 66.7% |
| 4 | Adds explicit breakout-volume wallet style | `entry_label = breakout_volume_entry` | 12 | 4 | 8 | 33.3% | -11.3% | 22 | 59.1% |
| 5 | Broad simulation layer: green continuation away from lows | `distance_to_20m_low >= 1.0 AND consecutive_red_candles <= 0` | 12 | 7 | 5 | 58.3% | 10.1% | 40 | 70.0% |
| 6 | Small rebound/scout behavior, not recommended for full size | `pre_1m_return >= 0.10 AND entry_range_position <= 0.50` | 5 | 4 | 1 | 80.0% | 7.0% | 8 | 87.5% |

## Combined Results

### Conservative Set: Layers A-D

- coverage: 49 / 100 (49.0%)
- wins/losses: 31 / 18
- win rate: 63.3%
- avg ROI: 21.6%

### Simulation Set: Layers A-F

- coverage: 66 / 100 (66.0%)
- wins/losses: 42 / 24
- win rate: 63.6%
- avg ROI: 18.4%

## Suggested Bot Interpretation

```yaml
entry_modes:
  A_momentum_high_range:
    confidence: high
    size_multiplier: 1.0
    pre_5m_return_gte: 0.25
    entry_range_position_gte: 0.75

  B_extreme_near_high:
    confidence: high_medium
    size_multiplier: 0.8
    entry_range_position_gte: 0.85
    distance_to_20m_high_lte: -0.05

  C_high_range_fallback:
    confidence: medium
    size_multiplier: 0.6
    entry_label: high_range_entry
    entry_range_position_gte: 0.85

  D_breakout_volume:
    confidence: medium
    size_multiplier: 0.6
    entry_label: breakout_volume_entry

  E_green_continuation:
    confidence: low_medium
    size_multiplier: 0.3
    distance_to_20m_low_gte: 1.0
    consecutive_red_candles_lte: 0

  F_short_rebound_scout:
    confidence: low
    size_multiplier: 0.2
    pre_1m_return_gte: 0.10
    entry_range_position_lte: 0.50
```

## Practical Recommendation

- If the goal is profit-focused follow trading, use A-B first, optionally C-D with reduced size.
- If the goal is behavioral simulation, include A-F and evaluate sell logic separately.
- Do not force 100% coverage. The remaining trades are likely trial/error, noisy entries, or require fields not present in this dataset.


## Refined Takeaway

The expansion shows an important split:

```text
A-B are tradable expansion layers.
C-F are behavioral simulation layers unless further validated.
```

Why:

- Layer A is the original high-confidence mode: 18 new matches, 88.9% win rate.
- Layer B adds 4 new matches, all winners, and captures very-high-range near-high entries that the first rule missed.
- Layer C and D increase behavioral coverage, but their incremental win rates are weak after A-B already captured the best overlap.
- Layer E is broad and helps simulate more wallet behavior, but should be small size if traded.
- Layer F is small-sample and should be treated as a scout mode only.

Recommended two-track usage:

```yaml
profit_focused_follow:
  enabled_layers: [A_momentum_high_range, B_extreme_near_high]
  note: "higher precision, lower coverage"

wallet_behavior_simulation:
  enabled_layers: [A_momentum_high_range, B_extreme_near_high, C_high_range_fallback, D_breakout_volume, E_green_continuation, F_short_rebound_scout]
  note: "higher coverage, noisier, requires tighter sell rules"
```

For live use, do not raise coverage by blindly copying every layer. Add one layer at a time and track its own realized ROI.
