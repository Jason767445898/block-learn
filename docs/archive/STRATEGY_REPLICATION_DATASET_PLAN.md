# 钱包交易策略复刻数据集规划

## 1. 目标

本文件专门规划“如何通过 K 线数据研究并复刻某个盈利钱包的交易策略”，不和现有钱包分析代码说明混在一起。

核心目标：

- 从目标钱包的历史交易中，挑选 50 个盈利 position 和 50 个亏损 position。
- 下载这些 position 对应 token 的完整 K 线窗口。
- 分析它在买入前、持仓中、平仓前的市场特征。
- 找出盈利交易和亏损交易之间的差异。
- 最终把这些差异转成可回测、可执行的交易规则。

这里的研究对象不是单笔交易，而是一个完整 position：

```text
wallet + mint + 从首次买入到最后一次卖出
```

如果同一个 mint 有多次分离的交易周期，后续可以再拆成多个 position。MVP 阶段可以先按 `wallet + mint` 聚合。

## 2. 为什么要分盈利样本和亏损样本

只看盈利交易会产生严重幸存者偏差。我们真正要找的是：

- 盈利交易在买入前有什么共同特征。
- 亏损交易在买入前是否也有相同特征。
- 盈利交易在持仓期间是否出现了更强的趋势延续。
- 亏损交易是否更早出现了失败信号。
- 钱包为什么有些亏损会快速平仓，有些亏损会继续拿。
- 钱包的盈利是否来自可重复规则，还是来自少数极端大盈利。

因此，50 个盈利 position 和 50 个亏损 position 是一个合理的第一版研究样本。

但要注意：50/50 的平衡样本适合做“特征对比”，不适合直接代表真实胜率。真实胜率仍然要用完整历史交易计算。

## 3. 样本定义

### 盈利 position

满足：

```text
realized_pnl_sol > 0
```

优先选择：

- 已完全卖出的 position。
- 有明确首次买入和最后卖出时间。
- K 线数据完整。
- PnL 排名前 50，或者按时间顺序取最近 50 个盈利样本。

建议 MVP 使用：

```text
最近 50 个已平仓盈利 position
```

这样更接近当前策略，而不是混入太久以前的市场环境。

### 亏损 position

满足：

```text
realized_pnl_sol < 0
```

优先选择：

- 已完全卖出的 position。
- 有明确首次买入和最后卖出时间。
- K 线数据完整。
- 最近 50 个亏损样本。

### 暂不纳入 MVP 的样本

MVP 先排除以下情况：

- 没有卖出的 position。
- 只买入但还在持仓的 position。
- K 线缺失严重的 token。
- 交易记录明显不完整的 position。
- 疑似 LP、发币、迁移、项目方操作导致的非普通交易 position。

这些样本后续可以单独建一个 `open_positions` 或 `special_cases` 数据集。

## 4. K 线下载范围

每个 position 需要下载三段窗口，但实际下载时建议一次性下载完整区间。

### 下载区间

```text
from = first_buy_time - pre_entry_window
to   = last_sell_time + post_exit_window
```

MVP 推荐：

```text
pre_entry_window = 20 minutes
post_exit_window = 20 minutes
resolution = 1m
```

也就是每个 position 至少覆盖：

- 买入前 20 根 1m K 线。
- 从首次买入到最后卖出期间的全部 1m K 线。
- 卖出后 20 根 1m K 线。

如果某个 position 持仓时间超过 60 分钟，可以额外下载 5m K 线，用于更高周期趋势判断。

### 为什么不能只下载当前 K 线和前一根 K 线

当前 K 线和前一根 K 线只能解释很局部的入场环境，无法回答：

- 买入前是否已经连续上涨。
- 买入前是否已经放量。
- 买入是否发生在突破之后。
- 持仓期间是否曾经大幅浮盈。
- 加仓是浮盈加仓还是亏损摊平。
- 平仓前是否已经出现趋势失败。
- 卖出后是否继续下跌，说明止损有效。
- 卖出后是否继续上涨，说明卖飞。

