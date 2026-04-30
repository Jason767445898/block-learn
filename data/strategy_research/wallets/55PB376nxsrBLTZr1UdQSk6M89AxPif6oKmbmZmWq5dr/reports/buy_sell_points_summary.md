# 买卖点总结

## 钱包

```text
55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr
```

## 核心结论

这个钱包不是简单的“买了就跟”，更像是做短线动量交易。

已总结出的有效模式是：

```text
买强势突破，不买弱反弹。
买入后快速观察动能。
有利润就根据强度分层卖。
复买失败要快速撤。
```

## 买点规则

### 基础买点

```yaml
entry_filter:
  pre_5m_return_gte: 0.25
  entry_range_position_gte: 0.75
```

含义：

```text
1. 钱包买入前，代币最近 5 分钟涨幅 >= 25%
2. 钱包买入价位于最近 20 分钟价格区间的上方 25%
```

这代表它是在追强势币，而不是抄底。

### 成交量确认

```yaml
optional_filter:
  pre_5m_volume_spike_gte: 1.5
```

含义：

```text
买入前 5 分钟成交量至少是近期平均成交量的 1.5 倍。
```

这个条件可以作为加分项。加上它后，交易次数更少，但信号更干净。

## GMGN 手动判断方式

### 判断 5 分钟涨幅

```text
1. 打开 GMGN 的 1m K 线
2. 找到钱包买入那根 K 线
3. 往前数 5 根 1m K
4. 当前价格相对 5 分钟前价格涨幅 >= 25%
```

例子：

```text
5 分钟前价格：0.00000100
钱包买入价格：0.00000125

涨幅 = 25%
符合条件
```

### 判断区间高位

公式：

```text
entry_range_position =
(买入价 - 过去20分钟最低价) / (过去20分钟最高价 - 过去20分钟最低价)
```

要求：

```text
entry_range_position >= 0.75
```

例子：

```text
20分钟最低价：0.00000100
20分钟最高价：0.00000200
钱包买入价：0.00000180

(0.00000180 - 0.00000100) / (0.00000200 - 0.00000100) = 0.80

符合条件
```

### 判断成交量

```text
1. 看钱包买入前 5 根 1m K
2. 最近成交量柱明显放大
3. 最好达到前面平均量的 1.5 倍以上
```

## 买点复盘结果

在 100 笔样本中：

```text
基础买点：
命中：18 笔
胜利：16 笔
失败：2 笔
胜率：88.9%
平均 ROI：34.33%
中位持仓：约 78 秒
```

加成交量过滤：

```text
命中：10 笔
胜利：9 笔
失败：1 笔
胜率：90.0%
平均 ROI：33.11%
中位持仓：约 82 秒
```

结论：

```text
买点规则基本成功逆向出了该钱包的高质量买入子集。
但它不是该钱包的全部买入行为。
```

## 多层买点规则

如果目标是“模拟这个钱包”，不能只用一条买点规则。这个钱包至少包含多个入场模式。

目前可以拆成两类：

```text
实盘优先层：A-B
行为模拟层：A-F
```

### A 层：动量高位买入

这是最稳的主规则。

```yaml
A_momentum_high_range:
  confidence: high
  size_multiplier: 1.0
  pre_5m_return_gte: 0.25
  entry_range_position_gte: 0.75
```

复盘结果：

```text
新增命中：18 笔
胜利：16 笔
失败：2 笔
胜率：88.9%
平均 ROI：34.3%
```

解释：

```text
最近 5 分钟已经明显上涨，并且钱包买在 20 分钟区间高位。
这是典型追强/突破买法。
```

### B 层：极高位接近新高买入

这是最值得加入的第二层规则。

```yaml
B_extreme_near_high:
  confidence: high_medium
  size_multiplier: 0.8
  entry_range_position_gte: 0.85
  distance_to_20m_high_lte: -0.05
```

复盘结果：

```text
新增命中：4 笔
胜利：4 笔
失败：0 笔
新增胜率：100.0%
新增平均 ROI：96.1%
```

解释：

