import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";

const wallet = "55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr";
const root = resolve(import.meta.dirname, "..");
const walletDir = join(root, "data", "strategy_research", "wallets", wallet);
const reportsDir = join(walletDir, "reports");
const positionsPath = join(walletDir, "selected_positions.csv");
const entryFeaturesPath = join(walletDir, "features", "entry_features.csv");
const klinesDir = join(walletDir, "klines");
const tradesCsvPath = join(reportsDir, "second_level_bot_simulation_trades.csv");
const reportPath = join(reportsDir, "second_level_bot_simulation.md");

const config = {
  basePositionSol: 0.05,
  maxBuyDelayMs: 1500,
  skipIfPriceMovedAfterWalletBuyPct: 0.15,
  hardStopRoi: -0.1,
  failedStopRoi: -0.05,
  noPushMaxRoi: 0.08,
  weakMaxRoi: 0.15,
  normalActivateRoi: 0.15,
  strongActivateRoi: 0.4,
  explosiveActivateRoi: 1,
  explosiveTp1Roi: 1.5,
  explosiveTp1SellPct: 50,
  normalTrailingDrawdown: -0.1,
  strongTrailingDrawdown: -0.15,
  explosiveTrailingDrawdown: -0.22,
  strongHardTakeProfitRoi: 0.6,
  explosiveHardTakeProfitRoi: 4,
  normalMaxHoldSeconds: 90,
  strongMaxHoldSeconds: 120,
  explosiveMaxHoldSeconds: 300,
};

function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = "";
  let quoted = false;

  for (let i = 0; i < text.length; i += 1) {
    const char = text[i];
    const next = text[i + 1];

    if (quoted) {
      if (char === '"' && next === '"') {
        field += '"';
        i += 1;
      } else if (char === '"') quoted = false;
      else field += char;
      continue;
    }

    if (char === '"') quoted = true;
    else if (char === ",") {
      row.push(field);
      field = "";
    } else if (char === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else if (char !== "\r") field += char;
  }

  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }

  const [headers, ...body] = rows.filter((items) => items.some((item) => item !== ""));
  return body.map((items) =>
    Object.fromEntries(headers.map((header, index) => [header, items[index] ?? ""])),
  );
}

