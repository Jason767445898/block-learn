# pump.fun 钱包策略与行为分析 MVP 使用说明

这个项目当前提供三类分析：

- `analyze`：分析钱包在 pump.fun 上的买卖策略。
- `behaviors`：分析钱包是否有 LP、发币、swap、转账、close account 等其他链上行为。
- `analyze-kline`：把钱包成交点叠到 K 线上，逆向判断追涨、抄底、量能确认、止损、出场效率等策略特征。

## 1. 每次打开新终端先做什么

环境变量只在当前终端窗口里生效。你每次重启终端、开新 tab、或新开一个 shell，都需要重新配置 API key。

进入项目目录：

```bash
cd ~/Desktop/blockchain
```

配置你需要用到的 API：

```bash
export BITQUERY_TOKEN="你的 Bitquery access token"
export HELIUS_API_KEY="你的 Helius API key"
export GMGN_API_KEY="你的 GMGN API key"
```

检查是否配置成功：

```bash
echo $BITQUERY_TOKEN
echo $HELIUS_API_KEY
echo $GMGN_API_KEY
```

如果能打印出值，说明当前终端已配置。不要把这些 key 发给别人，也不要提交到 git。

如果你不想每次手动 export，可以写入 `~/.zshrc`：

```bash
nano ~/.zshrc
```

添加：

```bash
export BITQUERY_TOKEN="你的 Bitquery access token"
export HELIUS_API_KEY="你的 Helius API key"
export GMGN_API_KEY="你的 GMGN API key"
```

保存后执行：

```bash
source ~/.zshrc
```

注意：写入 `~/.zshrc` 后，这些 key 会长期保存在本机 shell 配置里。只在你确认电脑安全时这样做。

## 2. 快速验证

不需要任何 API key，直接跑内置样例：

```bash
cargo run -- analyze <wallet> --source sample
cargo run -- behaviors <wallet> --source sample
cargo run -- analyze-kline <wallet> --trades-source sample --kline-source sample
```

示例：

```bash
cargo run -- analyze 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr --source sample
cargo run -- behaviors 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr --source sample
cargo run -- analyze-kline 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr --trades-source sample --kline-source sample
```

`sample` 模式使用程序内置假数据，只用于确认程序流程、策略标签和报告格式是否正常，不代表该钱包真实链上行为。

## 3. API 获取与用途

### Bitquery

用途：

- `analyze --source bitquery`
- `analyze-kline --trades-source bitquery`
- 拉取 pump.fun 历史买卖记录、PnL、持仓时间、仓位等。

申请入口：

- 官网：https://bitquery.io/
- 控制台/IDE：https://ide.bitquery.io/
- 授权文档：https://docs.bitquery.io/docs/authorisation/how-to-use/
- pump.fun API 文档：https://docs.bitquery.io/docs/blockchain/Solana/Pumpfun/Pump-Fun-API/

获取步骤：

1. 打开 Bitquery IDE：https://ide.bitquery.io/
2. 注册或登录账号。
3. 进入顶部导航里的 `Authorization`。
4. 如果还没有应用，点击 `New Application` 创建 application。
5. 在 application 右侧点击 `Tokens`。
6. 创建或复制 `Access Token`。
7. 注意：程序需要的是 `Access Token`，不是 application `ID`。
8. 在终端配置：

```bash
export BITQUERY_TOKEN="你的 Bitquery access token"
```

### Helius

用途：

- `behaviors --source helius`
- 分析钱包全量行为，尤其是 LP、发币、swap、转账、close account。

申请入口：

- 官网：https://www.helius.dev/
- 控制台：https://dashboard.helius.dev/
- Enhanced Transactions 文档：https://www.helius.dev/docs/api-reference/enhanced-transactions/gettransactionsbyaddress

获取步骤：

1. 打开 Helius Dashboard：https://dashboard.helius.dev/
2. 注册或登录账号。
3. 创建一个 project/app。
4. 进入项目后复制 API key。
5. 在终端配置：

```bash
export HELIUS_API_KEY="你的 Helius API key"
```

### GMGN

用途：

- `analyze-kline --kline-source gmgn`
- 拉取 token K 线，辅助判断钱包买卖点在 K 线结构中的位置。

申请入口：

