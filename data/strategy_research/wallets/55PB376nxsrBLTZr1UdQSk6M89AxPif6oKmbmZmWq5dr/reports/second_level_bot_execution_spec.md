# 秒级交易机器人执行规格

## 目标

把 `55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr` 的买卖规则转成机器人可执行逻辑。

核心约束：

```text
执行周期：1 秒
目标持仓：约 60-90 秒
默认模式：先纸交易，再小仓实盘
交易类型：短线动量跟随，不是无脑复制钱包
```

一句话版本：

```text
只跟强势高位突破；买入后 15s、30s、60s 分层判断。
弱单快速撤，普通拉升快卖，强拉升移动止盈，爆拉先卖半仓再留尾仓。
```

## 输入数据

机器人每秒需要维护以下数据。

### 钱包事件

```yaml
wallet_event:
  wallet: string
  token: string
  side: buy | sell
  wallet_trade_time_ms: integer
  wallet_trade_price: float
  wallet_trade_size_sol: float
```

只在监听到目标钱包买入时触发入场评估。

### 市场行情

```yaml
market_state:
  now_ms: integer
  token: string
  last_price: float
  bid_price: float
  ask_price: float
  last_1s_volume: float
  last_5s_volume: float
  last_60s_volume: float
```

### 滚动特征

这些特征要用 1s 数据实时滚动计算。

```yaml
features:
  price_5m_ago: float
  high_20m: float
  low_20m: float
  volume_5m: float
  avg_volume_prior_5m: float
  pre_1m_return: float
  pre_5m_return: float
  entry_range_position: float
  distance_to_20m_high: float
  distance_to_20m_low: float
  pre_5m_volume_spike: float
  consecutive_red_1m_candles: integer
```

计算公式：

```text
pre_5m_return = last_price / price_5m_ago - 1

entry_range_position =
  (last_price - low_20m) / (high_20m - low_20m)

distance_to_20m_high =
  last_price / high_20m - 1

distance_to_20m_low =
  last_price / low_20m - 1

pre_5m_volume_spike =
  volume_5m / avg_volume_prior_5m
```

边界处理：

```text
如果 high_20m <= low_20m，不允许入场。
如果 price_5m_ago 缺失，不允许 A 层入场，但仍可评估 B 层。
如果 avg_volume_prior_5m <= 0，成交量确认视为缺失，不作为加分。
```

## 全局风控

```yaml
risk:
  mode: paper_first
  base_position_sol: 0.05
  max_open_positions: 2
  max_token_positions: 1
  cooldown_after_exit_seconds: 120
  max_total_exposure_sol: 0.10
  max_reentries_per_token: 1
  third_entry_enabled: false
```

交易前保护：

```yaml
execution_guard:
  max_buy_delay_ms: 1500
  skip_if_price_moved_after_wallet_buy_pct: 0.15
  max_slippage_bps: 800
```

执行解释：

```text
如果当前时间 - 钱包买入时间 > 1500ms，跳过。
如果当前价格比钱包买入价高出 15% 以上，跳过。
如果预估滑点超过 8%，跳过。
如果该 token 正在持仓或刚退出未过冷却期，跳过。
```

## 入场模式

机器人先按 A 到 F 顺序匹配，命中多个时只采用最高优先级模式。

实盘优先只开启 A、B；C-F 默认用于行为模拟或小仓观察。

```yaml
entry_modes:
  A_momentum_high_range:
    enabled_for_live: true
    priority: 1
    confidence: high
    size_multiplier: 1.0
    conditions:
      pre_5m_return_gte: 0.25
      entry_range_position_gte: 0.75

  B_extreme_near_high:
    enabled_for_live: true
    priority: 2
    confidence: high_medium
    size_multiplier: 0.8
    conditions:
      entry_range_position_gte: 0.85
      distance_to_20m_high_gte: -0.05

  C_high_range_fallback:
    enabled_for_live: false
    priority: 3
    confidence: medium
    size_multiplier: 0.6
    conditions:
      entry_label_eq: high_range_entry
      entry_range_position_gte: 0.85

  D_breakout_volume:
    enabled_for_live: false
    priority: 4
    confidence: medium
    size_multiplier: 0.6
    conditions:
      entry_label_eq: breakout_volume_entry
      require_overlap_with_A_or_B: true

  E_green_continuation:
    enabled_for_live: false
    priority: 5
    confidence: low_medium
    size_multiplier: 0.3
    conditions:
      distance_to_20m_low_gte: 1.0
      consecutive_red_1m_candles_lte: 0

  F_short_rebound_scout:
    enabled_for_live: false
    priority: 6
    confidence: low
    size_multiplier: 0.2
    conditions:
      pre_1m_return_gte: 0.10
      entry_range_position_lte: 0.50
```