```text
即使 pre_5m_return 缺失或不够明显，
只要买入位置极高，并且接近/突破 20 分钟高点，
也可能是该钱包的重要买点。
```

这层可以补上部分原始 A 层漏掉的大盈利交易。

### C 层：高位标签兜底

```yaml
C_high_range_fallback:
  confidence: medium
  size_multiplier: 0.6
  entry_label: high_range_entry
  entry_range_position_gte: 0.85
```

复盘结果：

```text
新增命中：15 笔
胜利：7 笔
失败：8 笔
新增胜率：46.7%
新增平均 ROI：13.0%
```

解释：

```text
这层能提高钱包行为覆盖率，
但新增部分胜率不高，不适合直接满仓实盘。
```

### D 层：放量突破标签

```yaml
D_breakout_volume:
  confidence: medium
  size_multiplier: 0.6
  entry_label: breakout_volume_entry
```

复盘结果：

```text
新增命中：12 笔
胜利：4 笔
失败：8 笔
新增胜率：33.3%
新增平均 ROI：-11.3%
```

解释：

```text
单看 breakout_volume_entry 标签不够好。
它更适合和 A/B 条件重合时使用，不能单独当买入信号。
```

### E 层：无红 K 的延续买入

```yaml
E_green_continuation:
  confidence: low_medium
  size_multiplier: 0.3
  distance_to_20m_low_gte: 1.0
  consecutive_red_candles_lte: 0
```

复盘结果：

```text
新增命中：12 笔
胜利：7 笔
失败：5 笔
新增胜率：58.3%
新增平均 ROI：10.1%
```

解释：

```text
这层更像钱包的广义趋势延续行为。
可以用于模拟钱包，但实盘应该小仓。
```

### F 层：短线反弹试探

```yaml
F_short_rebound_scout:
  confidence: low
  size_multiplier: 0.2
  pre_1m_return_gte: 0.10
  entry_range_position_lte: 0.50
```

复盘结果：

```text
新增命中：5 笔
胜利：4 笔
失败：1 笔
新增胜率：80.0%
新增平均 ROI：7.0%
```

解释：

```text
样本太小，收益不高。
只能当小仓 scout 模式，不能和 A/B 同等权重。
```

## 多层规则组合结果

### 实盘优先组合：A-B

```yaml
profit_focused_follow:
  enabled_layers:
    - A_momentum_high_range
    - B_extreme_near_high
```

结论：

```text
覆盖率比原始 A 层略高。
质量仍然很强。
适合作为下一阶段优先纸交易/小仓实盘规则。
```

### 行为模拟组合：A-F

```yaml
wallet_behavior_simulation:
  enabled_layers:
    - A_momentum_high_range
    - B_extreme_near_high
    - C_high_range_fallback
    - D_breakout_volume
    - E_green_continuation
    - F_short_rebound_scout
```

复盘结果：

```text
覆盖：66 / 100 笔
覆盖率：66.0%
胜利：42 笔
失败：24 笔
胜率：63.6%
平均 ROI：18.4%
```

结论：

```text
A-F 更像是在模拟钱包行为，
但它会复制进更多试错单和低质量单。
如果实盘使用，C-F 必须降低仓位，并使用更严格卖出规则。
```

## 不建议跟的买点

避免这些情况：

```text
1. 低位磨盘后的小反弹
2. 下跌中的反抽
3. 买入时位置不在 20 分钟高位区域
4. 买入后价格已经比钱包成交价又冲高 15% 以上
5. 第三次、第四次追高复买
```

机器人保护：

```yaml
execution:
  max_buy_delay_ms: 1500
  skip_if_price_moved_after_wallet_buy_pct: 0.15
  max_slippage_bps: 800
```

## 卖点核心逻辑

卖点不能只用一个固定止盈。

更合理的是买入后动态分类：

```text
失败单：快速撤
普通拉升：小利润快卖
强拉升：拿到 50%-60% 或移动止盈
爆拉：先卖一半，剩余仓位吃主升浪
```

## 动态分类标准

### 失败单

判断：