- GMGN Agent API：https://gmgn.ai/ai
- 文档：https://docs.gmgn.ai/index/gmgn-agent-api

获取步骤：

1. 打开 https://gmgn.ai/ai
2. 登录 GMGN。
3. 按页面要求创建 API key。
4. 如果页面要求 public key，按 GMGN 文档生成 Ed25519 public key。
5. 复制 GMGN API key。
6. 在终端配置：

```bash
export GMGN_API_KEY="你的 GMGN API key"
```

GMGN K 线当前通过 `npx -y gmgn-cli market kline` 获取，所以本机需要可用的 Node.js/npm。

## 4. pump.fun 交易策略分析

用 Bitquery 拉真实 pump.fun 交易：

```bash
cargo run -- analyze <wallet> --source bitquery --days 7 --limit 10
```

示例：

```bash
cargo run -- analyze 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr --source bitquery --days 7 --limit 10
```

如果成功，再逐步加大样本：

```bash
cargo run -- analyze <wallet> --source bitquery --days 7 --limit 50
cargo run -- analyze <wallet> --source bitquery --days 14 --limit 100
```

输出包含：

- 买卖次数、交易 token 数
- 总买入 SOL、总卖出 SOL、已实现 PnL
- 胜率、平均买入仓位
- 中位持仓时间
- 盈利集中度
- 策略标签
- 盈亏最高的 position 摘要

CSV 离线模式：

```bash
cargo run -- analyze <wallet> --source csv --file trades.csv
```

交易 CSV 表头：

```csv
timestamp,tx_hash,wallet,mint,side,sol_amount,token_amount,price_sol,token_created_at
```

## 5. LP / 发币 / 其他行为分析

用 Helius 拉钱包全量行为：

```bash
cargo run -- behaviors <wallet> --source helius --limit 20
```

示例：

```bash
cargo run -- behaviors 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr --source helius --limit 20
```

输出包含：

- add liquidity 次数
- remove liquidity 次数
- create token 次数
- swap 次数
- transfer 次数
- close account 次数
- 交互过的协议
- 行为标签
- 关键事件列表

Helius JSON 离线模式：

```bash
cargo run -- behaviors <wallet> --source json --file helius-transactions.json
```

## 6. K 线逆向策略分析

`analyze-kline` 会把钱包成交点匹配到 K 线，分析它是在高位追涨、低位抄底、放量确认、低位止损，还是出场效率较高/较低。

### Bitquery 交易 + GMGN K 线

你已经配置 Bitquery 和 GMGN 时，优先跑这个：

```bash
cargo run -- analyze-kline <wallet> \
  --trades-source bitquery \
  --kline-source gmgn \
  --days 3 \
  --limit 3 \
  --resolution 1m
```

示例：

```bash
cargo run -- analyze-kline 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr \
  --trades-source bitquery \
  --kline-source gmgn \
  --days 3 \
  --limit 3 \
  --resolution 1m
```

确认能跑后再加大：

```bash
cargo run -- analyze-kline <wallet> \
  --trades-source bitquery \
  --kline-source gmgn \
  --days 7 \
  --limit 10 \
  --resolution 1m
```

可用 K 线周期：

```bash
--resolution 1m
--resolution 5m
--resolution 15m
--resolution 30m
--resolution 1h
--resolution 4h
--resolution 1d
```

注意：GMGN K 线价格通常是 USD 计价，而 Bitquery 交易价格是 SOL/token。为了避免单位混用，`--kline-source gmgn` 模式下，程序使用“成交所在 K 线的 close 价格”做结构分析，重点判断高低位、动量、量能和出场效率，而不是精确复原成交价格。

### Bitquery 交易 + K 线 CSV

如果 GMGN 超时，或你已经有外部 K 线数据，可以用 CSV：

```bash
cargo run -- analyze-kline <wallet> \
  --trades-source bitquery \
  --days 7 \
  --limit 50 \
  --kline-source csv \
  --kline-file klines.csv
```

K 线 CSV 表头：

```csv
mint,timestamp,open,high,low,close,volume
```

字段说明：

- `mint`：token mint，必须和交易里的 `mint` 一致。
- `timestamp`：K 线开始时间，RFC3339 格式，例如 `2026-04-28T13:00:00Z`。
- `open`：开盘价。
- `high`：最高价。
- `low`：最低价。
- `close`：收盘价。
- `volume`：成交量，可以是 token 数量或成交额，但同一文件内要保持一致。