推荐实盘配置：

```yaml
live_entry_layers:
  - A_momentum_high_range
  - B_extreme_near_high
```

成交量只作为加分，不作为 A/B 硬条件：

```yaml
volume_confirmation:
  preferred_pre_5m_volume_spike_gte: 1.5
  if_confirmed_size_multiplier_bonus: 0.15
  max_final_size_multiplier: 1.0
```

## 下单规则

```yaml
order:
  side: buy
  type: market_or_priority_swap
  size_sol: base_position_sol * layer.size_multiplier
  slippage_bps: 800
  timeout_ms: 1500
```

下单成功后创建持仓状态：

```yaml
position_state:
  token: string
  entry_mode: string
  entry_time_ms: integer
  entry_price: float
  size_sol: float
  remaining_pct: 100
  phase: OBSERVE
  max_price: entry_price
  max_roi: 0
  last_new_high_time_ms: entry_time_ms
  explosive_tp1_done: false
  reentry_count: 0
```

## 每秒持仓循环

每 1 秒对所有持仓执行一次。

```text
1. 更新 last_price。
2. 更新 ROI、max_price、max_roi。
3. 更新 drawdown_from_peak。
4. 更新 no_new_high_seconds。
5. 按退出优先级检查卖出条件。
6. 如果没有触发卖出，继续持仓。
```

实时指标：

```text
hold_seconds = now - entry_time
roi = last_price / entry_price - 1
max_roi = max(max_roi, roi)

如果 last_price > max_price:
  max_price = last_price
  last_new_high_time = now

drawdown_from_peak = last_price / max_price - 1
no_new_high_seconds = now - last_new_high_time
```

## 卖出优先级

卖出判断必须按这个顺序执行，先命中先处理。

```text
1. 硬止损
2. 15s 失败单
3. 30s 弱动能
4. 爆拉分批止盈
5. 强拉升移动止盈
6. 普通拉升移动止盈
7. 超时退出
```

### 1. 硬止损

任何时间触发。

```yaml
hard_stop:
  roi_lte: -0.10
  action: sell_all
```

### 2. 15s 失败单

用于快速砍掉买入后没有动能的单。

```yaml
failed_entry:
  check_after_seconds: 15
  stop_loss_roi_lte: -0.05
  no_push_max_roi_lt: 0.08
  action: sell_all
```

执行逻辑：

```text
如果 hold_seconds >= 15 且 roi <= -5%，全卖。
如果 hold_seconds >= 20 且 max_roi < +8%，全卖。
```

### 3. 30s 弱动能

用于清理没有进入有效拉升的持仓。

```yaml
weak_momentum:
  check_after_seconds: 30
  max_roi_lt: 0.15
  action: sell_all
```

执行逻辑：

```text
如果 hold_seconds >= 30 且 max_roi < +15%，全卖。
```

### 4. 爆拉分批止盈

爆拉优先级高于普通和强拉升。

```yaml
explosive_rally:
  detect_max_roi_gte: 1.00
  tp1_roi_gte: 1.50
  tp1_sell_pct: 50
  runner_trailing_drawdown_lte: -0.22
  runner_hard_take_profit_roi_gte: 4.00
  action:
    tp1: sell_50_pct_once
    runner_exit: sell_remaining
```

执行逻辑：

```text
如果 max_roi >= +100%，phase = EXPLOSIVE。
如果 phase = EXPLOSIVE 且 roi >= +150% 且未卖过 TP1，卖 50%。
如果 phase = EXPLOSIVE 且 drawdown_from_peak <= -22%，卖剩余仓位。
如果 phase = EXPLOSIVE 且 roi >= +400%，卖剩余仓位。
```

### 5. 强拉升移动止盈

```yaml
strong_rally:
  activate_max_roi_gte: 0.40
  preferred_take_profit_roi_gte: 0.50
  hard_take_profit_roi_gte: 0.60
  trailing_drawdown_lte: -0.15
  action: sell_all
```

执行逻辑：

```text
如果 max_roi >= +40%，phase = STRONG。
如果 phase = STRONG 且 roi >= +60%，全卖。
如果 phase = STRONG 且 drawdown_from_peak <= -15%，全卖。
```

说明：

```text
如果 30-60 秒内快速到 +40%，不要用普通拉升规则过早卖飞。
```

### 6. 普通拉升移动止盈

```yaml
normal_rally:
  activate_max_roi_gte: 0.15
  activate_max_roi_lt: 0.40
  sell_if_no_new_high_seconds_gte: 10
  trailing_drawdown_lte: -0.10
  action: sell_all
```

