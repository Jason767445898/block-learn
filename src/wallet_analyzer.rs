use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use std::{
    collections::HashMap, env, error::Error, fs, process::Command, time::Duration as StdDuration,
};

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const NATIVE_SOL_MINT: &str = "11111111111111111111111111111111";
const WRAPPED_SOL_MINT: &str = "So11111111111111111111111111111111111111112";

#[derive(Debug, Clone, PartialEq)]
enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
struct PumpTrade {
    tx_hash: String,
    wallet: String,
    timestamp: DateTime<Utc>,
    mint: String,
    side: Side,
    sol_amount: f64,
    token_amount: f64,
    price_sol: f64,
    token_created_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
struct PositionSummary {
    mint: String,
    first_buy_tx: String,
    first_buy_wallet: String,
    first_buy_time: DateTime<Utc>,
    first_sell_time: Option<DateTime<Utc>>,
    holding_seconds: Option<i64>,
    total_buy_sol: f64,
    total_sell_sol: f64,
    average_buy_price_sol: f64,
    realized_pnl_sol: f64,
    realized_roi: f64,
    buy_count: usize,
    sell_count: usize,
    entry_after_launch_seconds: Option<i64>,
    exit_pattern: ExitPattern,
}

#[derive(Debug, PartialEq)]
enum ExitPattern {
    FullExit,
    PartialExit,
    StillHolding,
    NoSell,
}

#[derive(Debug)]
struct StrategyStats {
    wallet: String,
    trades: usize,
    tokens: usize,
    buy_count: usize,
    sell_count: usize,
    total_buy_sol: f64,
    total_sell_sol: f64,
    realized_pnl_sol: f64,
    win_rate: f64,
    average_buy_sol: f64,
    median_holding_seconds: Option<i64>,
    median_entry_after_launch_seconds: Option<i64>,
    partial_sell_rate: f64,
    max_profit_share: f64,
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BehaviorKind {
    AddLiquidity,
    RemoveLiquidity,
    CreateToken,
    Swap,
    Transfer,
    CloseAccount,
    OtherDefi,
    Unknown,
}

impl BehaviorKind {
    fn label(&self) -> &'static str {
        match self {
            BehaviorKind::AddLiquidity => "add_liquidity",
            BehaviorKind::RemoveLiquidity => "remove_liquidity",
            BehaviorKind::CreateToken => "create_token",
            BehaviorKind::Swap => "swap",
            BehaviorKind::Transfer => "transfer",
            BehaviorKind::CloseAccount => "close_account",
            BehaviorKind::OtherDefi => "other_defi",
            BehaviorKind::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
struct BehaviorEvent {
    timestamp: DateTime<Utc>,
    tx_hash: String,
    protocol: String,
    kind: BehaviorKind,
    mint_a: Option<String>,
    mint_b: Option<String>,
    amount_a: Option<f64>,
    amount_b: Option<f64>,
    confidence: f64,
    evidence: String,
}

#[derive(Debug)]
struct BehaviorStats {
    wallet: String,
    events: usize,
    add_liquidity: usize,
    remove_liquidity: usize,
    create_token: usize,
    swaps: usize,
    transfers: usize,
    close_accounts: usize,
    protocols: Vec<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct KlineCandle {
    mint: String,
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Debug)]
struct KlineTradeContext {
    tx_hash: String,
    mint: String,
    side: Side,
    timestamp: DateTime<Utc>,
    trade_price: f64,
    range_position: Option<f64>,
    momentum: Option<f64>,
    volume_spike: Option<f64>,
    max_runup_after: Option<f64>,
    max_drawdown_after: Option<f64>,
    exit_efficiency: Option<f64>,
    tags: Vec<String>,
}

#[derive(Debug)]
struct KlineStrategyStats {
    wallet: String,
    matched_trades: usize,
    unmatched_trades: usize,
    avg_range_position: Option<f64>,
    avg_momentum: Option<f64>,
    avg_volume_spike: Option<f64>,
    avg_max_runup_after: Option<f64>,
    avg_max_drawdown_after: Option<f64>,
    avg_exit_efficiency: Option<f64>,
    tags: Vec<String>,
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        print_help();
        return Ok(());
    }

    match args[1].as_str() {
        "analyze" => analyze_command(&args[2..]).await,
        "behaviors" => behaviors_command(&args[2..]).await,
        "analyze-kline" => analyze_kline_command(&args[2..]).await,
        _ => {
            print_help();
            Ok(())
        }
    }
}

async fn analyze_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("missing wallet address: cargo run -- analyze <wallet>".into());
    }

    let wallet = args[0].clone();
    let source = get_arg(args, "--source").unwrap_or_else(|| "bitquery".to_string());
    let days = get_arg(args, "--days")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30);
    let limit = get_arg(args, "--limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(500);

    let trades = match source.as_str() {
        "bitquery" => fetch_bitquery_trades(&wallet, days, limit).await?,
        "csv" => {
            let path = get_arg(args, "--file").ok_or("--file is required for --source csv")?;
            read_csv_trades(&wallet, &path)?
        }
        "sample" => sample_trades(&wallet),
        other => return Err(format!("unknown source: {other}").into()),
    };

    if trades.is_empty() {
        println!("No pump.fun trades found for {wallet}.");
        return Ok(());
    }

    let positions = aggregate_positions(&trades);
    let stats = build_stats(&wallet, &trades, &positions);
    print_report(&stats, &positions);
    Ok(())
}

async fn behaviors_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("missing wallet address: cargo run -- behaviors <wallet>".into());
    }

    let wallet = args[0].clone();
    let source = get_arg(args, "--source").unwrap_or_else(|| "helius".to_string());
    let limit = get_arg(args, "--limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100);

    let events = match source.as_str() {
        "helius" => fetch_helius_behavior_events(&wallet, limit).await?,
        "json" => {
            let path = get_arg(args, "--file").ok_or("--file is required for --source json")?;
            read_helius_json_behavior_events(&wallet, &path)?
        }
        "sample" => sample_behavior_events(&wallet),
        other => return Err(format!("unknown behavior source: {other}").into()),
    };

    if events.is_empty() {
        println!("No non-trade behavior events found for {wallet}.");
        return Ok(());
    }

    let stats = build_behavior_stats(&wallet, &events);
    print_behavior_report(&stats, &events);
    Ok(())
}

async fn analyze_kline_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("missing wallet address: cargo run -- analyze-kline <wallet>".into());
    }

    let wallet = args[0].clone();
    let trades_source = get_arg(args, "--trades-source").unwrap_or_else(|| "bitquery".to_string());
    let kline_source = get_arg(args, "--kline-source").unwrap_or_else(|| "csv".to_string());
    let days = get_arg(args, "--days")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(7);
    let limit = get_arg(args, "--limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100);
    let lookback = get_arg(args, "--lookback")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10);
    let lookahead = get_arg(args, "--lookahead")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10);
    let resolution = get_arg(args, "--resolution").unwrap_or_else(|| "1m".to_string());

    let trades = match trades_source.as_str() {
        "bitquery" => fetch_bitquery_trades(&wallet, days, limit).await?,
        "csv" => {
            let path =
                get_arg(args, "--trades-file").ok_or("--trades-file is required for csv trades")?;
            read_csv_trades(&wallet, &path)?
        }
        "sample" => sample_trades(&wallet),
        other => return Err(format!("unknown trades source: {other}").into()),
    };

    if trades.is_empty() {
        println!("No trades found for {wallet}.");
        return Ok(());
    }

    let use_candle_price = kline_source == "gmgn";
    let candles = match kline_source.as_str() {
        "csv" => {
            let path =
                get_arg(args, "--kline-file").ok_or("--kline-file is required for csv kline")?;
            read_csv_klines(&path)?
        }
        "gmgn" => fetch_gmgn_klines_for_trades(&trades, &resolution, lookback, lookahead)?,
        "sample" => sample_klines_for_trades(&trades),
        other => return Err(format!("unknown kline source: {other}").into()),
    };

    if candles.is_empty() {
        println!("No kline candles found.");
        return Ok(());
    }

    let contexts = build_kline_contexts(&trades, &candles, lookback, lookahead, use_candle_price);
    let stats = build_kline_strategy_stats(&wallet, &trades, &contexts);
    print_kline_report(&stats, &contexts);
    Ok(())
}

