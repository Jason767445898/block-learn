# GMGN 钱包交易策略逆向解析

本项目的目标是：通过分析目标钱包在 GMGN / pump.fun 上的历史交易记录和对应 K 线，逆向归纳该钱包的可重复交易规则。

核心问题不是简单判断某笔交易盈亏，而是解释并验证：

- 因为什么买：买入前是否有动量、放量、突破、回踩、低位反弹等信号。
- 因为什么卖：卖出是否来自止盈、回撤、止损、时间退出或趋势失效。
- 因为什么加仓：加仓发生在浮盈、突破、回踩确认、亏损摊平还是固定拆单。
- 因为什么亏损平仓：是否是快速止损、动量失败、迟到止损或卖出后有效避险。

主文档：

- [GMGN 钱包策略逆向解析工作流](docs/GMGN_WALLET_STRATEGY_REVERSE_ENGINEERING.md)

## 快速运行

需要先配置 API key：

```bash
export BITQUERY_TOKEN="你的 Bitquery access token"
export GMGN_API_KEY="你的 GMGN API key"
```

运行默认钱包研究流程：

```bash
scripts/run_strategy_research.sh
```

分析其他钱包：

```bash
scripts/run_strategy_research.sh <wallet>
```

脚本会依次执行：

1. `build-strategy-dataset`
2. `extract-strategy-features`
3. `generate-rule-candidates`

## 当前目录

```text
src/                         Rust 分析程序
scripts/                     一键研究脚本
data/strategy_research/      钱包样本、K 线、特征和报告
docs/                        项目说明和合并后的研究文档
docs/archive/                已归档的旧计划、旧使用说明和非核心资料
```

## 当前结论摘要

默认样本钱包：

```text
55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr
```

当前 100 个平衡样本显示，该钱包更像是短线动量/高位突破型交易，而不是低位抄底型交易。样本内最强候选入场规则是：

```text
pre_5m_return >= 0.25
AND entry_range_position >= 0.75
```

该规则只覆盖部分钱包买点，但在当前平衡样本中筛出更高质量的交易子集。它仍是研究候选规则，不是可直接实盘的交易系统。