执行逻辑：

```text
如果 +15% <= max_roi < +40%，phase = NORMAL。
如果 phase = NORMAL 且 no_new_high_seconds >= 10，全卖。
如果 phase = NORMAL 且 drawdown_from_peak <= -10%，全卖。
```

### 7. 超时退出

这个策略平均持仓约 1 分钟，不应该长时间恋战。

```yaml
time_exit:
  normal_max_hold_seconds: 90
  strong_max_hold_seconds: 120
  explosive_runner_max_hold_seconds: 300
```

执行逻辑：

```text
如果 phase = OBSERVE 或 NORMAL，hold_seconds >= 90，全卖。
如果 phase = STRONG，hold_seconds >= 120，全卖。
如果 phase = EXPLOSIVE，hold_seconds >= 300，卖剩余仓位。
```

## 状态机

```text
WAIT_WALLET_BUY
  -> ENTRY_CHECK
  -> BUY_SUBMITTED
  -> POSITION_OPEN
  -> OBSERVE
  -> NORMAL | STRONG | EXPLOSIVE | EXITED
```

状态转移：

```yaml
states:
  WAIT_WALLET_BUY:
    on_wallet_buy: ENTRY_CHECK

  ENTRY_CHECK:
    if_no_layer_match: WAIT_WALLET_BUY
    if_execution_guard_failed: WAIT_WALLET_BUY
    if_layer_match: BUY_SUBMITTED

  BUY_SUBMITTED:
    if_order_filled: OBSERVE
    if_order_timeout_or_failed: WAIT_WALLET_BUY

  OBSERVE:
    if_exit_triggered: EXITED
    if_max_roi_gte_0_15: NORMAL
    if_max_roi_gte_0_40: STRONG
    if_max_roi_gte_1_00: EXPLOSIVE

  NORMAL:
    if_max_roi_gte_0_40: STRONG
    if_max_roi_gte_1_00: EXPLOSIVE
    if_exit_triggered: EXITED

  STRONG:
    if_max_roi_gte_1_00: EXPLOSIVE
    if_exit_triggered: EXITED

  EXPLOSIVE:
    if_tp1_triggered: EXPLOSIVE
    if_runner_exit_triggered: EXITED

  EXITED:
    start_cooldown: WAIT_WALLET_BUY
```

## 复买规则

复买默认只允许 1 次。

```yaml
reentry:
  enabled: true
  max_reentries: 1
  require_new_20m_high: true
  require_volume_continue: true
  size_multiplier: 0.6
  third_entry_enabled: false
```

复买触发条件：

```text
1. 同一 token 已经退出。
2. 冷却时间已过，或价格已经重新突破上次持仓期间高点。
3. 目标钱包再次买入。
4. 当前价格创 20m 新高或接近 20m 高点。
5. 最近 5m 成交量没有明显衰减。
```

复买卖出更严格：

```yaml
reentry_exit:
  fast_fail_check_after_seconds: 15
  stop_loss_roi_lte: -0.08
  weak_max_roi_lt: 0.10
  weak_check_after_seconds: 25
  max_hold_seconds: 60
```

第三次追高默认禁止。

## 伪代码

```python
def on_wallet_buy(event):
    if event.wallet != TARGET_WALLET:
        return

    market = get_market_state(event.token)
    features = compute_features(event.token, market.now_ms)

    if not pass_execution_guard(event, market):
        return

    layer = match_best_entry_layer(features)
    if layer is None:
        return

    if not layer.enabled_for_live and MODE == "live":
        return

    size_sol = BASE_POSITION_SOL * layer.size_multiplier
    if features.pre_5m_volume_spike >= 1.5:
        size_sol = min(BASE_POSITION_SOL, size_sol * 1.15)

    order = submit_buy(event.token, size_sol, MAX_SLIPPAGE_BPS)
    if order.filled:
        open_position(event.token, order.fill_price, size_sol, layer.name)


def on_second_tick(position):
    price = get_last_price(position.token)
    update_position_metrics(position, price)

    if position.roi <= -0.10:
        return sell_all(position, "hard_stop")

    if position.hold_seconds >= 15 and position.roi <= -0.05:
        return sell_all(position, "failed_15s_stop")

    if position.hold_seconds >= 20 and position.max_roi < 0.08:
        return sell_all(position, "failed_no_push")

    if position.hold_seconds >= 30 and position.max_roi < 0.15:
        return sell_all(position, "weak_30s")

    if position.max_roi >= 1.00:
        position.phase = "EXPLOSIVE"
        if position.roi >= 1.50 and not position.explosive_tp1_done:
            sell_pct(position, 50, "explosive_tp1")
            position.explosive_tp1_done = True
        if position.drawdown_from_peak <= -0.22 or position.roi >= 4.00:
            return sell_all(position, "explosive_runner_exit")
        if position.hold_seconds >= 300:
            return sell_all(position, "explosive_time_exit")
        return

    if position.max_roi >= 0.40:
        position.phase = "STRONG"
        if position.roi >= 0.60:
            return sell_all(position, "strong_hard_tp")
        if position.drawdown_from_peak <= -0.15:
            return sell_all(position, "strong_trailing_stop")
        if position.hold_seconds >= 120:
            return sell_all(position, "strong_time_exit")
        return

    if position.max_roi >= 0.15:
        position.phase = "NORMAL"
        if position.no_new_high_seconds >= 10:
            return sell_all(position, "normal_no_new_high")
        if position.drawdown_from_peak <= -0.10:
            return sell_all(position, "normal_trailing_stop")
        if position.hold_seconds >= 90:
            return sell_all(position, "normal_time_exit")
        return

    if position.hold_seconds >= 90:
        return sell_all(position, "observe_time_exit")
```