fn get_arg(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

async fn fetch_bitquery_trades(
    wallet: &str,
    days: i64,
    limit: usize,
) -> Result<Vec<PumpTrade>, Box<dyn Error>> {
    let token = env::var("BITQUERY_TOKEN")
        .map_err(|_| "BITQUERY_TOKEN is required for --source bitquery")?;
    let since = Utc::now() - Duration::days(days);
    let query = r#"
query WalletPumpTrades($wallet: String!, $since: DateTime!, $limit: Int!) {
  Solana(dataset: realtime) {
    DEXTrades(
      limit: { count: $limit }
      orderBy: { descending: Block_Time }
      where: {
        Transaction: { Result: { Success: true }, Signer: { is: $wallet } }
        Block: { Time: { since: $since } }
        Trade: { Dex: { ProgramAddress: { is: "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P" } } }
      }
    ) {
      Block { Time }
      Transaction { Signature Signer }
      Trade {
        Buy {
          Amount
          Account { Address }
          Currency { MintAddress Symbol Name }
        }
        Sell {
          Amount
          Account { Address }
          Currency { MintAddress Symbol Name }
        }
      }
    }
  }
}
"#;

    let body = json!({
        "query": query,
        "variables": {
            "wallet": wallet,
            "since": since.to_rfc3339(),
            "limit": limit,
        }
    });

    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(60))
        .user_agent("blockchain-wallet-strategy-mvp/0.1")
        .build()?;
    let mut last_error = String::new();
    let mut response_value = None;

    for attempt in 1..=3 {
        let result = client
            .post("https://streaming.bitquery.io/eap")
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await;

        match result {
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.json::<Value>().await {
                    Ok(response) => {
                        response_value = Some(response);
                        break;
                    }
                    Err(error) => {
                        last_error = format!("failed to decode Bitquery response body: {error}");
                    }
                },
                Err(error) => {
                    last_error = format!("Bitquery returned an HTTP error: {error}");
                }
            },
            Err(error) => {
                last_error = format!("failed to call Bitquery: {error}");
            }
        }

        if attempt < 3 {
            tokio::time::sleep(StdDuration::from_secs(attempt * 2)).await;
        }
    }

    let response = response_value.ok_or_else(|| {
        format!(
            "{last_error}. Try a smaller query, for example: cargo run -- analyze {wallet} --source bitquery --days 3 --limit 25"
        )
    })?;

    if let Some(errors) = response.get("errors") {
        return Err(format!("Bitquery returned errors: {errors}").into());
    }

    let rows = response
        .pointer("/data/Solana/DEXTrades")
        .and_then(Value::as_array)
        .ok_or("Bitquery response did not contain data.Solana.DEXTrades")?;

    let trades = rows
        .iter()
        .filter_map(|row| parse_bitquery_trade(wallet, row))
        .collect();
    Ok(trades)
}

fn parse_bitquery_trade(wallet: &str, row: &Value) -> Option<PumpTrade> {
    let timestamp = parse_time(row.pointer("/Block/Time")?.as_str()?)?;
    let tx_hash = row
        .pointer("/Transaction/Signature")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    let buy_mint = row.pointer("/Trade/Buy/Currency/MintAddress")?.as_str()?;
    let sell_mint = row.pointer("/Trade/Sell/Currency/MintAddress")?.as_str()?;
    let buy_amount = as_f64(row.pointer("/Trade/Buy/Amount")?)?;
    let sell_amount = as_f64(row.pointer("/Trade/Sell/Amount")?)?;
    let buy_account = row
        .pointer("/Trade/Buy/Account/Address")
        .and_then(Value::as_str)
        .unwrap_or("");
    let sell_account = row
        .pointer("/Trade/Sell/Account/Address")
        .and_then(Value::as_str)
        .unwrap_or("");

    let wallet_bought = buy_account == wallet || is_sol_mint(sell_mint);
    let wallet_sold = sell_account == wallet || is_sol_mint(buy_mint);

    let (side, mint, sol_amount, token_amount) = if wallet_bought && is_sol_mint(sell_mint) {
        (Side::Buy, buy_mint, sell_amount, buy_amount)
    } else if wallet_sold && is_sol_mint(buy_mint) {
        (Side::Sell, sell_mint, buy_amount, sell_amount)
    } else {
        return None;
    };

    Some(PumpTrade {
        tx_hash,
        wallet: wallet.to_string(),
        timestamp,
        mint: mint.to_string(),
        side,
        sol_amount,
        token_amount,
        price_sol: safe_div(sol_amount, token_amount),
        token_created_at: None,
    })
}

async fn fetch_helius_behavior_events(
    wallet: &str,
    limit: usize,
) -> Result<Vec<BehaviorEvent>, Box<dyn Error>> {
    let api_key =
        env::var("HELIUS_API_KEY").map_err(|_| "HELIUS_API_KEY is required for --source helius")?;
    let url = format!("https://api-mainnet.helius-rpc.com/v0/addresses/{wallet}/transactions");

    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(45))
        .user_agent("blockchain-wallet-behavior-mvp/0.1")
        .build()?;
    let request_limit = limit.min(100).to_string();
    let mut last_error = String::new();

    for attempt in 1..=3 {
        let result = client
            .get(&url)
            .query(&[
                ("api-key", api_key.as_str()),
                ("limit", request_limit.as_str()),
            ])
            .send()
            .await;

        match result {
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.json::<Value>().await {
                    Ok(response) => return parse_helius_behavior_events(wallet, &response),
                    Err(error) => {
                        last_error = format!("failed to decode Helius response body: {error}");
                    }
                },
                Err(error) => {
                    last_error = format!("Helius returned an HTTP error: {error}");
                }
            },
            Err(error) => {
                last_error = format!("failed to call Helius: {error}");
            }
        }

        if attempt < 3 {
            tokio::time::sleep(StdDuration::from_secs(attempt * 2)).await;
        }
    }

    Err(format!(
        "{last_error}. Try a smaller --limit, for example: cargo run -- behaviors {wallet} --source helius --limit 20"
    )
    .into())
}

fn read_helius_json_behavior_events(
    wallet: &str,
    path: &str,
) -> Result<Vec<BehaviorEvent>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&content)?;
    parse_helius_behavior_events(wallet, &json)
}

fn parse_helius_behavior_events(
    wallet: &str,
    response: &Value,
) -> Result<Vec<BehaviorEvent>, Box<dyn Error>> {
    let rows = response
        .as_array()
        .ok_or("Helius response must be a JSON array of enhanced transactions")?;
    let mut events = Vec::new();

    for row in rows {
        if let Some(event) = classify_helius_transaction(wallet, row) {
            if event.kind != BehaviorKind::Unknown {
                events.push(event);
            }
        }
    }

    events.sort_by_key(|event| event.timestamp);
    Ok(events)
}

