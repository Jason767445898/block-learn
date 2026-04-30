# GMGN 新数据钱包逆向分析实现步骤文档

## 1. 目标

使用 GMGN 上目标钱包交易过的品种的交易活动数据，逆向分析该钱包的交易策略和可复现规则。

本轮重点从“只看钱包自己的成交 + K 线”升级为：

- 钱包在什么市场活动环境下买入。
- 买入前目标品种是否出现资金涌入、交易人数增加、大单、连续买盘或聪明钱跟随。
- 加仓、减仓、清仓是否对应交易活动变化。
- 哪些活动信号能区分盈利交易和亏损交易。
- 哪些规则只能解释历史，哪些规则可以用于实时监控。

## 2. 数据范围

### 2.1 钱包交易数据

继续保留当前 position 级别研究单位：

```text
wallet + mint + 一段完整交易周期
```

需要字段：

- `wallet`
- `mint`
- `tx_hash`
- `timestamp`
- `side`
- `sol_amount`
- `token_amount`
- `price_sol`
- `realized_pnl_sol`
- `realized_roi`
- `holding_seconds`

### 2.2 GMGN 品种交易活动数据

新增每个 `mint` 在钱包买入前、持仓中、卖出后的交易活动快照或时间序列。

建议先做 1m 粒度，窗口与现有 K 线一致：

```text
first_buy_time - pre_minutes
到
last_sell_time + post_minutes
```

优先采集字段：

- `timestamp`
- `mint`
- `buy_count`
- `sell_count`
- `buy_volume_sol`
- `sell_volume_sol`
- `net_buy_volume_sol`
- `unique_buyers`
- `unique_sellers`
- `large_buy_count`
- `large_sell_count`
- `large_buy_volume_sol`
- `large_sell_volume_sol`
- `top_buyer_volume_share`
- `new_wallet_buy_count`
- `smart_wallet_buy_count`，如果 GMGN 可提供
- `holder_count`，如果 GMGN 可提供
- `market_cap` / `liquidity`，如果 GMGN 可提供

如果 GMGN API 只能提供成交明细，则先落原始成交，再离线聚合为上述 1m 活动特征。

### 2.3 K 线数据

继续使用现有 GMGN K 线：

- `open`
- `high`
- `low`
- `close`
- `volume`
- `resolution`

K 线主要用于价格结构，交易活动数据用于解释资金行为。

## 3. 目录与产物设计

每个钱包仍输出到：

```text
data/strategy_research/wallets/<wallet>/
```

新增目录：

```text
activity/
  <mint>__1m__<from>__<to>.json

features/
  activity_entry_features.csv
  activity_holding_features.csv
  activity_exit_features.csv
  enriched_position_features.csv

reports/
  activity_feature_comparison.md
  activity_rule_candidates.md
  gmgn_reverse_analysis_report.md
```

其中：

- `activity/*.json` 保存 GMGN 交易活动原始或聚合数据。
- `activity_entry_features.csv` 只包含买入前可见活动特征。
- `activity_holding_features.csv` 解释持仓和加仓行为。
- `activity_exit_features.csv` 解释卖出行为。
- `enriched_position_features.csv` 合并价格特征、活动特征和 position 标签。
- `gmgn_reverse_analysis_report.md` 作为最终策略逆向报告。

## 4. 实现阶段

### 阶段 1：确认 GMGN 新数据接口与字段映射

目标：把 GMGN 上“钱包交易品种的交易活动”定义成稳定数据结构。

步骤：

1. 梳理可用 GMGN 接口：K 线、成交明细、token 活动、聪明钱、holder、流动性。
2. 确认接口参数：`mint`、时间范围、分页、resolution、排序。
3. 确认速率限制和缓存策略。
4. 建立统一结构 `ActivityBucket` 或 `ActivityTrade`。
5. 写入样例 JSON，保证后续特征提取不依赖接口实时可用。

验收：

- 能对一个目标 `mint` 拉取指定时间窗口活动数据。
- 数据能保存到 `activity/`。
- 失败时记录 `exclude_reason`，不阻断整个钱包分析。

### 阶段 2：扩展数据集构建命令

目标：在 `build-strategy-dataset` 中同时保存 K 线和 GMGN 活动数据。

建议参数：

```bash
cargo run -- build-strategy-dataset <wallet> \
  --trades-source bitquery \
  --kline-source gmgn \
  --activity-source gmgn \
  --days 30 \
  --limit 3000 \
  --profit-samples 80 \
  --loss-samples 80 \
  --resolution 1m \
  --pre-minutes 60 \
  --post-minutes 60
```

