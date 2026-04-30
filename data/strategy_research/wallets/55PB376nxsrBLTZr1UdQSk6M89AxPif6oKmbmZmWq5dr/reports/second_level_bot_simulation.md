# 秒级机器人策略模拟结果

## 口径

这次使用本地 100 个样本和对应 1m K 线做近似模拟。

重要限制：

~~~text
当前目录只有 1m OHLC K 线，不是 1s K 线或逐笔 swap。
15s、30s 的卖出检查只能退化为首根/次根 1m K 线判断。
同一分钟内先触发高点还是低点无法确定，脚本采用偏保守的止损优先和 close 退出口径。
买入所在 1m K 的高低点可能发生在买入前，退出判断从下一根 K 线开始。
1500ms 跟单延迟和买后涨幅 15% 过滤需要秒级行情，本次 1m 回测不启用。
~~~

输出明细：

~~~text
/Users/lijason/Desktop/blockchain/data/strategy_research/wallets/55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr/reports/second_level_bot_simulation_trades.csv
~~~

钱包原始表现：

| 指标 | 数值 |
|---|---:|
| 样本数 | 100 |
| 胜率 | 50.00% |
| 平均 ROI | 9.02% |

### 全部 A-F 分层模拟

| 指标 | 数值 |
|---|---:|
| 样本数 | 66 |
| 触发交易 | 66 |
| 跳过 | 0 |
| 胜/负 | 18 / 48 |
| 胜率 | 27.27% |
| 平均 ROI | 3.73% |
| 中位 ROI | -10.00% |
| ROI 合计 | 2.4621 |
| 中位持仓 | 60.0 秒 |


### 实盘优先 A-B 模拟

| 指标 | 数值 |
|---|---:|
| 样本数 | 34 |
| 触发交易 | 34 |
| 跳过 | 0 |
| 胜/负 | 11 / 23 |
| 胜率 | 32.35% |
| 平均 ROI | 9.57% |
| 中位 ROI | -6.60% |
| ROI 合计 | 3.2542 |
| 中位持仓 | 60.0 秒 |


### 按入场层统计

| 分组 | 样本 | 交易 | 胜率 | 平均 ROI | 中位持仓 |
|---|---:|---:|---:|---:|---:|
| A_momentum_high_range | 18 | 18 | 38.89% | 13.10% | 60.0s |
| B_extreme_near_high | 16 | 16 | 25.00% | 5.60% | 60.0s |
| C_high_range_fallback | 4 | 4 | 25.00% | -0.10% | 60.0s |
| D_breakout_volume | 11 | 11 | 36.36% | 4.18% | 60.0s |
| E_green_continuation | 12 | 12 | 0.00% | -9.86% | 60.0s |
| F_short_rebound_scout | 5 | 5 | 40.00% | -1.29% | 60.0s |


### 跳过原因

| 原因 | 数量 |
|---|---:|
| no_entry_layer | 34 |

### 退出原因

| 退出原因 | 交易 | 胜率 | 平均 ROI | 中位持仓 |
|---|---:|---:|---:|---:|
| explosive_runner_trailing_stop | 1 | 100.00% | 148.00% | 60.0s |
| failed_15s_stop_1m_approx | 4 | 0.00% | -8.94% | 60.0s |
| failed_no_push_1m_approx | 10 | 20.00% | -1.40% | 60.0s |
| hard_stop | 36 | 0.00% | -10.00% | 60.0s |
| normal_trailing_stop | 5 | 100.00% | 21.29% | 60.0s |
| strong_hard_tp | 4 | 100.00% | 60.00% | 60.0s |
| strong_trailing_stop | 3 | 100.00% | 41.55% | 60.0s |
| weak_30s_1m_approx | 3 | 100.00% | 12.27% | 60.0s |

### 最好 10 笔