### K 线输出指标

- `range position`：成交价位于近期 K 线区间的百分位。越接近 100%，越偏高位追涨；越接近 0%，越偏低位止损或抄底。
- `pre-trade momentum`：成交前一段 K 线的价格动量。
- `volume spike`：当前 K 线成交量相对历史均量的倍数。
- `max runup after trade`：买入后未来窗口内最大浮盈。
- `max drawdown after trade`：买入后未来窗口内最大不利回撤。
- `exit efficiency`：卖出价相对持仓期间最高价的位置，越高说明卖得越接近局部高位。

可调参数：

```bash
--lookback 10
--lookahead 10
```

`lookback` 控制成交前参考多少根 K 线，`lookahead` 控制成交后观察多少根 K 线。K 线如果是 1 分钟级别，`--lookback 10 --lookahead 10` 就是前后约 10 分钟。

## 7. 推荐使用流程

第一步，跑 sample，确认本地程序没问题：

```bash
cargo run -- analyze <wallet> --source sample
cargo run -- behaviors <wallet> --source sample
cargo run -- analyze-kline <wallet> --trades-source sample --kline-source sample
```

第二步，用 Bitquery 看 pump.fun 交易策略：

```bash
cargo run -- analyze <wallet> --source bitquery --days 7 --limit 10
```

第三步，用 Helius 看 LP / 发币 / 其他行为：

```bash
cargo run -- behaviors <wallet> --source helius --limit 20
```

第四步，用 GMGN K 线逆向买卖点结构：

```bash
cargo run -- analyze-kline <wallet> \
  --trades-source bitquery \
  --kline-source gmgn \
  --days 3 \
  --limit 3 \
  --resolution 1m
```

最终把三个报告合起来看：

- `analyze` 判断它怎么交易 pump.fun。
- `behaviors` 判断它是不是还做 LP、发币、迁移后流动性管理、做市或项目方相关操作。
- `analyze-kline` 判断买卖点处在 K 线结构的什么位置，例如高位追涨、量能确认、低位止损、出场效率等。

## 8. 常见问题

### Bitquery 返回 408 Request Time-out

查询太大或服务端响应慢。先减小范围：

```bash
cargo run -- analyze <wallet> --source bitquery --days 3 --limit 25
```

如果成功，再逐步加大 `--days` 和 `--limit`。

### Bitquery IDE 提示 no table can query DEXTrade

把查询里的：

```graphql
Solana(dataset: combined)
```

改成：

```graphql
Solana(dataset: realtime)
```

当前 Rust 代码已经使用 `dataset: realtime`。

### Helius 读取响应超时

先降低 limit：

```bash
cargo run -- behaviors <wallet> --source helius --limit 20
```

如果 API key 泄露，去 Helius Dashboard 里轮换 key。

### GMGN ConnectTimeoutError

通常是本机到 `openapi.gmgn.ai` 网络超时，或 GMGN 临时响应慢。程序会对每个 token 重试 3 次，并跳过仍失败的 token。

先降低查询规模：

```bash
cargo run -- analyze-kline <wallet> \
  --trades-source bitquery \
  --kline-source gmgn \
  --days 3 \
  --limit 3 \
  --resolution 1m
```

检查网络：

```bash
curl -I https://openapi.gmgn.ai
```

如果仍然全部超时，临时改用：

```bash
cargo run -- analyze-kline <wallet> \
  --trades-source bitquery \
  --kline-source csv \
  --kline-file klines.csv
```

### unknown kline source: gmgn

说明代码还不是最新版本。确认 `src/main.rs` 中已经支持 `--kline-source gmgn`，然后重新编译运行：

```bash
cargo check
```

## 9. 安全注意事项

- 不要把 `BITQUERY_TOKEN`、`HELIUS_API_KEY`、`GMGN_API_KEY` 发到聊天、截图、git 仓库或公开文档。
- 如果 key 泄露，立即去对应平台 revoke / regenerate。
- 当前程序不会发交易，不会签名，不会读取私钥，只做链上数据分析。
- LP、项目方、做市等判断是启发式规则，不是审计级结论。
- `sample` 输出正确只代表程序逻辑跑通，不代表钱包真实链上行为。