需要改造：

- `src/strategy_dataset.rs`
- `scripts/run_strategy_research.sh`
- `docs/GMGN_WALLET_STRATEGY_REVERSE_ENGINEERING.md`

关键设计：

- `--activity-source gmgn|sample|none`
- `--activity-resolution 1m`
- `--activity-large-trade-sol-threshold`
- 下载失败不删除 position，只标记活动数据缺失。

验收：

- `selected_positions.csv` 增加 `activity_file` 或单独新增 `selected_activity.csv`。
- 每个 selected position 尽量有对应 `activity/*.json`。
- `dataset_summary.md` 增加 activity 成功/失败数量。

### 阶段 3：交易活动特征工程

目标：把 GMGN 活动数据转成可解释、可枚举规则的字段。

买入前特征：

- `pre_1m_net_buy_volume_sol`
- `pre_3m_net_buy_volume_sol`
- `pre_5m_net_buy_volume_sol`
- `pre_10m_net_buy_volume_sol`
- `pre_5m_buy_sell_ratio`
- `pre_5m_unique_buyers`
- `pre_5m_unique_buyers_growth`
- `pre_5m_large_buy_count`
- `pre_5m_large_buy_volume_sol`
- `pre_5m_large_buy_share`
- `pre_5m_top_buyer_volume_share`
- `pre_5m_smart_wallet_buy_count`
- `pre_5m_new_wallet_buy_count`
- `activity_acceleration_1m_vs_5m`

持仓中特征：

- `holding_net_buy_volume_sol`
- `holding_buy_sell_ratio`
- `net_buy_volume_before_add`
- `large_buy_before_add`
- `activity_peak_time_after_entry`
- `activity_fade_before_exit`
- `buyers_drop_from_peak`

卖出前后特征：

- `pre_exit_3m_net_buy_volume_sol`
- `pre_exit_5m_sell_pressure`
- `pre_exit_large_sell_count`
- `post_exit_5m_net_buy_volume_sol`
- `post_exit_20m_net_buy_volume_sol`
- `sell_before_activity_collapse`
- `missed_activity_after_exit`

验收：

- 输出 `activity_entry_features.csv`、`activity_holding_features.csv`、`activity_exit_features.csv`。
- 输出 `activity_feature_comparison.md`，比较盈利组和亏损组均值、中位数、分位数。
- 明确标记哪些字段是实时可用，哪些是事后验证字段。

### 阶段 4：构建价格 + 活动联合特征

目标：让规则不再只依赖价格 K 线，而是能表达“价格强 + 资金强”的组合。

新增 `enriched_position_features.csv`，合并：

- 当前 `position_features.csv`
- `activity_entry_features.csv`
- 持仓/卖出解释字段

典型联合特征：

- `pre_5m_return`
- `entry_range_position`
- `pre_5m_volume_spike`
- `pre_5m_net_buy_volume_sol`
- `pre_5m_buy_sell_ratio`
- `pre_5m_unique_buyers_growth`
- `pre_5m_large_buy_count`
- `pre_5m_top_buyer_volume_share`

验收：

- 同一个 position 一行。
- 缺失活动数据时字段为空，不影响已有价格规则分析。
- 报告中能比较“仅价格规则”和“价格 + 活动规则”。

### 阶段 5：规则挖掘与候选策略生成

目标：生成可解释的 GMGN 钱包交易规则。

规则分三类：

1. 入场规则：只能使用买入前可见数据。
2. 加仓规则：使用首次买入后、加仓前可见数据。
3. 出场解释规则：可用于理解卖出，但实时复刻需要单独验证。

候选入场规则示例：

```text
pre_5m_return >= 0.25
AND entry_range_position >= 0.75
AND pre_5m_net_buy_volume_sol >= X
AND pre_5m_buy_sell_ratio >= Y
```

```text
pre_5m_large_buy_count >= X
AND pre_5m_unique_buyers_growth >= Y
AND top_buyer_volume_share <= Z
```

需要新增或扩展：

- `src/strategy_rules.rs`
- 支持更多字段枚举。
- 支持三条件、四条件组合。
- 支持最小覆盖数、最大过拟合约束。
- 支持样本内与留出样本对比。

验收：

- 输出 `activity_rule_candidates.csv`。
- 输出 `activity_rule_candidates.md`。
- 每条规则包含：命中数、盈利数、亏损数、胜率、平均 ROI、平均 PnL、平均持仓时间、最大回撤、卖出效率。