```text
买入后 10-20 秒内：
ROI <= -5%
或者
没有出现 +8% 以上浮盈
或者
价格跌破买入价且成交量变弱
```

处理：

```yaml
failed_entry:
  check_after_seconds: 15
  stop_loss_roi: -0.05
  hard_stop_loss_roi: -0.10
  action: sell_all
```

口径：

```text
-5% 到 -10% 直接撤。
不要等它亏到 -30% 甚至 -50%。
```

### 普通拉升

判断：

```text
买入后 20-30 秒内：
最高浮盈达到 +15% 到 +35%
但不再连续创新高
或者从高点回撤 8%-12%
```

处理：

```yaml
normal_rally:
  roi_gte: 0.15
  roi_lt: 0.40
  sell_if_no_new_high_seconds: 10
  trailing_drawdown: 0.10
  action: sell_all
```

口径：

```text
+15% 到 +25% 快速卖。
这是小肉单，不要贪。
```

### 强拉升

判断：

```text
买入后 30-60 秒内：
ROI >= +40%
并且价格继续创新高
并且从高点回撤小于 12%-15%
```

处理：

```yaml
strong_rally:
  activate_roi_gte: 0.40
  target_roi: 0.50
  hard_take_profit_roi: 0.60
  trailing_drawdown: 0.15
```

口径：

```text
如果快速到 +40%，不要 +20% 就卖飞。
目标看 +50% 到 +60%，或者用 15% 回撤移动止盈。
```

### 爆拉

判断：

```text
买入后 20-40 秒内：
ROI >= +100%
并且价格连续创新高
并且成交量继续放大
并且回撤小于 15%-20%
```

处理：

```yaml
explosive_rally:
  detect_roi_gte: 1.00
  tp1_roi: 1.50
  tp1_sell_pct: 50
  runner_trailing_drawdown: 0.22
  runner_target_roi: 3.00
  runner_hard_take_profit_roi: 4.00
```

口径：

```text
+150% 左右卖一半。
剩余仓位用移动止盈，尝试吃 +300% 到 +400% 以上。
```

## 复买规则

这个钱包会复买，但复买风险明显更高。

建议：

```yaml
reentry:
  enabled: true
  max_reentries: 1
  require_new_high: true
  require_volume_continue: true
  size_multiplier: 0.6
```

第三次追高默认关闭：

```yaml
third_entry:
  enabled: false
```

如果一定允许第三次追：

```yaml
third_entry:
  size_multiplier: 0.3
  fast_fail_check_after_seconds: 15
  stop_loss_roi: -0.08
  max_hold_seconds: 30
```

## 完整机器人模板

```yaml
strategy:
  name: wallet_55PB_momentum_reverse_engineering
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

exit_classification:
  failed:
    check_after_seconds: 15
    roi_lte: -0.05
    hard_stop_loss_roi: -0.10
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

## 1s 数据验证口径

如果获取 1s K 线，可以更好地检验卖点。

验证方式：

```text
以钱包 first_buy_time 为 t0。

t0 + 15s:
  检查 ROI 是否 <= -5%
  检查是否出现过 +8% 以上浮盈

t0 + 30s:
  检查最高浮盈是否 >= +15%
  检查是否继续创新高

t0 + 60s:
  检查是否进入强拉升或爆拉

持仓期间：
  记录 max_runup
  记录 drawdown_from_peak
  记录 no_new_high_seconds
  判断触发哪类卖点
```

匹配标准：

```text
卖出时间误差 <= 10 秒：高度匹配
卖出时间误差 <= 30 秒：可接受
ROI 差距 <= 10%-20%：可接受
盈亏方向一致：基础正确
```

注意：

```text
1s K 线可以明显提升卖点验证质量。
但如果要完全逆向该钱包的分批卖出、复买、止损细节，
最好仍然获取逐笔 swap 数据。
```

## 最终一句话

```text
买点：跟它买强势高位突破。
卖点：买入后按 15s、30s、60s 动态分类。
弱就快跑，强就移动止盈，爆拉就先卖半仓再留尾仓。
```