fn classify_helius_transaction(wallet: &str, tx: &Value) -> Option<BehaviorEvent> {
    let tx_hash = tx
        .get("signature")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let timestamp = parse_helius_timestamp(tx)?;
    let source = tx
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");
    let tx_type = tx.get("type").and_then(Value::as_str).unwrap_or("UNKNOWN");
    let description = tx.get("description").and_then(Value::as_str).unwrap_or("");
    let protocol = normalize_protocol(source, tx);
    let searchable = format!(
        "{} {} {} {}",
        source,
        tx_type,
        description,
        tx.get("instructions")
            .map(Value::to_string)
            .unwrap_or_default()
    )
    .to_ascii_lowercase();
    let (mints, amounts) = extract_token_context(wallet, tx);

    let known_defi = is_known_defi_protocol(&protocol) || is_known_defi_text(&searchable);
    let (kind, confidence, evidence) = if contains_any(
        &searchable,
        &[
            "add_liquidity",
            "add liquidity",
            "increase_liquidity",
            "increase liquidity",
            "deposit_liquidity",
            "deposit liquidity",
            "addliquidity",
            "liquidity added",
        ],
    ) || (known_defi
        && contains_any(&searchable, &["deposit", "increase"])
        && contains_any(&searchable, &["liquidity", "position"]))
    {
        (
            BehaviorKind::AddLiquidity,
            0.90,
            "liquidity add/increase/deposit signal".to_string(),
        )
    } else if contains_any(
        &searchable,
        &[
            "remove_liquidity",
            "remove liquidity",
            "decrease_liquidity",
            "decrease liquidity",
            "withdraw_liquidity",
            "withdraw liquidity",
            "burn_liquidity",
            "liquidity removed",
        ],
    ) || (known_defi
        && contains_any(&searchable, &["withdraw", "decrease", "remove"])
        && contains_any(&searchable, &["liquidity", "position"]))
    {
        (
            BehaviorKind::RemoveLiquidity,
            0.90,
            "liquidity remove/decrease/withdraw signal".to_string(),
        )
    } else if contains_any(
        &searchable,
        &[
            "create_token",
            "create token",
            "token_mint",
            "initialize mint",
            "create mint",
            "mintto",
        ],
    ) || (protocol == "PUMP_FUN"
        && contains_any(&searchable, &["create", "initialize", "mint"]))
    {
        (
            BehaviorKind::CreateToken,
            0.85,
            "token creation or mint initialization signal".to_string(),
        )
    } else if contains_any(&searchable, &["swap", "buy", "sell"]) && known_defi {
        (
            BehaviorKind::Swap,
            0.70,
            "DEX swap/buy/sell signal".to_string(),
        )
    } else if contains_any(&searchable, &["close_account", "close account"]) {
        (
            BehaviorKind::CloseAccount,
            0.75,
            "token account close signal".to_string(),
        )
    } else if tx.get("tokenTransfers").and_then(Value::as_array).is_some()
        || tx
            .get("nativeTransfers")
            .and_then(Value::as_array)
            .is_some()
    {
        (
            BehaviorKind::Transfer,
            0.55,
            "token/native transfer signal".to_string(),
        )
    } else if known_defi {
        (
            BehaviorKind::OtherDefi,
            0.50,
            "known DeFi protocol interaction".to_string(),
        )
    } else {
        (BehaviorKind::Unknown, 0.0, String::new())
    };

    Some(BehaviorEvent {
        timestamp,
        tx_hash,
        protocol,
        kind,
        mint_a: mints.first().cloned(),
        mint_b: mints.get(1).cloned(),
        amount_a: amounts.first().copied(),
        amount_b: amounts.get(1).copied(),
        confidence,
        evidence,
    })
}

fn read_csv_trades(wallet: &str, path: &str) -> Result<Vec<PumpTrade>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let mut lines = content.lines();
    let header = lines.next().ok_or("CSV is empty")?;
    let columns: Vec<&str> = header.split(',').map(str::trim).collect();
    let index = |name: &str| columns.iter().position(|column| *column == name);

    let mut trades = Vec::new();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let values: Vec<&str> = line.split(',').map(str::trim).collect();
        let get = |name: &str| index(name).and_then(|i| values.get(i).copied());
        let side = match get("side").unwrap_or("").to_ascii_lowercase().as_str() {
            "buy" => Side::Buy,
            "sell" => Side::Sell,
            _ => continue,
        };
        let timestamp = match get("timestamp").and_then(parse_time) {
            Some(value) => value,
            None => continue,
        };
        let token_created_at = get("token_created_at").and_then(parse_time);
        let sol_amount = get("sol_amount")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let token_amount = get("token_amount")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);

        trades.push(PumpTrade {
            tx_hash: get("tx_hash").unwrap_or("csv").to_string(),
            wallet: get("wallet").unwrap_or(wallet).to_string(),
            timestamp,
            mint: get("mint").unwrap_or("unknown").to_string(),
            side,
            sol_amount,
            token_amount,
            price_sol: get("price_sol")
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| safe_div(sol_amount, token_amount)),
            token_created_at,
        });
    }
    Ok(trades)
}

fn read_csv_klines(path: &str) -> Result<Vec<KlineCandle>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let mut lines = content.lines();
    let header = lines.next().ok_or("kline CSV is empty")?;
    let columns: Vec<&str> = header.split(',').map(str::trim).collect();
    let index = |name: &str| columns.iter().position(|column| *column == name);

    let mut candles = Vec::new();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let values: Vec<&str> = line.split(',').map(str::trim).collect();
        let get = |name: &str| index(name).and_then(|i| values.get(i).copied());
        let Some(timestamp) = get("timestamp").and_then(parse_time) else {
            continue;
        };

        candles.push(KlineCandle {
            mint: get("mint").unwrap_or("unknown").to_string(),
            timestamp,
            open: get("open").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            high: get("high").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            low: get("low").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            close: get("close").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            volume: get("volume").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        });
    }

    candles.sort_by_key(|candle| (candle.mint.clone(), candle.timestamp));
    Ok(candles)
}

fn fetch_gmgn_klines_for_trades(
    trades: &[PumpTrade],
    resolution: &str,
    lookback: usize,
    lookahead: usize,
) -> Result<Vec<KlineCandle>, Box<dyn Error>> {
    if env::var("GMGN_API_KEY").is_err() {
        return Err("GMGN_API_KEY is required for --kline-source gmgn".into());
    }

    let interval_seconds = resolution_to_seconds(resolution).ok_or_else(|| {
        format!("unsupported GMGN resolution: {resolution}. Try 1m, 5m, 15m, 1h, 4h, or 1d")
    })?;
    let mut ranges: HashMap<String, (i64, i64)> = HashMap::new();

    for trade in trades {
        let timestamp = trade.timestamp.timestamp();
        let from = timestamp - (lookback as i64 + 3) * interval_seconds;
        let to = timestamp + (lookahead as i64 + 3) * interval_seconds;
        ranges
            .entry(trade.mint.clone())
            .and_modify(|range| {
                range.0 = range.0.min(from);
                range.1 = range.1.max(to);
            })
            .or_insert((from, to));
    }

    let mut candles = Vec::new();
    let mut failed_mints = Vec::new();
    for (mint, (from, to)) in ranges {
        match fetch_gmgn_klines_for_mint(&mint, resolution, from, to) {
            Ok(mut mint_candles) => candles.append(&mut mint_candles),
            Err(error) => {
                eprintln!(
                    "warning: skipped GMGN kline for {}: {error}",
                    short_mint(&mint)
                );
                failed_mints.push(mint);
            }
        }
    }

    candles.sort_by_key(|candle| (candle.mint.clone(), candle.timestamp));
    if candles.is_empty() && !failed_mints.is_empty() {
        return Err(
            "GMGN kline requests all failed. Try again, reduce --limit, check network access to openapi.gmgn.ai, or use --kline-source csv as a fallback."
                .into(),
        );
    }
    Ok(candles)
}

fn fetch_gmgn_klines_for_mint(
    mint: &str,
    resolution: &str,
    from: i64,
    to: i64,
) -> Result<Vec<KlineCandle>, Box<dyn Error>> {
    let mut last_error = String::new();

    for attempt in 1..=3 {
        let output = Command::new("npx")
            .args([
                "-y",
                "gmgn-cli",
                "market",
                "kline",
                "--chain",
                "sol",
                "--address",
                mint,
                "--resolution",
                resolution,
                "--from",
                &from.to_string(),
                "--to",
                &to.to_string(),
                "--raw",
            ])
            .output()
            .map_err(|error| {
                format!(
                    "failed to run gmgn-cli via npx: {error}. Make sure Node.js/npm are installed"
                )
            })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let value: Value = serde_json::from_str(&stdout).map_err(|error| {
                format!(
                    "failed to parse gmgn-cli JSON: {error}. Output: {}",
                    stdout.chars().take(300).collect::<String>()
                )
            })?;
            return Ok(parse_gmgn_kline_value(mint, &value));
        }

        last_error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if attempt < 3 {
            std::thread::sleep(StdDuration::from_secs(attempt * 3));
        }
    }

    Err(format!("gmgn-cli failed after 3 attempts: {last_error}").into())
}

