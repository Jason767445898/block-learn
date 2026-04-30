# GMGN Trade Playbook

## Purpose

This note records the practical GMGN buy and sell rules derived from the wallet analysis.

The strategy is not "copy every buy". The useful pattern is:

- Follow only momentum/high-range buys.
- Avoid unclear entries, weak rebounds, and late failed re-entries.
- Classify the trade after entry by price speed, new highs, volume continuation, and drawdown.

## Buy Setup

Use these as the main buy filters when the tracked wallet buys.

```yaml
entry_filter:
  pre_5m_return_gte: 0.25
  entry_range_position_gte: 0.75
  optional_pre_5m_volume_spike_gte: 1.5
```

### `pre_5m_return_gte: 0.25`

The token should already be up at least 25% in the 5 minutes before the wallet buy.

GMGN manual check:

- Use the 1m chart.
- Find the wallet buy candle.
- Compare the buy price with the price 5 candles earlier.
- Only buy if the token is up at least 25%.

Example:

```text
Price 5 minutes ago: 0.00000100
Wallet buy price:    0.00000125 or higher
Result: qualified
```

### `entry_range_position_gte: 0.75`

The wallet buy price should be near the high of the recent 20-minute range.

Formula:

```text
entry_range_position =
(buy_price - 20m_low) / (20m_high - 20m_low)
```

Qualified if the result is `>= 0.75`.

Example:

```text
20m low:          0.00000100
20m high:         0.00000200
Wallet buy price: 0.00000180

(0.00000180 - 0.00000100) / (0.00000200 - 0.00000100) = 0.80
Result: qualified
```

### `optional_pre_5m_volume_spike_gte: 1.5`

The recent volume should be at least 1.5x the prior average volume.

GMGN manual check:

- Use the 1m chart.
- Look at the 5 candles before the wallet buy.
- Volume bars should be clearly larger than the previous baseline.
- This is a confirmation filter, not always a hard requirement.

## Buy Interpretation

The desired buy is:

```text
The wallet buys while the token is already strong, near the local high,
and preferably with volume expansion.
```

Avoid:

- Low-range entries.
- Unclear entries.
- Weak dip/rebound entries.
- Late chase after price has already moved far beyond the wallet buy.

Suggested execution protection:

```yaml
execution:
  max_buy_delay_ms: 1500
  skip_if_price_moved_after_wallet_buy_pct: 0.15
  max_slippage_bps: 800
```

## Sell Logic Overview

The sell point should not be a single fixed percentage. Classify the trade after entry.

Observed categories:

```text
Normal rally:     +15% to +25%, sell quickly.
Strong rally:     +50% to +60%, sell or trail.
Explosive rally:  +150% sell partial, keep runner for +300% to +400%+.
Failed re-entry:  -5% to -10%, exit quickly.
```

The key is not knowing the category before entry. The category is determined dynamically after buying.

Use:

- Speed of profit after entry.
- Whether price keeps making new highs.
- Whether volume continues expanding.
- Whether drawdown stays shallow.

## Dynamic Classification

### First Check: 15 Seconds After Entry

```text
If ROI <= -5%:
  classify as failed entry or failed re-entry
  sell all

If ROI < +8%:
  classify as weak momentum
  continue observing, but do not hold long

If ROI >= +15%:
  trade is alive
  switch to normal/strong rally tracking
```

### Second Check: 30 Seconds After Entry

```text
If max ROI < +15%:
  momentum is too weak
  sell all

If ROI is +15% to +35%:
  normal rally
  sell if price stops making new highs or pulls back 8% to 12% from the high

If ROI >= +40%:
  strong rally
  enable trailing stop
```

### Third Check: 60 Seconds After Entry

```text
If ROI >= +50% and price still makes new highs:
  strong rally
  keep trailing

If ROI >= +100% and volume is expanding:
  explosive rally
  do not sell all too early

If no new high for 15 to 20 seconds:
  sell or tighten trailing stop
```

## Sell Rules

### Failed Entry Or Failed Re-entry