function csvEscape(value) {
  if (value === null || value === undefined) return "";
  const text = String(value);
  if (/[",\n\r]/.test(text)) return `"${text.replaceAll('"', '""')}"`;
  return text;
}

function toNumber(value) {
  if (value === "" || value === null || value === undefined) return null;
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function fmtPct(value) {
  if (value === null || value === undefined || !Number.isFinite(value)) return "";
  return `${(value * 100).toFixed(2)}%`;
}

function fmtNum(value, digits = 4) {
  if (value === null || value === undefined || !Number.isFinite(value)) return "";
  return value.toFixed(digits);
}

function findKlineFile(row) {
  if (row.kline_file) {
    const absolute = resolve(root, row.kline_file);
    try {
      readFileSync(absolute);
      return absolute;
    } catch {
      // Use mint lookup below.
    }
  }

  const file = readdirSync(klinesDir).find((name) => name.startsWith(`${row.mint}_1m_`));
  return file ? join(klinesDir, file) : null;
}

function loadCandles(path) {
  const json = JSON.parse(readFileSync(path, "utf8"));
  return (json.list ?? [])
    .map((item) => ({
      time: Number(item.time),
      open: Number(item.open),
      high: Number(item.high),
      low: Number(item.low),
      close: Number(item.close),
      volume: Number(item.volume),
    }))
    .filter((item) =>
      Number.isFinite(item.time) &&
      Number.isFinite(item.open) &&
      Number.isFinite(item.high) &&
      Number.isFinite(item.low) &&
      Number.isFinite(item.close),
    )
    .sort((a, b) => a.time - b.time);
}

function findEntryIndex(candles, buyMs) {
  if (candles.length === 0) return -1;
  let bestIndex = 0;
  let bestDistance = Math.abs(candles[0].time - buyMs);
  for (let index = 1; index < candles.length; index += 1) {
    const distance = Math.abs(candles[index].time - buyMs);
    if (distance < bestDistance) {
      bestIndex = index;
      bestDistance = distance;
    }
  }
  return bestIndex;
}

function matchEntryLayer(feature) {
  const pre5 = toNumber(feature.pre_5m_return);
  const range = toNumber(feature.entry_range_position);
  const distanceHigh = toNumber(feature.distance_to_20m_high);
  const distanceLow = toNumber(feature.distance_to_20m_low);
  const red = toNumber(feature.consecutive_red_candles);
  const pre1 = toNumber(feature.pre_1m_return);
  const label = feature.entry_label;

  if (pre5 !== null && range !== null && pre5 >= 0.25 && range >= 0.75) {
    return { name: "A_momentum_high_range", sizeMultiplier: 1.0, live: true };
  }

  if (range !== null && distanceHigh !== null && range >= 0.85 && distanceHigh >= -0.05) {
    return { name: "B_extreme_near_high", sizeMultiplier: 0.8, live: true };
  }

  if (label === "high_range_entry" && range !== null && range >= 0.85) {
    return { name: "C_high_range_fallback", sizeMultiplier: 0.6, live: false };
  }

  if (label === "breakout_volume_entry") {
    return { name: "D_breakout_volume", sizeMultiplier: 0.6, live: false };
  }

  if (distanceLow !== null && red !== null && distanceLow >= 1.0 && red <= 0) {
    return { name: "E_green_continuation", sizeMultiplier: 0.3, live: false };
  }

  if (pre1 !== null && range !== null && pre1 >= 0.1 && range <= 0.5) {
    return { name: "F_short_rebound_scout", sizeMultiplier: 0.2, live: false };
  }

  return null;
}

function candleWorstFirstRoi(candle, entryPrice) {
  return candle.low / entryPrice - 1;
}

function candleBestRoi(candle, entryPrice) {
  return candle.high / entryPrice - 1;
}

function simulateExit(candles, entryIndex, entryPrice) {
  let phase = "OBSERVE";
  let remainingPct = 100;
  let maxPrice = entryPrice;
  let maxRoi = 0;
  let lastNewHighTime = candles[entryIndex].time;
  let tp1Done = false;
  let tp1Time = null;
  let tp1Roi = null;
  let maxDrawdownFromPeak = 0;

  const firstExitIndex = Math.min(entryIndex + 1, candles.length - 1);
  for (let index = firstExitIndex; index < candles.length; index += 1) {
    const candle = candles[index];
    const holdSeconds = Math.max(0, Math.round((candle.time - candles[entryIndex].time) / 1000));
    const highRoi = candleBestRoi(candle, entryPrice);
    const lowRoi = candleWorstFirstRoi(candle, entryPrice);
    const closeRoi = candle.close / entryPrice - 1;

    if (candle.high > maxPrice) {
      maxPrice = candle.high;
      lastNewHighTime = candle.time;
    }
    maxRoi = Math.max(maxRoi, highRoi);

    const closeDrawdown = candle.close / maxPrice - 1;
    const lowDrawdown = candle.low / maxPrice - 1;
    maxDrawdownFromPeak = Math.min(maxDrawdownFromPeak, lowDrawdown);
    const noNewHighSeconds = Math.max(0, Math.round((candle.time - lastNewHighTime) / 1000));

    if (lowRoi <= config.hardStopRoi) {
      return finish("hard_stop", candle, holdSeconds, config.hardStopRoi);
    }

    if (holdSeconds >= 60 && closeRoi <= config.failedStopRoi) {
      return finish("failed_15s_stop_1m_approx", candle, holdSeconds, closeRoi);
    }

    if (holdSeconds >= 60 && maxRoi < config.noPushMaxRoi) {
      return finish("failed_no_push_1m_approx", candle, holdSeconds, closeRoi);
    }

    if (holdSeconds >= 60 && maxRoi >= config.explosiveActivateRoi) {
      phase = "EXPLOSIVE";
      if (!tp1Done && highRoi >= config.explosiveTp1Roi) {
        tp1Done = true;
        tp1Time = candle.time;
        tp1Roi = config.explosiveTp1Roi;
        remainingPct = 50;
      }
      if (lowDrawdown <= config.explosiveTrailingDrawdown) {
        return finish("explosive_runner_trailing_stop", candle, holdSeconds, closeRoi);
      }
      if (highRoi >= config.explosiveHardTakeProfitRoi) {
        return finish("explosive_hard_tp", candle, holdSeconds, config.explosiveHardTakeProfitRoi);
      }
      if (holdSeconds >= config.explosiveMaxHoldSeconds) {
        return finish("explosive_time_exit", candle, holdSeconds, closeRoi);
      }
      continue;
    }

    if (holdSeconds >= 60 && maxRoi >= config.strongActivateRoi) {
      phase = "STRONG";
      if (highRoi >= config.strongHardTakeProfitRoi) {
        return finish("strong_hard_tp", candle, holdSeconds, config.strongHardTakeProfitRoi);
      }
      if (lowDrawdown <= config.strongTrailingDrawdown) {
        return finish("strong_trailing_stop", candle, holdSeconds, closeRoi);
      }
      if (holdSeconds >= config.strongMaxHoldSeconds) {
        return finish("strong_time_exit", candle, holdSeconds, closeRoi);
      }
      continue;
    }

    if (holdSeconds >= 60 && maxRoi >= config.normalActivateRoi) {
      phase = "NORMAL";
      if (noNewHighSeconds >= 60) {
        return finish("normal_no_new_high_1m_approx", candle, holdSeconds, closeRoi);
      }
      if (lowDrawdown <= config.normalTrailingDrawdown) {
        return finish("normal_trailing_stop", candle, holdSeconds, closeRoi);
      }
      if (holdSeconds >= config.normalMaxHoldSeconds) {
        return finish("normal_time_exit", candle, holdSeconds, closeRoi);
      }
      continue;
    }

    if (holdSeconds >= 60 && maxRoi < config.weakMaxRoi) {
      return finish("weak_30s_1m_approx", candle, holdSeconds, closeRoi);
    }

    if (holdSeconds >= config.normalMaxHoldSeconds) {
      return finish("observe_time_exit", candle, holdSeconds, closeRoi);
    }

    function finish(exitReason, exitCandle, exitHoldSeconds, exitRoi) {
      const weightedRoi = tp1Done
        ? (tp1Roi * config.explosiveTp1SellPct + exitRoi * remainingPct) / 100
        : exitRoi;

      return {
        exitReason,
        phase,
        exitTime: new Date(exitCandle.time).toISOString(),
        exitPrice: entryPrice * (1 + exitRoi),
        exitRoi,
        weightedRoi,
        holdSeconds: exitHoldSeconds,
        maxRoi,
        maxDrawdownFromPeak,
        noNewHighSeconds,
        tp1Done,
        tp1Time: tp1Time ? new Date(tp1Time).toISOString() : "",
        tp1Roi,
      };
    }
  }

  const last = candles.at(-1);
  const exitRoi = last.close / entryPrice - 1;
  return {
    exitReason: "end_of_kline",
    phase,
    exitTime: new Date(last.time).toISOString(),
    exitPrice: last.close,
    exitRoi,
    weightedRoi: tp1Done
      ? (tp1Roi * config.explosiveTp1SellPct + exitRoi * remainingPct) / 100
      : exitRoi,
    holdSeconds: Math.round((last.time - candles[entryIndex].time) / 1000),
    maxRoi,
    maxDrawdownFromPeak,
    noNewHighSeconds: Math.round((last.time - lastNewHighTime) / 1000),
    tp1Done,
    tp1Time: tp1Time ? new Date(tp1Time).toISOString() : "",
    tp1Roi,
  };
}

function summarize(rows) {
  const traded = rows.filter((row) => row.bot_action === "TRADE");
  const roiSum = traded.reduce((sum, row) => sum + row.bot_roi, 0);
  const sortedRoi = traded.map((row) => row.bot_roi).sort((a, b) => a - b);
  const sortedHold = traded.map((row) => row.bot_hold_seconds).sort((a, b) => a - b);
  const median = (items) => {
    if (items.length === 0) return null;
    const middle = Math.floor(items.length / 2);
    return items.length % 2 ? items[middle] : (items[middle - 1] + items[middle]) / 2;
  };

  return {
    samples: rows.length,
    trades: traded.length,
    skipped: rows.length - traded.length,
    winners: traded.filter((row) => row.bot_roi > 0).length,
    losers: traded.filter((row) => row.bot_roi < 0).length,
    winRate: traded.length ? traded.filter((row) => row.bot_roi > 0).length / traded.length : 0,
    avgRoi: traded.length ? roiSum / traded.length : 0,
    medianRoi: median(sortedRoi),
    medianHold: median(sortedHold),
    totalRoi: roiSum,
  };
}

function groupBy(rows, key) {
  const groups = new Map();
  for (const row of rows) {
    const value = row[key] || "";
    if (!groups.has(value)) groups.set(value, []);
    groups.get(value).push(row);
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
}

mkdirSync(reportsDir, { recursive: true });

const positions = parseCsv(readFileSync(positionsPath, "utf8"));
const featureRows = parseCsv(readFileSync(entryFeaturesPath, "utf8"));
const featuresByMint = new Map(featureRows.map((row) => [row.mint, row]));

const results = positions.map((position) => {
  const feature = featuresByMint.get(position.mint);
  const layer = feature ? matchEntryLayer(feature) : null;
  const klineFile = findKlineFile(position);
  const buyMs = Date.parse(position.first_buy_time);

  const base = {
    mint: position.mint,
    sample_group: position.sample_group,
    wallet_roi: toNumber(position.realized_roi),
    wallet_hold_seconds: toNumber(position.holding_seconds),
    first_buy_time: position.first_buy_time,
    kline_file: klineFile ? basename(klineFile) : "",
    entry_layer: layer?.name ?? "",
    live_enabled: layer?.live ?? false,
    size_multiplier: layer?.sizeMultiplier ?? "",
    pre_5m_return: feature ? toNumber(feature.pre_5m_return) : null,
    entry_range_position: feature ? toNumber(feature.entry_range_position) : null,
    distance_to_20m_high: feature ? toNumber(feature.distance_to_20m_high) : null,
    pre_5m_volume_spike: feature ? toNumber(feature.pre_5m_volume_spike) : null,
    entry_label: feature?.entry_label ?? "",
  };

  if (!feature) {
    return { ...base, bot_action: "SKIP", skip_reason: "missing_entry_features" };
  }
  if (!layer) {
    return { ...base, bot_action: "SKIP", skip_reason: "no_entry_layer" };
  }
  if (!klineFile) {
    return { ...base, bot_action: "SKIP", skip_reason: "missing_kline" };
  }

  const candles = loadCandles(klineFile);
  const entryIndex = findEntryIndex(candles, buyMs);
  if (entryIndex < 0) {
    return { ...base, bot_action: "SKIP", skip_reason: "empty_kline" };
  }

  const entryCandle = candles[entryIndex];
  const entryPrice = entryCandle.close;
  // This guard needs wallet fill price and sub-second market price. With 1m candles,
  // using candle open/close would incorrectly reject valid signals.
  const movedAfterWalletBuy = null;

  const exit = simulateExit(candles, entryIndex, entryPrice);
  return {
    ...base,
    bot_action: "TRADE",
    skip_reason: "",
    bot_entry_time: new Date(entryCandle.time).toISOString(),
    bot_entry_price: entryPrice,
    bot_exit_time: exit.exitTime,
    bot_exit_price: exit.exitPrice,
    bot_roi: exit.weightedRoi,
    raw_exit_roi: exit.exitRoi,
    bot_hold_seconds: exit.holdSeconds,
    bot_max_roi: exit.maxRoi,
    bot_max_drawdown_from_peak: exit.maxDrawdownFromPeak,
    bot_no_new_high_seconds_at_exit: exit.noNewHighSeconds,
    exit_reason: exit.exitReason,
    exit_phase: exit.phase,
    tp1_done: exit.tp1Done,
    tp1_time: exit.tp1Time,
    tp1_roi: exit.tp1Roi,
    moved_after_wallet_buy: movedAfterWalletBuy,
  };
});

const headers = [
  "mint",
  "sample_group",
  "bot_action",
  "skip_reason",
  "entry_layer",
  "live_enabled",
  "size_multiplier",
  "wallet_roi",
  "wallet_hold_seconds",
  "bot_roi",
  "raw_exit_roi",
  "bot_hold_seconds",
  "bot_max_roi",
  "bot_max_drawdown_from_peak",
  "bot_no_new_high_seconds_at_exit",
  "exit_reason",
  "exit_phase",
  "tp1_done",
  "tp1_roi",
  "first_buy_time",
  "bot_entry_time",
  "bot_exit_time",
  "bot_entry_price",
  "bot_exit_price",
  "pre_5m_return",
  "entry_range_position",
  "distance_to_20m_high",
  "pre_5m_volume_spike",
  "entry_label",
  "moved_after_wallet_buy",
  "kline_file",
];

writeFileSync(
  tradesCsvPath,
  `${headers.join(",")}\n${results
    .map((row) => headers.map((header) => csvEscape(row[header])).join(","))
    .join("\n")}\n`,
);

const allSummary = summarize(results);
const liveSummary = summarize(results.filter((row) => row.live_enabled === true));
const simulationSummary = summarize(results.filter((row) => row.entry_layer));
const walletTrades = results.filter((row) => row.wallet_roi !== null);
const walletWinners = walletTrades.filter((row) => row.wallet_roi > 0).length;
const walletAvgRoi =
  walletTrades.reduce((sum, row) => sum + row.wallet_roi, 0) / Math.max(1, walletTrades.length);

function summaryTable(title, summary) {
  return `### ${title}

| 指标 | 数值 |
|---|---:|
| 样本数 | ${summary.samples} |
| 触发交易 | ${summary.trades} |
| 跳过 | ${summary.skipped} |
| 胜/负 | ${summary.winners} / ${summary.losers} |
| 胜率 | ${fmtPct(summary.winRate)} |
| 平均 ROI | ${fmtPct(summary.avgRoi)} |
| 中位 ROI | ${fmtPct(summary.medianRoi)} |
| ROI 合计 | ${fmtNum(summary.totalRoi, 4)} |
| 中位持仓 | ${fmtNum(summary.medianHold, 1)} 秒 |
`;
}

function groupedTable(title, rows, key) {
  const lines = [`### ${title}`, "", "| 分组 | 样本 | 交易 | 胜率 | 平均 ROI | 中位持仓 |", "|---|---:|---:|---:|---:|---:|"];
  for (const [name, items] of groupBy(rows, key)) {
    const summary = summarize(items);
    lines.push(
      `| ${name || "(空)"} | ${summary.samples} | ${summary.trades} | ${fmtPct(summary.winRate)} | ${fmtPct(summary.avgRoi)} | ${fmtNum(summary.medianHold, 1)}s |`,
    );
  }
  return `${lines.join("\n")}\n`;
}

const skipLines = groupBy(results, "skip_reason")
  .filter(([reason]) => reason)
  .map(([reason, items]) => `| ${reason} | ${items.length} |`)
  .join("\n");

const exitLines = groupBy(results.filter((row) => row.bot_action === "TRADE"), "exit_reason")
  .map(([reason, items]) => {
    const summary = summarize(items);
    return `| ${reason} | ${summary.trades} | ${fmtPct(summary.winRate)} | ${fmtPct(summary.avgRoi)} | ${fmtNum(summary.medianHold, 1)}s |`;
  })
  .join("\n");

const topTrades = [...results]
  .filter((row) => row.bot_action === "TRADE")
  .sort((a, b) => b.bot_roi - a.bot_roi)
  .slice(0, 10)
  .map(
    (row) =>
      `| ${row.mint} | ${row.entry_layer} | ${fmtPct(row.bot_roi)} | ${fmtPct(row.wallet_roi)} | ${row.exit_reason} | ${row.bot_hold_seconds}s |`,
  )
  .join("\n");

const worstTrades = [...results]
  .filter((row) => row.bot_action === "TRADE")
  .sort((a, b) => a.bot_roi - b.bot_roi)
  .slice(0, 10)
  .map(
    (row) =>
      `| ${row.mint} | ${row.entry_layer} | ${fmtPct(row.bot_roi)} | ${fmtPct(row.wallet_roi)} | ${row.exit_reason} | ${row.bot_hold_seconds}s |`,
  )
  .join("\n");

const report = `# 秒级机器人策略模拟结果

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
${tradesCsvPath}
~~~

钱包原始表现：

| 指标 | 数值 |
|---|---:|
| 样本数 | ${walletTrades.length} |
| 胜率 | ${fmtPct(walletWinners / Math.max(1, walletTrades.length))} |
| 平均 ROI | ${fmtPct(walletAvgRoi)} |

${summaryTable("全部 A-F 分层模拟", simulationSummary)}

${summaryTable("实盘优先 A-B 模拟", liveSummary)}

${groupedTable("按入场层统计", results.filter((row) => row.entry_layer), "entry_layer")}

### 跳过原因

| 原因 | 数量 |
|---|---:|
${skipLines || "| 无 | 0 |"}

### 退出原因

| 退出原因 | 交易 | 胜率 | 平均 ROI | 中位持仓 |
|---|---:|---:|---:|---:|
${exitLines}

### 最好 10 笔

| mint | 层 | 机器人 ROI | 钱包 ROI | 退出原因 | 持仓 |
|---|---|---:|---:|---|---:|
${topTrades}

### 最差 10 笔

| mint | 层 | 机器人 ROI | 钱包 ROI | 退出原因 | 持仓 |
|---|---|---:|---:|---|---:|
${worstTrades}

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
`;

writeFileSync(reportPath, report);

console.log(`Wrote ${tradesCsvPath}`);
console.log(`Wrote ${reportPath}`);
console.log(JSON.stringify({ allSummary, liveSummary, simulationSummary }, null, 2));