fn parse_gmgn_kline_value(mint: &str, value: &Value) -> Vec<KlineCandle> {
    let rows = value
        .as_array()
        .or_else(|| value.get("list").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/list").and_then(Value::as_array))
        .or_else(|| value.get("data").and_then(Value::as_array));

    let Some(rows) = rows else {
        return Vec::new();
    };

    rows.iter()
        .filter_map(|row| parse_gmgn_kline_row(mint, row))
        .collect()
}

fn parse_gmgn_kline_row(mint: &str, row: &Value) -> Option<KlineCandle> {
    let timestamp = gmgn_row_timestamp(row)?;
    let open = gmgn_number(row, &["open", "o"])?;
    let high = gmgn_number(row, &["high", "h"])?;
    let low = gmgn_number(row, &["low", "l"])?;
    let close = gmgn_number(row, &["close", "c"])?;
    let volume = gmgn_number(row, &["volume", "vol", "v"]).unwrap_or(0.0);

    Some(KlineCandle {
        mint: mint.to_string(),
        timestamp,
        open,
        high,
        low,
        close,
        volume,
    })
}

fn aggregate_positions(trades: &[PumpTrade]) -> Vec<PositionSummary> {
    let mut by_mint: HashMap<String, Vec<&PumpTrade>> = HashMap::new();
    for trade in trades {
        by_mint.entry(trade.mint.clone()).or_default().push(trade);
    }

    let mut positions = Vec::new();
    for (mint, mut rows) in by_mint {
        rows.sort_by_key(|trade| trade.timestamp);
        let buys: Vec<&&PumpTrade> = rows
            .iter()
            .filter(|trade| trade.side == Side::Buy)
            .collect();
        if buys.is_empty() {
            continue;
        }

        let sells: Vec<&&PumpTrade> = rows
            .iter()
            .filter(|trade| trade.side == Side::Sell)
            .collect();
        let total_buy_sol: f64 = buys.iter().map(|trade| trade.sol_amount).sum();
        let total_sell_sol: f64 = sells.iter().map(|trade| trade.sol_amount).sum();
        let bought_tokens: f64 = buys.iter().map(|trade| trade.token_amount).sum();
        let sold_tokens: f64 = sells.iter().map(|trade| trade.token_amount).sum();
        let average_buy_price_sol = safe_div(
            buys.iter().map(|trade| trade.price_sol).sum::<f64>(),
            buys.len() as f64,
        );
        let first_buy = buys[0];
        let first_sell_time = sells.first().map(|trade| trade.timestamp);
        let holding_seconds = first_sell_time.map(|sell_time| {
            sell_time
                .signed_duration_since(first_buy.timestamp)
                .num_seconds()
        });
        let entry_after_launch_seconds = first_buy.token_created_at.map(|created_at| {
            first_buy
                .timestamp
                .signed_duration_since(created_at)
                .num_seconds()
        });
        let exit_pattern = if sells.is_empty() {
            ExitPattern::NoSell
        } else if sold_tokens >= bought_tokens * 0.95 {
            ExitPattern::FullExit
        } else if sold_tokens > 0.0 {
            ExitPattern::PartialExit
        } else {
            ExitPattern::StillHolding
        };
        let realized_pnl_sol = total_sell_sol - total_buy_sol;

        positions.push(PositionSummary {
            mint,
            first_buy_tx: first_buy.tx_hash.clone(),
            first_buy_wallet: first_buy.wallet.clone(),
            first_buy_time: first_buy.timestamp,
            first_sell_time,
            holding_seconds,
            total_buy_sol,
            total_sell_sol,
            average_buy_price_sol,
            realized_pnl_sol,
            realized_roi: safe_div(realized_pnl_sol, total_buy_sol),
            buy_count: buys.len(),
            sell_count: sells.len(),
            entry_after_launch_seconds,
            exit_pattern,
        });
    }

    positions.sort_by_key(|position| position.first_buy_time);
    positions
}

fn build_stats(wallet: &str, trades: &[PumpTrade], positions: &[PositionSummary]) -> StrategyStats {
    let buy_count = trades
        .iter()
        .filter(|trade| trade.side == Side::Buy)
        .count();
    let sell_count = trades
        .iter()
        .filter(|trade| trade.side == Side::Sell)
        .count();
    let total_buy_sol: f64 = trades
        .iter()
        .filter(|trade| trade.side == Side::Buy)
        .map(|trade| trade.sol_amount)
        .sum();
    let total_sell_sol: f64 = trades
        .iter()
        .filter(|trade| trade.side == Side::Sell)
        .map(|trade| trade.sol_amount)
        .sum();
    let wins = positions
        .iter()
        .filter(|position| position.realized_pnl_sol > 0.0)
        .count();
    let profits: Vec<f64> = positions
        .iter()
        .map(|position| position.realized_pnl_sol)
        .filter(|pnl| *pnl > 0.0)
        .collect();
    let total_profit: f64 = profits.iter().sum();
    let max_profit = profits.iter().copied().fold(0.0, f64::max);
    let mut holding_seconds: Vec<i64> = positions
        .iter()
        .filter_map(|position| position.holding_seconds)
        .collect();
    let mut entry_after_launch_seconds: Vec<i64> = positions
        .iter()
        .filter_map(|position| position.entry_after_launch_seconds)
        .collect();
    let partial_sells = positions
        .iter()
        .filter(|position| position.exit_pattern == ExitPattern::PartialExit)
        .count();

    let mut stats = StrategyStats {
        wallet: wallet.to_string(),
        trades: trades.len(),
        tokens: positions.len(),
        buy_count,
        sell_count,
        total_buy_sol,
        total_sell_sol,
        realized_pnl_sol: total_sell_sol - total_buy_sol,
        win_rate: safe_div(wins as f64, positions.len() as f64),
        average_buy_sol: safe_div(total_buy_sol, buy_count as f64),
        median_holding_seconds: median_i64(&mut holding_seconds),
        median_entry_after_launch_seconds: median_i64(&mut entry_after_launch_seconds),
        partial_sell_rate: safe_div(partial_sells as f64, positions.len() as f64),
        max_profit_share: safe_div(max_profit, total_profit),
        tags: Vec::new(),
    };
    stats.tags = classify_strategy(&stats);
    stats
}

fn build_behavior_stats(wallet: &str, events: &[BehaviorEvent]) -> BehaviorStats {
    let mut counts: HashMap<BehaviorKind, usize> = HashMap::new();
    let mut protocols: Vec<String> = Vec::new();

    for event in events {
        *counts.entry(event.kind.clone()).or_insert(0) += 1;
        if event.protocol != "UNKNOWN" && !protocols.contains(&event.protocol) {
            protocols.push(event.protocol.clone());
        }
    }

    protocols.sort();
    let mut stats = BehaviorStats {
        wallet: wallet.to_string(),
        events: events.len(),
        add_liquidity: *counts.get(&BehaviorKind::AddLiquidity).unwrap_or(&0),
        remove_liquidity: *counts.get(&BehaviorKind::RemoveLiquidity).unwrap_or(&0),
        create_token: *counts.get(&BehaviorKind::CreateToken).unwrap_or(&0),
        swaps: *counts.get(&BehaviorKind::Swap).unwrap_or(&0),
        transfers: *counts.get(&BehaviorKind::Transfer).unwrap_or(&0),
        close_accounts: *counts.get(&BehaviorKind::CloseAccount).unwrap_or(&0),
        protocols,
        tags: Vec::new(),
    };
    stats.tags = classify_behavior_tags(&stats);
    stats
}

fn build_kline_contexts(
    trades: &[PumpTrade],
    candles: &[KlineCandle],
    lookback: usize,
    lookahead: usize,
    use_candle_price: bool,
) -> Vec<KlineTradeContext> {
    let mut candles_by_mint: HashMap<String, Vec<&KlineCandle>> = HashMap::new();
    for candle in candles {
        candles_by_mint
            .entry(candle.mint.clone())
            .or_default()
            .push(candle);
    }
    for rows in candles_by_mint.values_mut() {
        rows.sort_by_key(|candle| candle.timestamp);
    }

    let positions = aggregate_positions(trades);
    let mut contexts = Vec::new();

    for trade in trades {
        let Some(rows) = candles_by_mint.get(&trade.mint) else {
            continue;
        };
        let Some(index) = candle_index_for_trade(rows, trade.timestamp) else {
            continue;
        };

        let start = index.saturating_sub(lookback);
        let end = (index + lookahead + 1).min(rows.len());
        let history = &rows[start..=index];
        let future = &rows[index..end];
        let current = rows[index];
        let analysis_price = if use_candle_price {
            current.close
        } else {
            trade.price_sol
        };
        let recent_low = history
            .iter()
            .map(|candle| candle.low)
            .fold(f64::INFINITY, f64::min);
        let recent_high = history.iter().map(|candle| candle.high).fold(0.0, f64::max);
        let range_position = if recent_high > recent_low {
            Some(((analysis_price - recent_low) / (recent_high - recent_low)).clamp(0.0, 1.0))
        } else {
            None
        };
        let first_close = history.first().map(|candle| candle.close).unwrap_or(0.0);
        let momentum = if first_close > 0.0 {
            Some(safe_div(current.close - first_close, first_close))
        } else {
            None
        };
        let candle_body = if current.open > 0.0 {
            Some(safe_div(current.close - current.open, current.open))
        } else {
            None
        };
        let previous_volume_count = history.len().saturating_sub(1);
        let previous_volume_sum: f64 = history
            .iter()
            .take(previous_volume_count)
            .map(|candle| candle.volume)
            .sum();
        let avg_previous_volume = safe_div(previous_volume_sum, previous_volume_count as f64);
        let volume_spike = if avg_previous_volume > 0.0 {
            Some(current.volume / avg_previous_volume)
        } else {
            None
        };
        let (max_runup_after, max_drawdown_after) =
            if trade.side == Side::Buy && analysis_price > 0.0 {
                let future_high = future.iter().map(|candle| candle.high).fold(0.0, f64::max);
                let future_low = future
                    .iter()
                    .map(|candle| candle.low)
                    .fold(f64::INFINITY, f64::min);
                let runup = safe_div(future_high - analysis_price, analysis_price).max(0.0);
                let drawdown = if future_low.is_finite() {
                    safe_div(future_low - analysis_price, analysis_price).min(0.0)
                } else {
                    0.0
                };
                (Some(runup), Some(drawdown))
            } else {
                (None, None)
            };
        let exit_efficiency = if trade.side == Side::Sell {
            exit_efficiency_for_sell(trade, analysis_price, &positions, rows)
        } else {
            None
        };

        let tags = classify_kline_trade_tags(
            &trade.side,
            range_position,
            momentum,
            candle_body,
            volume_spike,
            max_runup_after,
            max_drawdown_after,
            exit_efficiency,
        );

        contexts.push(KlineTradeContext {
            tx_hash: trade.tx_hash.clone(),
            mint: trade.mint.clone(),
            side: trade.side.clone(),
            timestamp: trade.timestamp,
            trade_price: analysis_price,
            range_position,
            momentum,
            volume_spike,
            max_runup_after,
            max_drawdown_after,
            exit_efficiency,
            tags,
        });
    }

    contexts.sort_by_key(|context| context.timestamp);
    contexts
}

fn build_kline_strategy_stats(
    wallet: &str,
    trades: &[PumpTrade],
    contexts: &[KlineTradeContext],
) -> KlineStrategyStats {
    let avg_range_position = average_option(contexts.iter().map(|context| context.range_position));
    let avg_momentum = average_option(contexts.iter().map(|context| context.momentum));
    let avg_volume_spike = average_option(contexts.iter().map(|context| context.volume_spike));
    let avg_max_runup_after =
        average_option(contexts.iter().map(|context| context.max_runup_after));
    let avg_max_drawdown_after =
        average_option(contexts.iter().map(|context| context.max_drawdown_after));
    let avg_exit_efficiency =
        average_option(contexts.iter().map(|context| context.exit_efficiency));
    let mut stats = KlineStrategyStats {
        wallet: wallet.to_string(),
        matched_trades: contexts.len(),
        unmatched_trades: trades.len().saturating_sub(contexts.len()),
        avg_range_position,
        avg_momentum,
        avg_volume_spike,
        avg_max_runup_after,
        avg_max_drawdown_after,
        avg_exit_efficiency,
        tags: Vec::new(),
    };
    stats.tags = classify_kline_strategy_tags(&stats, contexts);
    stats
}

fn classify_behavior_tags(stats: &BehaviorStats) -> Vec<String> {
    let mut tags = Vec::new();
    let lp_events = stats.add_liquidity + stats.remove_liquidity;

    if lp_events > 0 {
        tags.push("有 LP 行为".to_string());
    }
    if stats.add_liquidity > 0 && stats.remove_liquidity > 0 {
        tags.push("流动性进出管理".to_string());
    }
    if stats.create_token > 0 && lp_events > 0 {
        tags.push("疑似项目方/发行相关钱包".to_string());
    } else if stats.create_token > 0 {
        tags.push("有发币/初始化 mint 行为".to_string());
    }
    if stats.swaps > 0 && lp_events > 0 {
        tags.push("交易 + LP 混合钱包".to_string());
    }
    if stats.close_accounts > 0 {
        tags.push("有清理 token account 痕迹".to_string());
    }
    if tags.is_empty() {
        tags.push("未发现明显 LP/项目方行为".to_string());
    }

    tags
}

fn classify_strategy(stats: &StrategyStats) -> Vec<String> {
    let mut tags = Vec::new();
    if stats
        .median_entry_after_launch_seconds
        .is_some_and(|seconds| seconds <= 120)
    {
        tags.push("新盘狙击".to_string());
    }
    if stats
        .median_holding_seconds
        .is_some_and(|seconds| seconds <= 300)
    {
        tags.push("超短线快进快出".to_string());
    }
    if stats.average_buy_sol > 0.0 && stats.average_buy_sol <= 0.2 && stats.tokens >= 20 {
        tags.push("小仓位高频撒网".to_string());
    }
    if stats.partial_sell_rate >= 0.35 {
        tags.push("分批止盈/留仓".to_string());
    }
    if stats.win_rate < 0.35 && stats.max_profit_share >= 0.70 {
        tags.push("低胜率博大倍数".to_string());
    }
    if tags.is_empty() {
        tags.push("样本不足，偏通用短线交易".to_string());
    }
    tags
}

fn classify_kline_trade_tags(
    side: &Side,
    range_position: Option<f64>,
    momentum: Option<f64>,
    candle_body: Option<f64>,
    volume_spike: Option<f64>,
    max_runup_after: Option<f64>,
    max_drawdown_after: Option<f64>,
    exit_efficiency: Option<f64>,
) -> Vec<String> {
    let mut tags = Vec::new();

    if *side == Side::Buy {
        if range_position.is_some_and(|value| value >= 0.80)
            && momentum.is_some_and(|value| value >= 0.20)
        {
            tags.push("高位追涨".to_string());
        }
        if range_position.is_some_and(|value| value <= 0.25)
            && momentum.is_some_and(|value| value <= -0.15)
        {
            tags.push("回撤抄底".to_string());
        }
        if volume_spike.is_some_and(|value| value >= 3.0) {
            tags.push("放量确认".to_string());
        }
        if candle_body.is_some_and(|value| value >= 0.12) {
            tags.push("阳线追入".to_string());
        }
        if max_drawdown_after.is_some_and(|value| value <= -0.30) {
            tags.push("买后承压大".to_string());
        }
        if max_runup_after.is_some_and(|value| value >= 0.50) {
            tags.push("买后弹性高".to_string());
        }
    } else {
        if exit_efficiency.is_some_and(|value| value >= 0.80) {
            tags.push("接近高位卖出".to_string());
        }
        if exit_efficiency.is_some_and(|value| value <= 0.45) {
            tags.push("出场效率偏低".to_string());
        }
        if range_position.is_some_and(|value| value <= 0.30) {
            tags.push("低位止损/割肉".to_string());
        }
    }

    if tags.is_empty() {
        tags.push("结构信号一般".to_string());
    }
    tags
}

fn classify_kline_strategy_tags(
    stats: &KlineStrategyStats,
    contexts: &[KlineTradeContext],
) -> Vec<String> {
    let mut tags = Vec::new();
    let buys: Vec<&KlineTradeContext> = contexts
        .iter()
        .filter(|context| context.side == Side::Buy)
        .collect();
    let sells: Vec<&KlineTradeContext> = contexts
        .iter()
        .filter(|context| context.side == Side::Sell)
        .collect();
    let high_chase_rate = safe_div(
        buys.iter()
            .filter(|context| context.tags.iter().any(|tag| tag == "高位追涨"))
            .count() as f64,
        buys.len() as f64,
    );
    let dip_buy_rate = safe_div(
        buys.iter()
            .filter(|context| context.tags.iter().any(|tag| tag == "回撤抄底"))
            .count() as f64,
        buys.len() as f64,
    );
    let volume_confirm_rate = safe_div(
        buys.iter()
            .filter(|context| context.tags.iter().any(|tag| tag == "放量确认"))
            .count() as f64,
        buys.len() as f64,
    );
    let low_exit_rate = safe_div(
        sells
            .iter()
            .filter(|context| context.tags.iter().any(|tag| tag == "低位止损/割肉"))
            .count() as f64,
        sells.len() as f64,
    );

    if high_chase_rate >= 0.45 {
        tags.push("动量追涨型".to_string());
    }
    if dip_buy_rate >= 0.35 {
        tags.push("回撤/反弹交易型".to_string());
    }
    if volume_confirm_rate >= 0.45 {
        tags.push("量能确认入场".to_string());
    }
    if low_exit_rate >= 0.35 {
        tags.push("快速止损/割肉明显".to_string());
    }
    if stats.avg_exit_efficiency.is_some_and(|value| value >= 0.75) {
        tags.push("出场效率较高".to_string());
    } else if stats.avg_exit_efficiency.is_some_and(|value| value <= 0.50) {
        tags.push("出场效率偏低".to_string());
    }
    if tags.is_empty() {
        tags.push("K线结构信号不足".to_string());
    }

    tags
}

fn print_report(stats: &StrategyStats, positions: &[PositionSummary]) {
    println!("pump.fun wallet strategy MVP");
    println!("program: {PUMP_PROGRAM_ID}");
    println!("wallet: {}", stats.wallet);
    println!();
    println!("summary");
    println!(
        "- trades: {} (buys {}, sells {})",
        stats.trades, stats.buy_count, stats.sell_count
    );
    println!("- tokens traded: {}", stats.tokens);
    println!("- total buy: {:.4} SOL", stats.total_buy_sol);
    println!("- total sell: {:.4} SOL", stats.total_sell_sol);
    println!("- realized pnl: {:.4} SOL", stats.realized_pnl_sol);
    println!("- win rate: {:.1}%", stats.win_rate * 100.0);
    println!("- average buy size: {:.4} SOL", stats.average_buy_sol);
    println!(
        "- median holding time: {}",
        format_seconds(stats.median_holding_seconds)
    );
    println!(
        "- median entry after launch: {}",
        format_seconds(stats.median_entry_after_launch_seconds)
    );
    println!(
        "- max profit concentration: {:.1}%",
        stats.max_profit_share * 100.0
    );
    println!();
    println!("strategy tags: {}", stats.tags.join(", "));
    println!();
    println!("{}", narrative(stats));
    println!();
    println!("top positions by pnl");
    for position in top_positions(positions, 8) {
        println!(
            "- {} pnl {:.4} SOL roi {:.1}% buy {:.4} sell {:.4} avg_price {:.10} holding {} buys {} sells {} first_tx {} wallet {} first_sell {} exit {:?}",
            short_mint(&position.mint),
            position.realized_pnl_sol,
            position.realized_roi * 100.0,
            position.total_buy_sol,
            position.total_sell_sol,
            position.average_buy_price_sol,
            format_seconds(position.holding_seconds),
            position.buy_count,
            position.sell_count,
            short_id(&position.first_buy_tx),
            short_id(&position.first_buy_wallet),
            position
                .first_sell_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "-".to_string()),
            position.exit_pattern
        );
    }
}