## 机器人配置模板

```yaml
strategy:
  name: wallet_55PB_second_level_momentum
  target_wallet: 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr
  tick_interval_ms: 1000
  expected_hold_seconds: 60
  mode: paper

live_layers:
  - A_momentum_high_range
  - B_extreme_near_high

risk:
  base_position_sol: 0.05
  max_open_positions: 2
  max_token_positions: 1
  cooldown_after_exit_seconds: 120
  max_total_exposure_sol: 0.10

execution_guard:
  max_buy_delay_ms: 1500
  skip_if_price_moved_after_wallet_buy_pct: 0.15
  max_slippage_bps: 800

entry:
  A_momentum_high_range:
    size_multiplier: 1.0
    pre_5m_return_gte: 0.25
    entry_range_position_gte: 0.75

  B_extreme_near_high:
    size_multiplier: 0.8
    entry_range_position_gte: 0.85
    distance_to_20m_high_gte: -0.05

exit:
  hard_stop_roi_lte: -0.10

  failed:
    check_after_seconds: 15
    stop_loss_roi_lte: -0.05
    no_push_check_after_seconds: 20
    no_push_max_roi_lt: 0.08

  weak:
    check_after_seconds: 30
    max_roi_lt: 0.15

  normal:
    activate_max_roi_gte: 0.15
    activate_max_roi_lt: 0.40
    no_new_high_seconds_gte: 10
    trailing_drawdown_lte: -0.10
    max_hold_seconds: 90

  strong:
    activate_max_roi_gte: 0.40
    hard_take_profit_roi_gte: 0.60
    trailing_drawdown_lte: -0.15
    max_hold_seconds: 120

  explosive:
    activate_max_roi_gte: 1.00
    tp1_roi_gte: 1.50
    tp1_sell_pct: 50
    runner_trailing_drawdown_lte: -0.22
    runner_hard_take_profit_roi_gte: 4.00
    runner_max_hold_seconds: 300

reentry:
  enabled: true
  max_reentries: 1
  require_new_20m_high: true
  require_volume_continue: true
  size_multiplier: 0.6
  third_entry_enabled: false
```

## 日志字段

每笔交易必须记录这些字段，方便回测和实盘复盘。

```yaml
trade_log:
  token: string
  wallet_buy_time: timestamp
  bot_buy_time: timestamp
  bot_sell_time: timestamp
  entry_mode: string
  entry_price: float
  exit_price: float
  size_sol: float
  realized_roi: float
  max_roi: float
  hold_seconds: float
  exit_reason: string
  pre_5m_return: float
  entry_range_position: float
  distance_to_20m_high: float
  pre_5m_volume_spike: float
  max_drawdown_from_peak: float
  no_new_high_seconds_at_exit: float
  slippage_bps: float
  buy_delay_ms: integer
```

## 验收标准

纸交易阶段至少按以下标准验收。

```text
1. A/B 层单独统计，不和 C-F 混在一起。
2. 每笔平均持仓应接近 60-90 秒。
3. 失败单大部分应在 15-30 秒内退出。
4. 普通盈利单主要由 no_new_high 或 10% 回撤退出。
5. 强拉升单不能在 +15% 到 +25% 区间过早卖飞。
6. 爆拉单必须出现分批卖出记录。
7. 每笔必须有 exit_reason，不能只有 manual 或 unknown。
```

建议先跑：

```text
paper 100 笔 A/B 层信号
再单独评估：
- 胜率
- 平均 ROI
- 中位持仓秒数
- 15s/30s 快速退出占比
- 卖飞比例
- 超时退出占比
```
