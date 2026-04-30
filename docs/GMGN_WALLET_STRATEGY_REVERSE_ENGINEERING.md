# GMGN 钱包策略逆向解析工作流

## 1. 项目目标

本项目要做的是：用目标钱包的历史交易记录和 GMGN K 线，反推该钱包可能遵循的交易策略和交易规则。

需要回答的问题：

- 为什么买：买入前是否出现上涨、放量、突破、回踩、低位反弹、连续阴跌等信号。
- 为什么买多少：买入金额是否与信号强度、波动率、历史买入均值、前序盈亏有关。
- 为什么加仓：加仓发生在浮盈扩大、突破加速、回踩确认、亏损摊平还是固定拆单。
- 为什么卖：卖出是否发生在冲高、回撤、跌破结构、止盈、止损或超时。
- 为什么亏损卖：亏损平仓是快速止损、动量失败、迟到止损，还是卖出后有效避险。

注意：这里的“原因”不是读心，而是用成交点附近可观察的市场状态做高置信度推断。K 线能解释市场环境，不能证明钱包是否拥有私有信息、群消息、速度优势或关联钱包协同。

## 2. 核心研究对象

最小研究单位是 position，而不是单笔 trade：

```text
wallet + mint + 从首次买入到最后一次卖出
```

原因是单笔成交只能看到一个点，无法完整解释：

- 首次买入前的市场结构。
- 买入后是否马上浮盈或回撤。
- 是否分批加仓。
- 平仓是否接近局部高点。
- 卖出后是否继续跌，证明止损有效。

MVP 阶段先按 `wallet + mint` 聚合。如果同一个 mint 出现多段明显分离的交易周期，后续再拆成多个 position。

## 3. 当前工作流

### Step 1: 构建研究数据集

命令：

```bash
cargo run -- build-strategy-dataset <wallet> \
  --trades-source bitquery \
  --kline-source gmgn \
  --days 30 \
  --limit 2000 \
  --profit-samples 50 \
  --loss-samples 50 \
  --resolution 1m \
  --pre-minutes 20 \
  --post-minutes 20
```

作用：

- 用 Bitquery 拉取目标钱包 pump.fun 交易。
- 聚合为 position。
- 选择最近 50 个盈利已平仓 position。
- 选择最近 50 个亏损已平仓 position。
- 用 GMGN 下载每个 position 的 1m K 线。
- K 线窗口覆盖 `first_buy_time - 20m` 到 `last_sell_time + 20m`。

输出：

```text
data/strategy_research/wallets/<wallet>/
  positions.csv
  selected_positions.csv
  klines/*.json
  reports/dataset_summary.md
```

### Step 2: 提取策略特征

命令：

```bash
cargo run -- extract-strategy-features \
  --dataset data/strategy_research/wallets/<wallet>
```

输出：

```text
features/entry_features.csv
features/holding_features.csv
features/exit_features.csv
features/position_features.csv
reports/feature_comparison.md
```

特征分三段：

- 买入前特征：解释为什么买。
- 持仓中特征：解释为什么拿、为什么加仓、能忍受多大回撤。
- 平仓前后特征：解释为什么卖，以及卖出是否有效。

### Step 3: 生成候选规则

命令：

```bash
cargo run -- generate-rule-candidates \
  --dataset data/strategy_research/wallets/<wallet> \
  --min-matches 8 \
  --top 25
```

输出：

```text
reports/rule_candidates.csv
reports/rule_candidates.md
```

这一步会枚举入场侧候选规则，统计命中数、盈利数、亏损数、样本内胜率、平均 ROI、平均 PnL、持仓时间、最大浮盈、最大回撤和卖出效率。

### 一键脚本

```bash
scripts/run_strategy_research.sh <wallet>
```

可调参数：

```bash
DAYS=30 \
LIMIT=2000 \
PROFIT_SAMPLES=50 \
LOSS_SAMPLES=50 \
RESOLUTION=1m \
PRE_MINUTES=20 \
POST_MINUTES=20 \
MIN_MATCHES=8 \
TOP=25 \
scripts/run_strategy_research.sh <wallet>
```

## 4. 关键特征解释

买入前：

```text
pre_5m_return              买入前 5 分钟涨跌幅
pre_20m_return             买入前 20 分钟涨跌幅
pre_5m_volume_spike        买入附近成交量相对前序均量倍数
entry_range_position       买入点在最近 20 分钟高低区间的位置
```

持仓中：

```text
max_runup_during_holding   持仓期间最大浮盈
max_drawdown_during_holding 持仓期间最大不利回撤
holding_seconds            持仓秒数
```

平仓：

```text
exit_efficiency            卖出点相对持仓期间高点的位置
post_exit_20m_return       卖出后 20 分钟走势
```

实时入场规则只能使用入场前可见字段。`max_runup`、`max_drawdown`、`exit_efficiency` 属于事后字段，只能用于理解和验证，不能直接作为实时入场条件。

## 5. 当前样本结论

默认分析钱包：

```text
55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr
```

数据集：

```text
trades: 349
positions: 137
closed positions: 135
profitable closed positions: 59
losing closed positions: 76
selected: 50 profit / 50 loss
klines: 100 saved / 0 skipped
```

盈利组和亏损组的主要差异：

```text
盈利组 pre_5m_return: 60.25%
亏损组 pre_5m_return: 8.70%

盈利组 pre_20m_return: 75.01%
亏损组 pre_20m_return: 14.42%

盈利组 pre_5m_volume_spike: 14.20x
亏损组 pre_5m_volume_spike: 2.33x

盈利组 entry_range_position: 0.7355
亏损组 entry_range_position: 0.5565

盈利组 exit_efficiency: 0.7984
亏损组 exit_efficiency: 0.6968
```