fn print_behavior_report(stats: &BehaviorStats, events: &[BehaviorEvent]) {
    println!("wallet behavior MVP");
    println!("wallet: {}", stats.wallet);
    println!();
    println!("summary");
    println!("- classified events: {}", stats.events);
    println!("- add liquidity: {}", stats.add_liquidity);
    println!("- remove liquidity: {}", stats.remove_liquidity);
    println!("- create token: {}", stats.create_token);
    println!("- swaps: {}", stats.swaps);
    println!("- transfers: {}", stats.transfers);
    println!("- close accounts: {}", stats.close_accounts);
    println!(
        "- protocols touched: {}",
        if stats.protocols.is_empty() {
            "n/a".to_string()
        } else {
            stats.protocols.join(", ")
        }
    );
    println!();
    println!("behavior tags: {}", stats.tags.join(", "));
    println!();
    println!("{}", behavior_narrative(stats));
    println!();
    println!("important events");
    for event in important_behavior_events(events, 12) {
        println!(
            "- {} {} protocol {} tx {} mints {} amounts {} confidence {:.0}% evidence {}",
            event.timestamp.to_rfc3339(),
            event.kind.label(),
            event.protocol,
            short_id(&event.tx_hash),
            format_pair(event.mint_a.as_deref(), event.mint_b.as_deref()),
            format_amount_pair(event.amount_a, event.amount_b),
            event.confidence * 100.0,
            event.evidence
        );
    }
}