### 阶段 6：验证与反过拟合

目标：避免把少数历史巧合当成策略。

验证方法：

- 时间切分：前 70% 生成规则，后 30% 验证规则。
- 盈亏平衡样本和全量样本都跑一遍。
- 最小命中数：默认不低于 10，样本扩大后建议不低于 20。
- 排除事后字段进入入场规则。
- 对比基线：钱包全量胜率、当前价格规则胜率、活动增强规则胜率。

报告需要展示：

- 样本内表现。
- 留出样本表现。
- 规则覆盖率。
- 规则稳定性。
- 最大连续亏损。
- 对手续费和滑点的敏感性。

验收：

- 报告明确写出“可复刻候选规则”和“仅用于解释的钱包行为模式”。
- 不把 `max_runup`、`exit_efficiency`、`post_exit_*` 作为实时入场条件。

### 阶段 7：最终逆向报告

目标：输出一份人能直接阅读和决策的分析报告。

报告结构：

```text
1. 数据集概览
2. 钱包交易画像
3. 盈利交易 vs 亏损交易差异
4. 买入前价格结构
5. 买入前 GMGN 交易活动结构
6. 加仓行为解释
7. 卖出行为解释
8. 候选入场规则
9. 候选风控/出场规则
10. 可复刻性评级
11. 风险和下一步
```

可复刻性评级建议：

- `A`：规则覆盖足够、留出样本仍有效、字段实时可用。
- `B`：样本内强，留出样本一般，需要继续扩样本。
- `C`：能解释钱包，但不适合复刻。
- `D`：高度依赖事后字段或疑似私有信息。

## 5. 推荐执行顺序

第一轮先做最小闭环：

1. 新增 GMGN activity 数据缓存。
2. 对 selected positions 下载活动数据。
3. 提取买入前活动特征。
4. 合并 `enriched_position_features.csv`。
5. 生成活动增强版入场规则。
6. 输出 `activity_rule_candidates.md`。

第二轮再扩展：

1. 加仓规则。
2. 出场解释规则。
3. 时间切分验证。
4. 全量样本验证。
5. 最终逆向报告。

## 5.1 当前已落地的无 Bitquery MVP

新增独立流程：

```text
src/gmgn_reverse/
scripts/run_gmgn_reverse_research.sh
data/gmgn_reverse/wallets/<wallet>/
```

运行 sample 闭环：

```bash
scripts/run_gmgn_reverse_research.sh
```

使用 GMGN 导出的本地钱包交易 CSV：

```bash
WALLET_TRADES_SOURCE=csv \
WALLET_TRADES_FILE=data/gmgn_exports/<wallet>_trades.csv \
ACTIVITY_SOURCE=csv-dir \
ACTIVITY_DIR=data/gmgn_exports/activity \
scripts/run_gmgn_reverse_research.sh <wallet>
```

当前流程不会调用 Bitquery API。第一阶段优先支持 GMGN 导出的 CSV/JSON 和活动数据目录，等 GMGN 钱包交易/活动接口字段确认后，再把 `gmgn-json` 扩展为直接抓取。

## 6. 关键风险

- GMGN 活动接口字段可能不稳定，需要缓存原始响应。
- 钱包可能依赖链上速度、私有群、关联钱包或机器人逻辑，公开活动数据只能解释可观察部分。
- 小样本规则容易过拟合，必须做时间切分验证。
- 平衡样本适合找差异，全量样本才适合估算真实收益。
- 活动数据和钱包成交时间必须统一到 UTC，并处理秒级/分钟级对齐误差。

## 7. 完成标准

本轮实现完成后，应能运行：

```bash
ACTIVITY_SOURCE=gmgn \
PRE_MINUTES=60 \
POST_MINUTES=60 \
PROFIT_SAMPLES=80 \
LOSS_SAMPLES=80 \
scripts/run_strategy_research.sh <wallet>
```

并生成：

```text
data/strategy_research/wallets/<wallet>/
  activity/
  features/activity_entry_features.csv
  features/enriched_position_features.csv
  reports/activity_feature_comparison.md
  reports/activity_rule_candidates.md
  reports/gmgn_reverse_analysis_report.md
```

最终判断标准：

- 能说明该钱包买入前最常见的 GMGN 活动信号。
- 能说明盈利交易和亏损交易在活动数据上的主要差异。
- 能给出一组只依赖实时可见字段的候选入场规则。
- 能明确哪些行为可复刻，哪些只是事后解释。