初步解释：该钱包在当前样本里更像是做放量上涨后的动量/突破交易，而不是低位抄底交易。盈利样本通常在买入前已经更强，买入点也更靠近局部高位。

## 6. 当前候选规则

样本内最强候选入场规则：

```text
pre_5m_return >= 0.5000
AND entry_range_position >= 0.7500
```

当前平衡样本结果：

```text
matches: 10
profit/loss: 9 / 1
win rate: 90.00%
avg ROI: 36.48%
avg PnL: 0.2080 SOL
median hold: 95s
```

覆盖更广的候选规则：

```text
pre_5m_return >= 0.2500
AND entry_range_position >= 0.7500
```

当前平衡样本结果：

```text
matches: 18
profit/loss: 16 / 2
win rate: 88.89%
avg ROI: 34.33%
median hold: 81s
```

加入量能确认：

```text
pre_5m_return >= 0.1000
AND pre_5m_volume_spike >= 1.5000
AND entry_range_position >= 0.7500
```

当前平衡样本结果：

```text
matches: 10
profit/loss: 9 / 1
win rate: 90.00%
avg ROI: 33.11%
avg PnL: 0.1816 SOL
median hold: 89s
```

解读：

- 规则更像是在筛选“钱包买入强势 token 时的高质量子集”，不是复制钱包所有买点。
- 当前规则覆盖率不高，但命中交易质量明显高于全样本。
- 样本是 50 盈利 / 50 亏损的平衡样本，胜率只能说明区分度，不能代表真实交易胜率。

## 7. 手工 GMGN 交易规则草案

当前可读成一个 playbook：

```yaml
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
```

出场不适合只用固定百分比，应在买入后动态分类：

```text
15 秒：如果 ROI <= -5%，视为失败入场，快速退出。
30 秒：如果最大 ROI 仍低于 +15%，视为动量不足，退出或收紧。
60 秒：如果 ROI >= +50% 且继续创新高，进入强势追踪。
```

粗略分类：

```text
failed entry:     -5% 到 -10%，快速止损
normal rally:     +15% 到 +25%，不创新高就卖
strong rally:     +40% 到 +60%，用 trailing stop
explosive rally:  +100% 以上，考虑分批止盈并保留 runner
```

这套卖出规则目前只做了 1m 级别方向性回放，不能精确复原 15s/30s 的链上真实决策。

## 8. 现有规则回放结果

基于 `position_features.csv` 的入场规则回放：

```text
base rule:
pre_5m_return >= 25%
AND entry_range_position >= 0.75

matches: 18 / 100
wins/losses: 16 / 2
win rate: 88.9%
avg ROI: 34.33%
median hold: 78s
```

加入量能：

```text
base rule
AND volume_spike >= 1.5x

matches: 10 / 100
wins/losses: 9 / 1
win rate: 90.0%
avg ROI: 33.11%
median hold: 82s
```

卖出规则回放：

```text
all positions:
same P/L sign: 47/100
within 60s: 67
within 120s: 89
median abs time diff: 40s

entry base matches:
same P/L sign: 13/18
within 60s: 10
within 120s: 15
median abs time diff: 52s
```

结论：入场规则能较好筛出高质量子集；卖出规则只算部分验证，受限于 1m K 线粒度和真实成交价格缺失。

## 9. 当前项目结构

```text
.
├── README.md
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs
│   ├── strategy_dataset.rs       # 构建 position 数据集并下载 GMGN K 线
│   ├── strategy_features.rs      # 提取入场、持仓、平仓特征
│   ├── strategy_rules.rs         # 生成候选入场规则
│   └── wallet_analyzer.rs        # 早期钱包分析命令，暂保留兼容
├── scripts/
│   └── run_strategy_research.sh
├── data/
│   └── strategy_research/
│       └── wallets/<wallet>/
│           ├── positions.csv
│           ├── selected_positions.csv
│           ├── klines/
│           ├── features/
│           └── reports/
└── docs/
    ├── GMGN_WALLET_STRATEGY_REVERSE_ENGINEERING.md
    └── archive/
```

归档原则：

- 根目录只保留项目入口、代码、脚本和数据。
- 逆向解析主流程统一看本文件。
- 旧的阶段性计划、早期钱包分析说明、机器人架构说明放入 `docs/archive/`。
- 数据目录中的 `reports/*.md` 是运行产物，保留在对应钱包目录，方便追溯每次样本结果。

## 10. 下一步优先级

1. 做样本外验证：用未进入 100 个平衡样本的 position 测当前规则。
2. 统计真实胜率：不要用 50/50 平衡样本估计实盘胜率。
3. 补交易级数据：保存 exact buy/sell timestamp、side、token amount、SOL amount、execution price。
4. 提高卖出回放粒度：至少支持秒级成交点，避免 1m K 线掩盖快速止盈止损。
5. 加入滑点、手续费、延迟假设：尤其是 GMGN 跟单时的买入延迟和价格漂移。
6. 分析盈利集中度：去掉最大盈利的 1-3 笔后，规则是否仍有效。
7. 纸面交易验证：先记录信号，不发交易；结果稳定后再小资金验证。

## 11. 风险边界

- 当前结果是研究工具输出，不是自动交易系统。
- 候选规则只能说明样本内有区分度，不能证明未来盈利。
- GMGN K 线价格和 Bitquery 成交价格可能存在单位差异，不能混用绝对价格。
- pump.fun 资产波动极大，单根 K 线 high/low 可能异常，回测需要抗异常值。
- 目标钱包可能有速度、信息源、资金规模或多钱包协同优势，单地址 K 线逆向无法完全复刻。
- 直接实盘前必须做样本外、滑点、手续费、延迟和小资金验证。