fn print_kline_report(stats: &KlineStrategyStats, contexts: &[KlineTradeContext]) {
    println!("wallet kline strategy MVP");
    println!("wallet: {}", stats.wallet);
    println!();
    println!("summary");
    println!("- matched trades: {}", stats.matched_trades);
    println!("- unmatched trades: {}", stats.unmatched_trades);
    println!(
        "- avg range position: {}",
        format_percent_option(stats.avg_range_position)
    );
    println!(
        "- avg pre-trade momentum: {}",
        format_percent_option(stats.avg_momentum)
    );
    println!(
        "- avg volume spike: {}",
        format_ratio_option(stats.avg_volume_spike)
    );
    println!(
        "- avg max runup after trade: {}",
        format_percent_option(stats.avg_max_runup_after)
    );
    println!(
        "- avg max drawdown after trade: {}",
        format_percent_option(stats.avg_max_drawdown_after)
    );
    println!(
        "- avg exit efficiency: {}",
        format_percent_option(stats.avg_exit_efficiency)
    );
    println!();
    println!("kline strategy tags: {}", stats.tags.join(", "));
    println!();
    println!("{}", kline_narrative(stats));
    println!();
    println!("trade kline contexts");
    for context in contexts.iter().take(16) {
        println!(
            "- {} {:?} {} price {:.10} range {} momentum {} volume {} runup {} drawdown {} exit_eff {} tx {} tags {}",
            context.timestamp.to_rfc3339(),
            context.side,
            short_mint(&context.mint),
            context.trade_price,
            format_percent_option(context.range_position),
            format_percent_option(context.momentum),
            format_ratio_option(context.volume_spike),
            format_percent_option(context.max_runup_after),
            format_percent_option(context.max_drawdown_after),
            format_percent_option(context.exit_efficiency),
            short_id(&context.tx_hash),
            context.tags.join("|")
        );
    }
}

fn kline_narrative(stats: &KlineStrategyStats) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "该钱包从 K 线结构看更接近「{}」。",
        stats.tags.join(" + ")
    ));
    if let Some(position) = stats.avg_range_position {
        lines.push(format!(
            "平均成交位置处于近期区间的 {:.1}%，{}。",
            position * 100.0,
            if position >= 0.75 {
                "偏高，说明更常在上涨或高位结构里行动"
            } else if position <= 0.35 {
                "偏低，说明更常在回撤或低位结构里行动"
            } else {
                "居中，入场/出场位置并不极端"
            }
        ));
    }
    if let Some(spike) = stats.avg_volume_spike {
        if spike >= 2.0 {
            lines.push(format!(
                "平均成交量放大约 {spike:.2}x，说明它的成交点经常伴随量能变化。"
            ));
        }
    }
    if let Some(efficiency) = stats.avg_exit_efficiency {
        lines.push(format!(
            "平均出场效率约 {:.1}%，{}。",
            efficiency * 100.0,
            if efficiency >= 0.75 {
                "卖点相对接近持仓区间高位"
            } else {
                "卖点距离持仓区间高位有明显折损"
            }
        ));
    }
    lines.join("\n")
}

fn behavior_narrative(stats: &BehaviorStats) -> String {
    let mut lines = Vec::new();
    let lp_events = stats.add_liquidity + stats.remove_liquidity;

    if lp_events > 0 {
        lines.push(format!(
            "该钱包出现 {} 次 LP 相关行为，其中加流动性 {} 次、撤流动性 {} 次。",
            lp_events, stats.add_liquidity, stats.remove_liquidity
        ));
    } else {
        lines.push("当前样本没有识别到明确的 LP 加/撤流动性行为。".to_string());
    }

    if stats.create_token > 0 && lp_events > 0 {
        lines.push(
            "它同时有发币/初始化 mint 与 LP 行为，需要优先按项目方、做市或迁移后流动性管理钱包来审视。"
                .to_string(),
        );
    } else if stats.create_token > 0 {
        lines.push("它有发币/初始化 mint 痕迹，但当前样本没有同时捕捉到 LP 行为。".to_string());
    } else if lp_events > 0 && stats.swaps > 0 {
        lines.push("它既交易又管理流动性，更像交易 + LP 混合策略，而不是纯跟单对象。".to_string());
    }

    lines.join("\n")
}