所以最小分析单位必须是“围绕 position 的完整 K 线窗口”。

## 5. 数据目录规划

建议单独建立数据目录，不和代码逻辑混在一起：

```text
data/
  strategy_research/
    wallets/
      <wallet>/
        positions.csv
        selected_positions.csv
        klines/
          <mint>_1m_<from>_<to>.json
        features/
          entry_features.csv
          holding_features.csv
          exit_features.csv
          position_features.csv
        reports/
          dataset_summary.md
          feature_comparison.md
          rule_candidates.md
```

### `positions.csv`

保存目标钱包完整 position 列表。

字段建议：

```text
wallet
mint
first_buy_time
last_sell_time
buy_count
sell_count
total_buy_sol
total_sell_sol
realized_pnl_sol
realized_roi
holding_seconds
is_closed
```

### `selected_positions.csv`

保存最终选入研究数据集的 100 个样本。

字段建议：

```text
wallet
mint
sample_group
first_buy_time
last_sell_time
realized_pnl_sol
realized_roi
holding_seconds
kline_file
exclude_reason
```

其中：

```text
sample_group = profit / loss
```

### K 线文件

每个 token position 一个 K 线文件：

```text
<mint>_1m_<from>_<to>.json
```

K 线字段：

```text
mint
timestamp
open
high
low
close
volume
resolution
source
```

## 6. 特征分析设计

每个 position 分为三类特征：

- 买入前特征：解释为什么买。
- 持仓中特征：解释为什么拿、为什么加仓、是否经历浮盈或回撤。
- 平仓前特征：解释为什么卖、为什么亏损卖、是否止盈或止损。

## 7. 买入前特征

分析窗口：

```text
first_buy_time 前 20 根 1m K 线
```

建议计算：

```text
pre_1m_return
pre_3m_return
pre_5m_return
pre_10m_return
pre_20m_return
pre_5m_volume_spike
pre_20m_volume_spike
entry_range_position
break_previous_high
distance_to_20m_high
distance_to_20m_low
consecutive_green_candles
consecutive_red_candles
volatility_20m
```

要回答的问题：

- 它是不是追涨买入？
- 它是不是放量买入？
- 它是不是突破前高买入？
- 它是不是下跌后抄底？
- 它是不是在高波动阶段入场？
- 盈利样本和亏损样本的入场前走势是否不同？

可能的标签：

```text
放量追涨
突破买入
回踩买入
低位反弹
连续下跌接入
无明显信号
```

## 8. 持仓中特征

分析窗口：

```text
first_buy_time 到 last_sell_time
```

建议计算：

```text
max_runup_during_holding
max_drawdown_during_holding
time_to_max_runup
time_to_max_drawdown
highest_price_before_exit
lowest_price_before_exit
holding_return_path
add_count
add_after_profit_count
add_after_loss_count
avg_add_interval_seconds
largest_add_size_sol
```

要回答的问题：

- 盈利 position 是否很快出现浮盈？
- 亏损 position 是否买入后立刻走弱？
- 该钱包是否会在浮盈时加仓？
- 该钱包是否会在亏损时摊平？
- 它能忍受多大的回撤？
- 它通常持仓多久没有上涨就会卖？

加仓分类：

```text
浮盈加仓
突破加仓
回踩加仓
亏损摊平
拆单建仓
无加仓
```

## 9. 平仓前特征

分析窗口：

```text
last_sell_time 前 20 根 1m K 线
```

同时保留：

```text
last_sell_time 后 20 根 1m K 线
```

卖出后窗口不用来解释当时决策，但可用于评估卖出效果。

建议计算：

```text
pre_exit_1m_return
pre_exit_3m_return
pre_exit_5m_return
exit_range_position
exit_efficiency
drawdown_from_peak_before_exit
sell_after_breakdown
post_exit_5m_return
post_exit_20m_return
missed_profit_after_exit
loss_avoided_after_exit
```

要回答的问题：

- 它是在冲高卖出，还是回撤后卖出？
- 它是固定盈利就卖，还是等趋势走坏再卖？
- 亏损卖出是及时止损，还是恐慌割肉？
- 卖出后继续跌，说明止损有效。
- 卖出后继续涨，说明卖飞。