| mint | 层 | 机器人 ROI | 钱包 ROI | 退出原因 | 持仓 |
|---|---|---:|---:|---|---:|
| 2Ku7NnBci7sMSLRYV6xpESJFYmg9FTizpX2wTqXopump | A_momentum_high_range | 148.00% | 1.30% | explosive_runner_trailing_stop | 60s |
| Gj2wMDacswzYG43CgpwCcWRihPhGsLQJf5F11hhvpump | A_momentum_high_range | 60.00% | 24.67% | strong_hard_tp | 60s |
| E4VuwYbTv5RmmqJGQhKdoxUWZzJGNxUXyDcsP73apzJu | B_extreme_near_high | 60.00% | 43.09% | strong_hard_tp | 60s |
| 9LdQTLF47JL1X3gwGcqZuBBPqE5qAyNhenmJYxDppump | B_extreme_near_high | 60.00% | -0.35% | strong_hard_tp | 60s |
| DiqmbYhdd7HoytAabr5AQdTBrB8DMTupgukE2ZHEpump | D_breakout_volume | 60.00% | -4.00% | strong_hard_tp | 60s |
| HS1Vh4K1rpXLAw2RUNx8MCjWb9arnE6qQnXyBZ47pump | A_momentum_high_range | 48.42% | 54.39% | strong_trailing_stop | 60s |
| 27sFia5GZfmt44iiDAym8Y2TL2duTewvYAjkLavhpump | A_momentum_high_range | 42.54% | 20.96% | strong_trailing_stop | 60s |
| 8TwGaAgqQ9ovYfNwBGBLJbnYqRKmBEQiLNHRBSeTpump | D_breakout_volume | 33.70% | 4.71% | strong_trailing_stop | 60s |
| 6eZAb5VQBD75KuEMEuUAVfsdnjikVxZmHCggLy4Ppump | C_high_range_fallback | 29.61% | 1.76% | normal_trailing_stop | 60s |
| G76fNn6QnTtQCWtGUKA4H9yQ6YNMn4aPSAeQxb9Jpump | A_momentum_high_range | 29.28% | 135.70% | normal_trailing_stop | 60s |

### 最差 10 笔

| mint | 层 | 机器人 ROI | 钱包 ROI | 退出原因 | 持仓 |
|---|---|---:|---:|---|---:|
| 9zr5VVFzpgUf9wnk2a9bkgdZxNXkPJ7XoJXuBidSpump | E_green_continuation | -10.00% | 12.62% | hard_stop | 60s |
| 5nYVDeouoW7ahb1svHewcDcRzexezCmWqJdk4PsApump | C_high_range_fallback | -10.00% | 32.92% | hard_stop | 60s |
| 2L33pPAAJkAR23KWv7mR2k2F9KKYhpWt7k2ArwYmpump | D_breakout_volume | -10.00% | 3.72% | hard_stop | 60s |
| HXYZgD8QoE3oGnXAdw1CztmDTDoygbuRU9ALTdX4pump | D_breakout_volume | -10.00% | 2.44% | hard_stop | 60s |
| 9uaNxDribD1UC7MUXQogBVdUscQBVHBuCzh5PrJ5pump | D_breakout_volume | -10.00% | 4.32% | hard_stop | 60s |
| 3pbggCjSuDLSB4iQ4v1Uc6HjbMT1hGqBd8AMKNxUpump | C_high_range_fallback | -10.00% | 167.30% | hard_stop | 60s |
| BDDumgWwRv5zynmmjxKALavMAHgPh6UocJRQs6Hmpump | E_green_continuation | -10.00% | 180.78% | hard_stop | 60s |
| H6SBg5q554YUFBwh3LqZJgfMxX4eofPdh9RXHFpBpump | B_extreme_near_high | -10.00% | 50.40% | hard_stop | 60s |
| 2vDVTKZNE96oJfeXeHQ618BGKS9ud493KReqgKXcpump | F_short_rebound_scout | -10.00% | -2.04% | hard_stop | 60s |
| 524zXrkbMkoDF8tZSy45PwTNb9pNgvwx5VSdk2R4pump | B_extreme_near_high | -10.00% | 98.06% | hard_stop | 60s |

## 解读

1m K 线模拟更适合看入场层质量和退出规则是否过紧，不适合精确评估 15s/30s 秒级止损。

如果要验证文档里的真实秒级策略，需要补充 1s K 线或逐笔 swap。那时可以准确重放：

~~~text
t+15s 失败单
t+30s 弱动能
10s 无新高
10%-22% 峰值回撤
爆拉分批止盈
~~~