fn important_behavior_events(events: &[BehaviorEvent], limit: usize) -> Vec<&BehaviorEvent> {
    let mut rows: Vec<&BehaviorEvent> = events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                BehaviorKind::AddLiquidity
                    | BehaviorKind::RemoveLiquidity
                    | BehaviorKind::CreateToken
                    | BehaviorKind::CloseAccount
                    | BehaviorKind::OtherDefi
            )
        })
        .collect();
    rows.sort_by_key(|event| event.timestamp);

    if rows.len() < limit {
        let existing: Vec<String> = rows.iter().map(|event| event.tx_hash.clone()).collect();
        rows.extend(
            events
                .iter()
                .filter(|event| !existing.contains(&event.tx_hash))
                .take(limit - rows.len()),
        );
    }

    rows.sort_by_key(|event| event.timestamp);
    rows.into_iter().take(limit).collect()
}

fn narrative(stats: &StrategyStats) -> String {
    let mut lines = Vec::new();
    lines.push(format!("该钱包更接近「{}」策略。", stats.tags.join(" + ")));
    if stats.average_buy_sol > 0.0 {
        lines.push(format!(
            "它的平均买入仓位约 {:.4} SOL，说明仓位{}。",
            stats.average_buy_sol,
            if stats.average_buy_sol <= 0.2 {
                "偏小，更像分散试错"
            } else if stats.average_buy_sol <= 1.0 {
                "中等，可能会选择性加注"
            } else {
                "偏大，风险暴露较高"
            }
        ));
    }
    if let Some(seconds) = stats.median_holding_seconds {
        lines.push(format!(
            "中位持仓时间为 {}，出场节奏{}。",
            format_seconds(Some(seconds)),
            if seconds <= 300 {
                "很快，偏 scalping"
            } else if seconds <= 3600 {
                "偏短线"
            } else {
                "更像波段或被动持有"
            }
        ));
    }
    if stats.max_profit_share >= 0.7 {
        lines.push("盈利集中度很高，整体结果可能主要依赖少数大幅盈利 token。".to_string());
    }
    if stats.win_rate < 0.4 {
        lines.push("胜率偏低，需要结合最大回撤和未卖出仓位继续判断风险。".to_string());
    }
    lines.join("\n")
}

fn top_positions(positions: &[PositionSummary], limit: usize) -> Vec<&PositionSummary> {
    let mut rows: Vec<&PositionSummary> = positions.iter().collect();
    rows.sort_by(|a, b| b.realized_pnl_sol.total_cmp(&a.realized_pnl_sol));
    rows.into_iter().take(limit).collect()
}

fn sample_trades(wallet: &str) -> Vec<PumpTrade> {
    let now = Utc::now();
    let mut rows = Vec::new();
    for i in 0..24 {
        let created_at = now - Duration::minutes(240 - i * 7);
        let buy_time = created_at + Duration::seconds(45 + (i % 4) * 35);
        let mint = format!("SampleMint{i:02}");
        let buy_sol = 0.08 + (i % 3) as f64 * 0.03;
        rows.push(PumpTrade {
            tx_hash: format!("sample-buy-{i}"),
            wallet: wallet.to_string(),
            timestamp: buy_time,
            mint: mint.clone(),
            side: Side::Buy,
            sol_amount: buy_sol,
            token_amount: 1_000_000.0,
            price_sol: buy_sol / 1_000_000.0,
            token_created_at: Some(created_at),
        });
        if i % 5 != 0 {
            let multiplier = if i == 17 {
                12.0
            } else if i % 4 == 0 {
                2.0
            } else {
                0.55
            };
            rows.push(PumpTrade {
                tx_hash: format!("sample-sell-{i}"),
                wallet: wallet.to_string(),
                timestamp: buy_time + Duration::seconds(90 + (i % 6) * 40),
                mint,
                side: Side::Sell,
                sol_amount: buy_sol * multiplier,
                token_amount: if i % 4 == 0 { 650_000.0 } else { 1_000_000.0 },
                price_sol: (buy_sol * multiplier) / 1_000_000.0,
                token_created_at: Some(created_at),
            });
        }
    }
    rows
}

fn sample_behavior_events(wallet: &str) -> Vec<BehaviorEvent> {
    let now = Utc::now();
    vec![
        BehaviorEvent {
            timestamp: now - Duration::minutes(42),
            tx_hash: "sample-create-token".to_string(),
            protocol: "PUMP_FUN".to_string(),
            kind: BehaviorKind::CreateToken,
            mint_a: Some("SampleProjectMint111".to_string()),
            mint_b: None,
            amount_a: None,
            amount_b: None,
            confidence: 0.90,
            evidence: format!("wallet {wallet} initialized a token mint"),
        },
        BehaviorEvent {
            timestamp: now - Duration::minutes(36),
            tx_hash: "sample-add-lp".to_string(),
            protocol: "PUMPSWAP".to_string(),
            kind: BehaviorKind::AddLiquidity,
            mint_a: Some("SampleProjectMint111".to_string()),
            mint_b: Some(WRAPPED_SOL_MINT.to_string()),
            amount_a: Some(12_000_000.0),
            amount_b: Some(3.2),
            confidence: 0.92,
            evidence: "liquidity add/increase/deposit signal".to_string(),
        },
        BehaviorEvent {
            timestamp: now - Duration::minutes(22),
            tx_hash: "sample-swap".to_string(),
            protocol: "RAYDIUM".to_string(),
            kind: BehaviorKind::Swap,
            mint_a: Some(WRAPPED_SOL_MINT.to_string()),
            mint_b: Some("AnotherMint222".to_string()),
            amount_a: Some(0.8),
            amount_b: Some(820_000.0),
            confidence: 0.70,
            evidence: "DEX swap/buy/sell signal".to_string(),
        },
        BehaviorEvent {
            timestamp: now - Duration::minutes(9),
            tx_hash: "sample-remove-lp".to_string(),
            protocol: "PUMPSWAP".to_string(),
            kind: BehaviorKind::RemoveLiquidity,
            mint_a: Some("SampleProjectMint111".to_string()),
            mint_b: Some(WRAPPED_SOL_MINT.to_string()),
            amount_a: Some(6_500_000.0),
            amount_b: Some(1.9),
            confidence: 0.92,
            evidence: "liquidity remove/decrease/withdraw signal".to_string(),
        },
    ]
}

fn sample_klines_for_trades(trades: &[PumpTrade]) -> Vec<KlineCandle> {
    let mut candles = Vec::new();
    let mut seen = Vec::new();

    for trade in trades {
        if seen.iter().any(|mint| mint == &trade.mint) {
            continue;
        }
        seen.push(trade.mint.clone());
        let start = trade.timestamp - Duration::minutes(12);
        let mut price = (trade.price_sol * 0.65).max(0.000000001);

        for i in 0..36 {
            let drift = if i < 12 {
                1.08
            } else if i < 22 {
                1.03
            } else {
                0.96
            };
            let open = price;
            let close = (open * drift).max(0.000000001);
            let high = open.max(close) * (1.04 + (i % 3) as f64 * 0.01);
            let low = open.min(close) * (0.96 - (i % 2) as f64 * 0.01);
            let volume = if (10..=14).contains(&i) {
                1200.0 + i as f64 * 90.0
            } else {
                260.0 + i as f64 * 13.0
            };

            candles.push(KlineCandle {
                mint: trade.mint.clone(),
                timestamp: start + Duration::minutes(i),
                open,
                high,
                low,
                close,
                volume,
            });
            price = close;
        }
    }

    candles
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .ok()
}

fn parse_helius_timestamp(tx: &Value) -> Option<DateTime<Utc>> {
    if let Some(seconds) = tx.get("timestamp").and_then(Value::as_i64) {
        return DateTime::<Utc>::from_timestamp(seconds, 0);
    }

    tx.get("timestamp")
        .and_then(Value::as_str)
        .and_then(parse_time)
}