平仓分类：

```text
高效止盈
回撤止盈
快速止损
跌破结构止损
时间止损
卖飞
有效避险
迟到止损
```

## 10. 盈利样本 vs 亏损样本对比

最终要做的不是单个样本解释，而是对比两个组的统计差异。

重点对比：

```text
盈利组平均 pre_5m_return vs 亏损组平均 pre_5m_return
盈利组平均 volume_spike vs 亏损组平均 volume_spike
盈利组 entry_range_position 分布 vs 亏损组 entry_range_position 分布
盈利组 max_runup 出现速度 vs 亏损组 max_runup 出现速度
盈利组 max_drawdown vs 亏损组 max_drawdown
盈利组 holding_seconds vs 亏损组 holding_seconds
盈利组 exit_efficiency vs 亏损组 exit_efficiency
亏损卖出后继续下跌比例
盈利卖出后继续上涨比例
```

如果发现：

```text
盈利样本买入前 volume_spike 显著更高
亏损样本买入后 3 分钟内没有新高
盈利样本通常在 2 分钟内出现 >30% 浮盈
亏损样本通常在 -15% 后快速卖出
```

就可以形成候选策略规则：

```text
只买入放量突破样本
买入后 3 分钟不创新高则退出
盈利超过 30% 后回撤 15% 止盈
亏损超过 15% 立即止损
```

## 11. MVP 实施步骤

### 第一步：构建 position 列表

从 Bitquery 下载目标钱包历史 pump.fun 交易，聚合成 position。

输出：

```text
positions.csv
```

### 第二步：选择样本

从 `positions.csv` 中筛选：

```text
50 个最近盈利已平仓 position
50 个最近亏损已平仓 position
```

输出：

```text
selected_positions.csv
```

### 第三步：下载 K 线

对 `selected_positions.csv` 中每个 mint 下载 GMGN K 线。

下载范围：

```text
first_buy_time - 20m
last_sell_time + 20m
```

输出：

```text
klines/*.json
```

### 第四步：提取特征

对每个 position 生成：

```text
entry_features.csv
holding_features.csv
exit_features.csv
position_features.csv
```

### 第五步：生成对比报告

对盈利组和亏损组做统计对比。

输出：

```text
reports/feature_comparison.md
```

### 第六步：提炼候选规则

根据差异最大的特征生成规则。

输出：

```text
reports/rule_candidates.md
```

## 12. 成功标准

第一版 MVP 成功的标准不是立刻赚钱，而是能回答：

- 盈利交易和亏损交易在入场前是否有明显区别？
- 钱包是否有稳定的仓位模式？
- 加仓更常发生在浮盈后还是亏损后？
- 亏损平仓是否有固定阈值或结构信号？
- 盈利平仓是否能卖在相对高位？
- 能否提炼出 3-5 条明确、可回测的候选规则？

如果无法找到明显差异，也是一种有价值的结论：说明这个钱包的盈利可能不是简单 K 线策略，而可能来自速度、信息源、链上事件、项目方关系、资金优势或少数极端盈利样本。

## 13. 推荐下一步开发

下一步不要直接优化现有 `analyze-kline` 输出，而是新建一条独立的数据集流水线：

```text
build-strategy-dataset
```

建议命令形态：

```bash
cargo run -- build-strategy-dataset <wallet> \
  --trades-source bitquery \
  --kline-source gmgn \
  --profit-samples 50 \
  --loss-samples 50 \
  --resolution 1m \
  --pre-minutes 20 \
  --post-minutes 20 \
  --out data/strategy_research/wallets/<wallet>
```

这条命令只负责：

- 拉交易。
- 聚合 position。
- 选样本。
- 下载 K 线。
- 保存数据。

后续再新增：

```text
extract-strategy-features
compare-strategy-features
generate-rule-candidates
```

这样研究数据、特征提取、规则生成会更清晰，也不会和原来的钱包分析 MVP 混在一起。