Use this when price fails shortly after buying.

```yaml
failed_entry:
  check_after_seconds: 15
  roi_lte: -0.05
  sell_pct: 100
```

More conservative:

```yaml
failed_entry:
  check_after_seconds: 15
  no_profit_roi_gte: 0.08
  stop_loss_roi: -0.08
  sell_pct: 100
```

Practical rule:

```text
If it does not push within 15 to 30 seconds, do not wait for a deep loss.
```

### Normal Rally

Use this when the trade gives a quick but limited move.

```yaml
normal_rally:
  roi_gte: 0.15
  roi_lt: 0.40
  sell_if_no_new_high_seconds: 10
  trailing_drawdown: 0.10
  sell_pct: 100
```

Practical rule:

```text
Take +15% to +25% if the price stops making new highs.
```

### Strong Rally

Use this when price keeps pushing after entry.

```yaml
strong_rally:
  activate_roi_gte: 0.40
  target_roi: 0.50
  hard_take_profit_roi: 0.60
  trailing_drawdown: 0.15
```

Practical rule:

```text
If it reaches +40% quickly and still makes new highs, do not sell at +20%.
Let it reach the +50% to +60% zone or use trailing stop.
```

### Explosive Rally

Use this when the token goes vertical shortly after entry.

```yaml
explosive_rally:
  detect_roi_gte: 1.00
  tp1_roi: 1.50
  tp1_sell_pct: 50
  runner_trailing_drawdown: 0.22
  runner_hard_take_profit_roi: 4.00
  max_hold_seconds: 120
```

Practical rule:

```text
At around +150%, sell half to lock profit.
Keep the rest with trailing stop for a possible +300% to +400%+ move.
```

## Re-entry Rules

The wallet often re-enters after selling, but late re-entries are risky.

Observed behavior:

- First entry can produce fast profit.
- Second entry may still work if the token makes a new high.
- Third entry often has worse risk/reward.

Suggested robot behavior:

```yaml
reentry:
  enabled: true
  max_reentries: 1
  require_new_high: true
  require_volume_continue: true
  size_multiplier: 0.6
```

Avoid third chase by default:

```yaml
third_entry:
  enabled: false
```

If third entry is allowed:

```yaml
third_entry:
  size_multiplier: 0.3
  fast_fail_check_after_seconds: 15
  stop_loss_roi: -0.08
  max_hold_seconds: 30
```

## Practical Bot Template

```yaml
strategy:
  name: gmgn_wallet_momentum_playbook
  mode: paper_first

entry_filter:
  pre_5m_return_gte: 0.25
  entry_range_position_gte: 0.75
  optional_pre_5m_volume_spike_gte: 1.5

execution:
  max_buy_delay_ms: 1500
  skip_if_price_moved_after_wallet_buy_pct: 0.15
  max_slippage_bps: 800

position:
  size_sol: 0.05
  max_open_positions: 2
  cooldown_seconds: 120

classification:
  failed:
    check_after_seconds: 15
    roi_lte: -0.05
    action: sell_all

  weak:
    check_after_seconds: 30
    max_roi_lt: 0.15
    action: sell_all

  normal:
    roi_gte: 0.15
    roi_lt: 0.40
    sell_if_no_new_high_seconds: 10
    trailing_drawdown: 0.10
    action: sell_all

  strong:
    roi_gte: 0.40
    hard_take_profit_roi: 0.60
    trailing_drawdown: 0.15

  explosive:
    roi_gte: 1.00
    tp1_roi: 1.50
    tp1_sell_pct: 50
    runner_trailing_drawdown: 0.22
    runner_hard_take_profit_roi: 4.00

reentry:
  enabled: true
  max_reentries: 1
  require_new_high: true
  require_volume_continue: true
  size_multiplier: 0.6
```

## Final Rule Of Thumb

```text
Buy only when the wallet buys strength.
After buying, classify the trade at 15s, 30s, and 60s.
Fast profit + new highs + shallow pullback = hold or trail.
No push + no new high + quick drawdown = exit immediately.
```