fn as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn normalize_protocol(source: &str, tx: &Value) -> String {
    let source = source.to_ascii_uppercase();
    if source != "UNKNOWN" {
        return source;
    }

    let raw = tx.to_string().to_ascii_lowercase();
    if raw.contains("pumpswap") {
        "PUMPSWAP".to_string()
    } else if raw.contains(&PUMP_PROGRAM_ID.to_ascii_lowercase()) || raw.contains("pump") {
        "PUMP_FUN".to_string()
    } else if raw.contains("raydium") {
        "RAYDIUM".to_string()
    } else if raw.contains("orca") || raw.contains("whirlpool") {
        "ORCA".to_string()
    } else if raw.contains("meteora") || raw.contains("dlmm") {
        "METEORA".to_string()
    } else {
        "UNKNOWN".to_string()
    }
}

fn is_known_defi_protocol(protocol: &str) -> bool {
    matches!(
        protocol,
        "PUMP_FUN" | "PUMPSWAP" | "RAYDIUM" | "ORCA" | "METEORA" | "JUPITER"
    )
}

fn is_known_defi_text(text: &str) -> bool {
    contains_any(
        text,
        &[
            "pump.fun",
            "pumpfun",
            "pumpswap",
            "raydium",
            "orca",
            "whirlpool",
            "meteora",
            "dlmm",
            "jupiter",
        ],
    )
}

fn extract_token_context(wallet: &str, tx: &Value) -> (Vec<String>, Vec<f64>) {
    let mut mints = Vec::new();
    let mut amounts = Vec::new();

    if let Some(transfers) = tx.get("tokenTransfers").and_then(Value::as_array) {
        for transfer in transfers {
            let related_to_wallet = transfer
                .get("fromUserAccount")
                .and_then(Value::as_str)
                .is_some_and(|account| account == wallet)
                || transfer
                    .get("toUserAccount")
                    .and_then(Value::as_str)
                    .is_some_and(|account| account == wallet)
                || transfer
                    .get("userAccount")
                    .and_then(Value::as_str)
                    .is_some_and(|account| account == wallet);

            if !related_to_wallet && !mints.is_empty() {
                continue;
            }

            if let Some(mint) = transfer.get("mint").and_then(Value::as_str) {
                if !mints.iter().any(|known| known == mint) {
                    mints.push(mint.to_string());
                }
            }

            if let Some(amount) = transfer
                .get("tokenAmount")
                .and_then(as_f64)
                .or_else(|| transfer.get("amount").and_then(as_f64))
                .or_else(|| {
                    transfer
                        .pointer("/rawTokenAmount/tokenAmount")
                        .and_then(as_f64)
                })
            {
                amounts.push(amount);
            }
        }
    }

    if let Some(native_transfers) = tx.get("nativeTransfers").and_then(Value::as_array) {
        for transfer in native_transfers {
            let related_to_wallet = transfer
                .get("fromUserAccount")
                .and_then(Value::as_str)
                .is_some_and(|account| account == wallet)
                || transfer
                    .get("toUserAccount")
                    .and_then(Value::as_str)
                    .is_some_and(|account| account == wallet);
            if related_to_wallet {
                if !mints.iter().any(|known| known == WRAPPED_SOL_MINT) {
                    mints.push(WRAPPED_SOL_MINT.to_string());
                }
                if let Some(lamports) = transfer.get("amount").and_then(as_f64) {
                    amounts.push(lamports / 1_000_000_000.0);
                }
            }
        }
    }

    (mints, amounts)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn candle_index_for_trade(rows: &[&KlineCandle], timestamp: DateTime<Utc>) -> Option<usize> {
    rows.iter()
        .enumerate()
        .take_while(|(_, candle)| candle.timestamp <= timestamp)
        .last()
        .map(|(index, _)| index)
}

fn resolution_to_seconds(resolution: &str) -> Option<i64> {
    match resolution {
        "1m" | "1" => Some(60),
        "5m" | "5" => Some(300),
        "15m" | "15" => Some(900),
        "30m" | "30" => Some(1800),
        "1h" | "60" => Some(3600),
        "4h" | "240" => Some(14_400),
        "1d" | "1D" | "D" => Some(86_400),
        _ => None,
    }
}

fn gmgn_row_timestamp(row: &Value) -> Option<DateTime<Utc>> {
    let value = row
        .get("time")
        .or_else(|| row.get("timestamp"))
        .or_else(|| row.get("t"))?;

    if let Some(text) = value.as_str() {
        if let Some(time) = parse_time(text) {
            return Some(time);
        }
        if let Ok(number) = text.parse::<i64>() {
            return unix_timestamp(number);
        }
    }

    value.as_i64().and_then(unix_timestamp)
}

fn unix_timestamp(value: i64) -> Option<DateTime<Utc>> {
    let seconds = if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    };
    DateTime::<Utc>::from_timestamp(seconds, 0)
}

fn gmgn_number(row: &Value, names: &[&str]) -> Option<f64> {
    for name in names {
        if let Some(value) = row.get(*name).and_then(as_f64) {
            return Some(value);
        }
    }
    None
}

fn exit_efficiency_for_sell(
    trade: &PumpTrade,
    analysis_price: f64,
    positions: &[PositionSummary],
    rows: &[&KlineCandle],
) -> Option<f64> {
    let position = positions.iter().find(|position| {
        position.mint == trade.mint
            && position
                .first_sell_time
                .is_some_and(|sell_time| sell_time == trade.timestamp)
    })?;

    let high_during_hold = rows
        .iter()
        .filter(|candle| {
            candle.timestamp >= position.first_buy_time && candle.timestamp <= trade.timestamp
        })
        .map(|candle| candle.high)
        .fold(0.0, f64::max);

    if high_during_hold > 0.0 {
        Some((analysis_price / high_during_hold).clamp(0.0, 1.0))
    } else {
        None
    }
}

fn average_option(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;

    for value in values.flatten() {
        if value.is_finite() {
            total += value;
            count += 1;
        }
    }

    if count == 0 {
        None
    } else {
        Some(total / count as f64)
    }
}

fn is_sol_mint(mint: &str) -> bool {
    mint == NATIVE_SOL_MINT || mint == WRAPPED_SOL_MINT
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

fn median_i64(values: &mut [i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

fn format_seconds(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return "n/a".to_string();
    };
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{:.1}m", seconds as f64 / 60.0)
    } else {
        format!("{:.1}h", seconds as f64 / 3600.0)
    }
}

fn short_mint(mint: &str) -> String {
    if mint.len() <= 12 {
        mint.to_string()
    } else {
        format!("{}...{}", &mint[..6], &mint[mint.len() - 4..])
    }
}

fn short_id(value: &str) -> String {
    if value.len() <= 14 {
        value.to_string()
    } else {
        format!("{}...{}", &value[..6], &value[value.len() - 4..])
    }
}

fn format_pair(left: Option<&str>, right: Option<&str>) -> String {
    match (left, right) {
        (Some(left), Some(right)) => format!("{} / {}", short_mint(left), short_mint(right)),
        (Some(left), None) => short_mint(left),
        _ => "n/a".to_string(),
    }
}

fn format_amount_pair(left: Option<f64>, right: Option<f64>) -> String {
    match (left, right) {
        (Some(left), Some(right)) => format!("{left:.4} / {right:.4}"),
        (Some(left), None) => format!("{left:.4}"),
        _ => "n/a".to_string(),
    }
}

fn format_percent_option(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.1}%", value * 100.0))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_ratio_option(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.2}x"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn print_help() {
    println!(
        r#"pump.fun wallet strategy MVP

Usage:
  cargo run -- analyze <wallet> [--source bitquery|csv|sample] [--days 30] [--limit 500]
  cargo run -- analyze <wallet> --source csv --file trades.csv
  cargo run -- behaviors <wallet> [--source helius|json|sample] [--limit 100]
  cargo run -- behaviors <wallet> --source json --file helius-transactions.json
  cargo run -- analyze-kline <wallet> --trades-source bitquery --kline-source csv --kline-file klines.csv
  cargo run -- analyze-kline <wallet> --trades-source bitquery --kline-source gmgn --resolution 1m
  cargo run -- analyze-kline <wallet> --trades-source sample --kline-source sample

Bitquery:
  export BITQUERY_TOKEN=<your token>

Helius:
  export HELIUS_API_KEY=<your key>

GMGN:
  export GMGN_API_KEY=<your key>

CSV columns:
  timestamp,tx_hash,wallet,mint,side,sol_amount,token_amount,price_sol,token_created_at

Kline CSV columns:
  mint,timestamp,open,high,low,close,volume
"#
    );
}
