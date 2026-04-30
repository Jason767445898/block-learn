import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

const wallet = "55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr";
const root = resolve(import.meta.dirname, "..");
const walletDir = join(root, "data", "strategy_research", "wallets", wallet);
const positionsPath = join(walletDir, "selected_positions.csv");
const klinesDir = join(walletDir, "klines");
const outputPath = join(walletDir, "reports", "kline_trade_visualization.html");

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
      } else if (char === '"') {
        quoted = false;
      } else {
        field += char;
      }
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

  const [headers, ...body] = rows.filter((items) => items.some(Boolean));
  return body.map((items) =>
    Object.fromEntries(headers.map((header, index) => [header, items[index] ?? ""])),
  );
}

function toNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function findKlineFile(row) {
  if (row.kline_file) {
    const absolute = resolve(root, row.kline_file);
    try {
      readFileSync(absolute);
      return absolute;
    } catch {
      // Fall through to filename lookup.
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
      amount: Number(item.amount),
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

function estimatePriceAt(candles, isoTime, fallback) {
  const target = Date.parse(isoTime);
  if (!Number.isFinite(target) || candles.length === 0) return fallback ?? null;

  let nearest = candles[0];
  let bestDistance = Math.abs(nearest.time - target);
  for (const candle of candles) {
    const distance = Math.abs(candle.time - target);
    if (distance < bestDistance) {
      nearest = candle;
      bestDistance = distance;
    }
  }
  return nearest.close;
}

const positions = parseCsv(readFileSync(positionsPath, "utf8"));
const charts = positions
  .map((row) => {
    const klineFile = findKlineFile(row);
    if (!klineFile) return null;

    const candles = loadCandles(klineFile);
    if (candles.length === 0) return null;

    const buyPrice = estimatePriceAt(candles, row.first_buy_time, toNumber(row.average_buy_price_sol));
    const sellPrice = row.last_sell_time
      ? estimatePriceAt(candles, row.last_sell_time, null)
      : null;

    return {
      wallet,
      mint: row.mint,
      group: row.sample_group,
      pnl: toNumber(row.realized_pnl_sol),
      roi: toNumber(row.realized_roi),
      holdingSeconds: toNumber(row.holding_seconds),
      firstBuyTx: row.first_buy_tx,
      firstBuyTime: row.first_buy_time,
      lastSellTime: row.last_sell_time,
      klineFile: basename(klineFile),
      markers: [
        {
          side: "buy",
          time: Date.parse(row.first_buy_time),
          price: buyPrice,
          label: "First buy",
        },
        row.last_sell_time
          ? {
              side: "sell",
              time: Date.parse(row.last_sell_time),
              price: sellPrice,
              label: "Last sell",
            }
          : null,
      ].filter(Boolean),
      candles,
    };
  })
  .filter(Boolean)
  .sort((a, b) => (b.pnl ?? 0) - (a.pnl ?? 0));

const summary = {
  wallet,
  generatedAt: new Date().toISOString(),
  charts: charts.length,
  totalPnl: charts.reduce((sum, item) => sum + (item.pnl ?? 0), 0),
  winners: charts.filter((item) => (item.pnl ?? 0) > 0).length,
  losers: charts.filter((item) => (item.pnl ?? 0) < 0).length,
};

function htmlEscape(text) {
  return String(text)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

const payload = JSON.stringify({ summary, charts });
const html = `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>K线交易标记 - ${wallet}</title>
  <style>
    :root {
      color-scheme: light;
      --bg: #f7f8fa;
      --panel: #ffffff;
      --ink: #15171a;
      --muted: #667085;
      --line: #d9dee7;
      --buy: #138a5b;
      --sell: #c24135;
      --accent: #2457d6;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: var(--bg);
      color: var(--ink);
      font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    header {
      border-bottom: 1px solid var(--line);
      background: var(--panel);
      padding: 18px 24px 14px;
    }
    h1 { margin: 0 0 8px; font-size: 22px; font-weight: 700; letter-spacing: 0; }
    .wallet { color: var(--muted); font-size: 13px; overflow-wrap: anywhere; }
    main { display: grid; grid-template-columns: 340px minmax(0, 1fr); gap: 16px; padding: 16px; }
    .sidebar, .stage, .table-wrap {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
    }
    .sidebar { min-height: calc(100vh - 120px); overflow: hidden; }
    .controls { display: grid; gap: 10px; padding: 14px; border-bottom: 1px solid var(--line); }
    input, select {
      width: 100%;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 9px 10px;
      background: #fff;
      color: var(--ink);
      font-size: 14px;
    }
    .stats { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
    .stat { border: 1px solid var(--line); border-radius: 6px; padding: 9px; }
    .stat b { display: block; font-size: 17px; }
    .stat span { color: var(--muted); font-size: 12px; }
    .list { height: calc(100vh - 302px); overflow: auto; }
    .token {
      display: block;
      width: 100%;
      border: 0;
      border-bottom: 1px solid var(--line);
      background: transparent;
      padding: 11px 14px;
      text-align: left;
      cursor: pointer;
    }
    .token.active { background: #eef4ff; box-shadow: inset 3px 0 0 var(--accent); }
    .token .top { display: flex; justify-content: space-between; gap: 10px; align-items: baseline; }
    .mint { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .profit { color: var(--buy); }
    .loss { color: var(--sell); }
    .meta { color: var(--muted); font-size: 12px; margin-top: 4px; }
    .stage { padding: 14px; min-width: 0; }
    .chart-head { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 12px; align-items: end; margin-bottom: 10px; }
    .chart-title { min-width: 0; }
    .chart-title h2 { margin: 0 0 4px; font-size: 18px; overflow-wrap: anywhere; }
    .chart-title div, .note { color: var(--muted); font-size: 12px; }
    .badge-row { display: flex; flex-wrap: wrap; gap: 8px; }
    .badge { border: 1px solid var(--line); border-radius: 999px; padding: 5px 8px; font-size: 12px; background: #fff; }
    .canvas-wrap { position: relative; height: min(66vh, 680px); min-height: 420px; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
    canvas { display: block; width: 100%; height: 100%; background: #fff; }
    .tooltip {
      position: absolute;
      pointer-events: none;
      display: none;
      max-width: 280px;
      padding: 8px 9px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: rgba(255,255,255,.96);
      box-shadow: 0 10px 24px rgba(15, 23, 42, .12);
      font-size: 12px;
      z-index: 2;
    }
    .table-wrap { margin-top: 12px; overflow: auto; }
    table { width: 100%; border-collapse: collapse; font-size: 13px; }
    th, td { padding: 9px 10px; border-bottom: 1px solid var(--line); text-align: right; white-space: nowrap; }
    th:first-child, td:first-child { text-align: left; }
    th { color: var(--muted); font-weight: 600; background: #fbfcfe; }
    @media (max-width: 860px) {
      main { grid-template-columns: 1fr; padding: 10px; }
      .sidebar { min-height: 0; }
      .list { height: 240px; }
      .canvas-wrap { height: 460px; min-height: 360px; }
    }
  </style>
</head>
<body>
  <header>
    <h1>K线交易标记</h1>
    <div class="wallet">${htmlEscape(wallet)}</div>
  </header>
  <main>
    <aside class="sidebar">
      <div class="controls">
        <div class="stats">
          <div class="stat"><b id="chartCount">0</b><span>有K线样本</span></div>
          <div class="stat"><b id="totalPnl">0</b><span>合计PnL SOL</span></div>
          <div class="stat"><b id="winnerCount">0</b><span>盈利样本</span></div>
          <div class="stat"><b id="loserCount">0</b><span>亏损样本</span></div>
        </div>
        <input id="search" placeholder="搜索 mint">
        <select id="groupFilter">
          <option value="all">全部样本</option>
          <option value="profit">盈利样本</option>
          <option value="loss">亏损样本</option>
        </select>
        <select id="sortMode">
          <option value="pnl_desc">PnL 从高到低</option>
          <option value="pnl_asc">PnL 从低到高</option>
          <option value="time_asc">买入时间从早到晚</option>
          <option value="time_desc">买入时间从晚到早</option>
        </select>
      </div>
      <div id="tokenList" class="list"></div>
    </aside>
    <section class="stage">
      <div class="chart-head">
        <div class="chart-title">
          <h2 id="mintTitle"></h2>
          <div id="chartSubtitle"></div>
        </div>
        <div class="badge-row" id="badges"></div>
      </div>
      <div class="canvas-wrap" id="canvasWrap">
        <canvas id="chart"></canvas>
        <div id="tooltip" class="tooltip"></div>
      </div>
      <p class="note">标记基于当前数据集中可用的 position 聚合字段：绿色为首次买入，红色为最后卖出；多次加仓/分批卖出只有次数汇总，没有逐笔时间点。</p>
      <div class="table-wrap">
        <table>
          <thead><tr><th>事件</th><th>时间</th><th>价格</th><th>说明</th></tr></thead>
          <tbody id="markerRows"></tbody>
        </table>
      </div>
    </section>
  </main>
  <script>
    const DATA = ${payload};
    let charts = DATA.charts;
    let selected = charts[0];

    const count = document.getElementById("chartCount");
    const totalPnl = document.getElementById("totalPnl");
    const winnerCount = document.getElementById("winnerCount");
    const loserCount = document.getElementById("loserCount");
    const list = document.getElementById("tokenList");
    const search = document.getElementById("search");
    const groupFilter = document.getElementById("groupFilter");
    const sortMode = document.getElementById("sortMode");
    const canvas = document.getElementById("chart");
    const wrap = document.getElementById("canvasWrap");
    const tip = document.getElementById("tooltip");
    const ctx = canvas.getContext("2d");

    function fmtNumber(value, digits = 4) {
      return Number.isFinite(value) ? value.toFixed(digits) : "-";
    }
    function fmtPrice(value) {
      if (!Number.isFinite(value)) return "-";
      return value >= 0.0001 ? value.toFixed(8) : value.toExponential(4);
    }
    function fmtTime(value) {
      return new Date(value).toLocaleString("zh-CN", { hour12: false });
    }
    function shortMint(mint) {
      return mint.length > 18 ? mint.slice(0, 8) + "..." + mint.slice(-6) : mint;
    }
    function filteredCharts() {
      const term = search.value.trim().toLowerCase();
      let rows = charts.filter((item) => {
        const groupMatch = groupFilter.value === "all" || item.group === groupFilter.value;
        return groupMatch && item.mint.toLowerCase().includes(term);
      });
      rows.sort((a, b) => {
        if (sortMode.value === "pnl_asc") return (a.pnl ?? 0) - (b.pnl ?? 0);
        if (sortMode.value === "time_asc") return Date.parse(a.firstBuyTime) - Date.parse(b.firstBuyTime);
        if (sortMode.value === "time_desc") return Date.parse(b.firstBuyTime) - Date.parse(a.firstBuyTime);
        return (b.pnl ?? 0) - (a.pnl ?? 0);
      });
      return rows;
    }
    function renderList() {
      const rows = filteredCharts();
      list.innerHTML = rows.map((item, index) => {
        const cls = (item.pnl ?? 0) >= 0 ? "profit" : "loss";
        const active = selected && selected.mint === item.mint ? " active" : "";
        return '<button class="token' + active + '" data-index="' + index + '">' +
          '<div class="top"><span class="mint">' + shortMint(item.mint) + '</span><b class="' + cls + '">' + fmtNumber(item.pnl, 4) + '</b></div>' +
          '<div class="meta">ROI ' + fmtNumber((item.roi ?? 0) * 100, 1) + '% · 持仓 ' + fmtNumber((item.holdingSeconds ?? 0), 0) + 's</div>' +
          '</button>';
      }).join("");
      [...list.querySelectorAll(".token")].forEach((button) => {
        button.addEventListener("click", () => {
          selected = rows[Number(button.dataset.index)];
          render();
        });
      });
    }
    function renderMeta() {
      count.textContent = DATA.summary.charts;
      totalPnl.textContent = fmtNumber(DATA.summary.totalPnl, 4);
      winnerCount.textContent = DATA.summary.winners;
      loserCount.textContent = DATA.summary.losers;
      document.getElementById("mintTitle").textContent = selected.mint;
      document.getElementById("chartSubtitle").textContent = selected.klineFile + " · " + selected.candles.length + " 根1m K线";
      document.getElementById("badges").innerHTML = [
        ["样本", selected.group],
        ["PnL", fmtNumber(selected.pnl, 6) + " SOL"],
        ["ROI", fmtNumber((selected.roi ?? 0) * 100, 2) + "%"],
        ["持仓", fmtNumber(selected.holdingSeconds, 0) + "s"],
      ].map(([label, value]) => '<span class="badge">' + label + ': ' + value + '</span>').join("");
      document.getElementById("markerRows").innerHTML = selected.markers.map((marker) => (
        '<tr><td>' + (marker.side === "buy" ? "买入" : "卖出") + '</td><td>' + fmtTime(marker.time) + '</td><td>' + fmtPrice(marker.price) + '</td><td>' + marker.label + '</td></tr>'
      )).join("");
    }
    function chartScales(item) {
      const padLeft = 70, padRight = 22, padTop = 26, padBottom = 72;
      const w = canvas.clientWidth, h = canvas.clientHeight;
      const allTimes = item.candles.map((c) => c.time).concat(item.markers.map((m) => m.time));
      const minTime = Math.min(...allTimes), maxTime = Math.max(...allTimes);
      const minPrice = Math.min(...item.candles.map((c) => c.low), ...item.markers.map((m) => m.price).filter(Number.isFinite));
      const maxPrice = Math.max(...item.candles.map((c) => c.high), ...item.markers.map((m) => m.price).filter(Number.isFinite));
      const pricePad = (maxPrice - minPrice) * 0.12 || maxPrice * 0.05 || 1;
      const minY = minPrice - pricePad, maxY = maxPrice + pricePad;
      const x = (time) => padLeft + ((time - minTime) / Math.max(1, maxTime - minTime)) * (w - padLeft - padRight);
      const y = (price) => padTop + (1 - (price - minY) / Math.max(1e-18, maxY - minY)) * (h - padTop - padBottom);
      return { w, h, padLeft, padRight, padTop, padBottom, minTime, maxTime, minY, maxY, x, y };
    }
    function resizeCanvas() {
      const dpr = window.devicePixelRatio || 1;
      const rect = wrap.getBoundingClientRect();
      canvas.width = Math.floor(rect.width * dpr);
      canvas.height = Math.floor(rect.height * dpr);
      canvas.style.width = rect.width + "px";
      canvas.style.height = rect.height + "px";
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    function drawChart() {
      resizeCanvas();
      const s = chartScales(selected);
      ctx.clearRect(0, 0, s.w, s.h);
      ctx.fillStyle = "#fff";
      ctx.fillRect(0, 0, s.w, s.h);
      ctx.strokeStyle = "#e8edf4";
      ctx.lineWidth = 1;
      ctx.font = "12px ui-sans-serif, system-ui";
      ctx.fillStyle = "#667085";
      for (let i = 0; i <= 5; i += 1) {
        const y = s.padTop + i * (s.h - s.padTop - s.padBottom) / 5;
        const price = s.maxY - i * (s.maxY - s.minY) / 5;
        ctx.beginPath();
        ctx.moveTo(s.padLeft, y);
        ctx.lineTo(s.w - s.padRight, y);
        ctx.stroke();
        ctx.fillText(fmtPrice(price), 10, y + 4);
      }
      const candleWidth = Math.max(3, Math.min(11, (s.w - s.padLeft - s.padRight) / selected.candles.length * 0.58));
      for (const candle of selected.candles) {
        const x = s.x(candle.time);
        const up = candle.close >= candle.open;
        const color = up ? "#138a5b" : "#c24135";
        ctx.strokeStyle = color;
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.moveTo(x, s.y(candle.high));
        ctx.lineTo(x, s.y(candle.low));
        ctx.stroke();
        const top = s.y(Math.max(candle.open, candle.close));
        const bottom = s.y(Math.min(candle.open, candle.close));
        ctx.fillRect(x - candleWidth / 2, top, candleWidth, Math.max(1, bottom - top));
      }
      for (const marker of selected.markers) {
        if (!Number.isFinite(marker.price)) continue;
        const x = s.x(marker.time), y = s.y(marker.price);
        const isBuy = marker.side === "buy";
        ctx.fillStyle = isBuy ? "#138a5b" : "#c24135";
        ctx.strokeStyle = "#fff";
        ctx.lineWidth = 2;
        ctx.beginPath();
        if (isBuy) {
          ctx.moveTo(x, y - 12); ctx.lineTo(x - 9, y + 8); ctx.lineTo(x + 9, y + 8);
        } else {
          ctx.moveTo(x, y + 12); ctx.lineTo(x - 9, y - 8); ctx.lineTo(x + 9, y - 8);
        }
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = "#15171a";
        ctx.fillText(isBuy ? "BUY" : "SELL", x + 10, y + (isBuy ? 6 : -8));
      }
      ctx.fillStyle = "#667085";
      for (let i = 0; i <= 4; i += 1) {
        const time = s.minTime + i * (s.maxTime - s.minTime) / 4;
        ctx.fillText(fmtTime(time).slice(5), s.x(time) - 38, s.h - 28);
      }
    }
    function nearestPoint(clientX, clientY) {
      const rect = canvas.getBoundingClientRect();
      const px = clientX - rect.left;
      const py = clientY - rect.top;
      const s = chartScales(selected);
      const points = selected.markers
        .filter((m) => Number.isFinite(m.price))
        .map((m) => ({ marker: m, x: s.x(m.time), y: s.y(m.price) }));
      return points.find((p) => Math.hypot(p.x - px, p.y - py) < 18);
    }
    canvas.addEventListener("mousemove", (event) => {
      const point = nearestPoint(event.clientX, event.clientY);
      if (!point) {
        tip.style.display = "none";
        return;
      }
      tip.style.display = "block";
      tip.style.left = Math.min(point.x + 12, wrap.clientWidth - 292) + "px";
      tip.style.top = Math.max(8, point.y - 42) + "px";
      tip.innerHTML = '<b>' + (point.marker.side === "buy" ? "首次买入" : "最后卖出") + '</b><br>' +
        fmtTime(point.marker.time) + '<br>价格 ' + fmtPrice(point.marker.price);
    });
    canvas.addEventListener("mouseleave", () => { tip.style.display = "none"; });
    function render() {
      if (!selected) return;
      renderList();
      renderMeta();
      drawChart();
    }
    [search, groupFilter, sortMode].forEach((control) => {
      control.addEventListener("input", () => {
        const rows = filteredCharts();
        selected = rows.find((row) => row.mint === selected?.mint) ?? rows[0] ?? charts[0];
        render();
      });
    });
    window.addEventListener("resize", drawChart);
    render();
  </script>
</body>
</html>
`;

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, html);

console.log(`Wrote ${outputPath}`);
console.log(`Charts: ${summary.charts}`);
console.log(`Total PnL SOL: ${summary.totalPnl.toFixed(6)}`);
