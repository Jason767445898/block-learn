# 钱包策略复刻研究使用报告

本文档总结当前已经实现的三步流程：构建数据集、提取特征、生成候选规则。

目标是研究一个钱包的历史交易，找出它盈利交易与亏损交易之间的差异，进一步提炼可回测的候选交易规则。

注意：这套流程目前是研究工具，不是自动交易系统。候选规则只能说明“在当前样本里有区分度”，不能直接证明未来可盈利。

## 1. 前置条件

进入项目目录：

```bash
cd ~/Desktop/blockchain
```

每次新开终端都需要配置 API：

```bash
export BITQUERY_TOKEN="你的 Bitquery access token"
export GMGN_API_KEY="你的 GMGN API key"
```

检查：

```bash
echo $BITQUERY_TOKEN
echo $GMGN_API_KEY
```

如果为空，说明当前终端还没有配置。

## 2. 一键脚本

已经提供脚本：

```bash
scripts/run_strategy_research.sh
```

默认分析的钱包是：

```text
55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr
```

直接运行：

```bash
scripts/run_strategy_research.sh
```

分析其他钱包：

```bash
scripts/run_strategy_research.sh <wallet>
```

可通过环境变量调整参数：

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

脚本会依次执行：

1. `build-strategy-dataset`
2. `extract-strategy-features`
3. `generate-rule-candidates`

## 3. 第一步：构建策略研究数据集

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
- 选出最近 50 个盈利已平仓 position。
- 选出最近 50 个亏损已平仓 position。
- 用 GMGN 下载每个 position 的 1m K 线。
- K 线窗口为 `first_buy_time - 20m` 到 `last_sell_time + 20m`。

输出目录：

```text
data/strategy_research/wallets/<wallet>/
```

关键文件：

```text
positions.csv
selected_positions.csv
klines/*.json
reports/dataset_summary.md
```

当前钱包运行结果：

```text
trades: 349
positions: 137
closed positions: 135
profitable closed positions: 59
losing closed positions: 76
selected: 50 profit / 50 loss
klines: 100 saved / 0 skipped
```

## 4. 第二步：提取策略特征

命令：

```bash
cargo run -- extract-strategy-features \
  --dataset data/strategy_research/wallets/<wallet>
```

作用：

- 读取 `selected_positions.csv`。
- 读取 `klines/*.json`。
- 计算买入前、持仓中、平仓前后的特征。
- 输出四类 CSV 和一个对比报告。

输出文件：

```text
features/entry_features.csv
features/holding_features.csv
features/exit_features.csv
features/position_features.csv
reports/feature_comparison.md
```

主要特征：

```text
pre_5m_return
pre_20m_return
pre_5m_volume_spike
entry_range_position
max_runup_during_holding
max_drawdown_during_holding
exit_efficiency
post_exit_20m_return
```

当前钱包的特征对比摘要：

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

初步解释：

这个钱包在样本里更像是做放量上涨后的动量/突破交易，而不是低位抄底交易。

## 5. 第三步：生成候选规则

命令：

```bash
cargo run -- generate-rule-candidates \
  --dataset data/strategy_research/wallets/<wallet> \
  --min-matches 8 \
  --top 25
```

作用：

- 读取 `features/position_features.csv`。
- 枚举一批入场侧候选规则。
- 统计每条规则命中的样本数、盈利数、亏损数、胜率、平均 ROI、平均 PnL。
- 输出候选规则报告。

输出文件：

```text
reports/rule_candidates.csv
reports/rule_candidates.md
```

当前钱包最强候选规则：

```text
pre_5m_return >= 0.5000
AND entry_range_position >= 0.7500
```

当前样本内结果：

```text
matches: 10
profit/loss: 9 / 1
win rate: 90.00%
avg ROI: 36.48%
avg PnL: 0.2080 SOL
median hold: 95s
```

另一个候选规则：

```text
pre_5m_return >= 0.1000
AND pre_5m_volume_spike >= 1.5000
AND entry_range_position >= 0.7500
```

当前样本内结果：

```text
matches: 10
profit/loss: 9 / 1
win rate: 90.00%
avg ROI: 33.11%
avg PnL: 0.1816 SOL
median hold: 89s
```

## 6. 如何解读

当前结果说明：

- 盈利样本通常在买入前已经明显上涨。
- 盈利样本更常买在 20 分钟局部区间的偏高位置。
- 量能放大是有效增强条件，但最强信号是 `pre_5m_return + entry_range_position`。
- 持仓时间很短，候选规则的中位持仓大约 1-2 分钟。
- 样本内回撤很大，平均最大回撤可到 -40% 甚至更深，所以没有止损规则前不能直接实盘。

不要误读：

- 这里的 90% 胜率来自 50 盈利 / 50 亏损的平衡样本，不是钱包真实胜率。
- 当前规则是样本内规则，还没有做样本外验证。
- `max_runup`、`max_drawdown`、`exit_efficiency` 是事后分析字段，不能作为实时入场条件。

## 7. 推荐下一步

下一阶段应该做样本外验证：

- 用没有进入 100 个样本的 position 测这些规则。
- 检查规则命中后真实胜率、平均 ROI、最大回撤。
- 加入滑点、手续费、延迟假设。
- 再根据结果设计止盈、止损、超时退出规则。

建议新增命令：

```bash
cargo run -- validate-rule-candidates \
  --dataset data/strategy_research/wallets/<wallet> \
  --rules reports/rule_candidates.csv
```

在样本外验证前，不建议把候选规则接入自动交易。
