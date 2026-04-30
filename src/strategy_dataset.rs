use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration as StdDuration,
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
struct DatasetTrade {
    tx_hash: String,
    wallet: String,
    timestamp: DateTime<Utc>,
    mint: String,
    side: Side,
    sol_amount: f64,
    token_amount: f64,
    price_sol: f64,
}

#[derive(Debug, Clone)]
struct DatasetPosition {
    wallet: String,
    mint: String,
    first_buy_tx: String,
    first_buy_wallet: String,
    first_buy_time: DateTime<Utc>,
    last_sell_time: Option<DateTime<Utc>>,
    buy_count: usize,
    sell_count: usize,
    total_buy_sol: f64,
    total_sell_sol: f64,
    average_buy_price_sol: f64,
    realized_pnl_sol: f64,
    realized_roi: f64,
    holding_seconds: Option<i64>,
    is_closed: bool,
}

#[derive(Debug, Clone)]
struct SelectedPosition {
    position: DatasetPosition,
    sample_group: String,
    kline_file: Option<String>,
    exclude_reason: String,
}

#[derive(Debug, Serialize)]
struct SampleKlineCandle {
    mint: String,
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    resolution: String,
    source: String,
}

pub async fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return Ok(());
    }

    let wallet = args[0].clone();
    let trades_source = get_arg(args, "--trades-source").unwrap_or_else(|| "bitquery".to_string());
    let kline_source = get_arg(args, "--kline-source").unwrap_or_else(|| "gmgn".to_string());
    let days = parse_arg(args, "--days", 30_i64);
    let limit = parse_arg(args, "--limit", 2_000_usize);
    let profit_samples = parse_arg(args, "--profit-samples", 50_usize);
    let loss_samples = parse_arg(args, "--loss-samples", 50_usize);
    let resolution = get_arg(args, "--resolution").unwrap_or_else(|| "1m".to_string());
    let pre_minutes = parse_arg(args, "--pre-minutes", 20_i64);
    let post_minutes = parse_arg(args, "--post-minutes", 20_i64);
    let out_dir = get_arg(args, "--out").unwrap_or_else(|| {
        format!(
            "data/strategy_research/wallets/{}",
            sanitize_path_component(&wallet)
        )
    });

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
        println!("No pump.fun trades found for {wallet}.");
        return Ok(());
    }

    let mut positions = aggregate_positions(&wallet, &trades);
    positions.sort_by_key(|position| position.first_buy_time);
    let mut selected = select_positions(&positions, profit_samples, loss_samples);

    let out_dir = PathBuf::from(out_dir);
    let kline_dir = out_dir.join("klines");
    let feature_dir = out_dir.join("features");
    let report_dir = out_dir.join("reports");
    fs::create_dir_all(&kline_dir)?;
    fs::create_dir_all(&feature_dir)?;
    fs::create_dir_all(&report_dir)?;

    if kline_source == "gmgn" {
        env::var("GMGN_API_KEY").map_err(|_| "GMGN_API_KEY is required for --kline-source gmgn")?;
    }

    for selected_position in &mut selected {
        let from = selected_position.position.first_buy_time - Duration::minutes(pre_minutes);
        let last_sell_time = selected_position
            .position
            .last_sell_time
            .unwrap_or(selected_position.position.first_buy_time);
        let to = last_sell_time + Duration::minutes(post_minutes);
        let file_name = kline_file_name(&selected_position.position.mint, &resolution, from, to);
        let file_path = kline_dir.join(&file_name);

        if file_path.exists() {
            selected_position.kline_file = Some(path_to_string(&file_path));
            continue;
        }

        match kline_source.as_str() {
            "gmgn" => {
                match fetch_and_save_gmgn_kline(
                    &selected_position.position.mint,
                    &resolution,
                    from.timestamp(),
                    to.timestamp(),
                    &file_path,
                ) {
                    Ok(()) => {
                        selected_position.kline_file = Some(path_to_string(&file_path));
                    }
                    Err(error) => {
                        selected_position.exclude_reason =
                            format!("kline_download_failed: {error}");
                        eprintln!(
                            "warning: skipped kline for {}: {error}",
                            short_mint(&selected_position.position.mint)
                        );
                    }
                }
            }
            "sample" => {
                let candles = sample_klines_for_position(
                    &selected_position.position.mint,
                    &resolution,
                    from,
                    to,
                );
                fs::write(&file_path, serde_json::to_string_pretty(&candles)?)?;
                selected_position.kline_file = Some(path_to_string(&file_path));
            }
            other => return Err(format!("unknown kline source: {other}").into()),
        }
    }

    write_positions_csv(&out_dir.join("positions.csv"), &positions)?;
    write_selected_positions_csv(&out_dir.join("selected_positions.csv"), &selected)?;
    write_dataset_summary(
        &report_dir.join("dataset_summary.md"),
        &wallet,
        &trades_source,
        &kline_source,
        &resolution,
        &positions,
        &selected,
    )?;

    let profit_count = selected
        .iter()
        .filter(|position| position.sample_group == "profit")
        .count();
    let loss_count = selected
        .iter()
        .filter(|position| position.sample_group == "loss")
        .count();
    let kline_count = selected
        .iter()
        .filter(|position| position.kline_file.is_some())
        .count();
    let skipped_count = selected.len().saturating_sub(kline_count);

    println!("strategy replication dataset MVP");
    println!("wallet: {wallet}");
    println!("out: {}", out_dir.display());
    println!("trades: {}", trades.len());
    println!("positions: {}", positions.len());
    println!("selected: {} profit / {} loss", profit_count, loss_count);
    println!("klines: {} saved / {} skipped", kline_count, skipped_count);
    println!();
    println!("written");
    println!("- {}", out_dir.join("positions.csv").display());
    println!("- {}", out_dir.join("selected_positions.csv").display());
    println!("- {}", kline_dir.display());
    println!("- {}", report_dir.join("dataset_summary.md").display());

    Ok(())
}

fn get_arg(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn parse_arg<T>(args: &[String], name: &str, default: T) -> T
where
    T: std::str::FromStr,
{
    get_arg(args, name)
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}

async fn fetch_bitquery_trades(
    wallet: &str,
    days: i64,
    limit: usize,
) -> Result<Vec<DatasetTrade>, Box<dyn Error>> {
    let token = env::var("BITQUERY_TOKEN")
        .map_err(|_| "BITQUERY_TOKEN is required for --trades-source bitquery")?;
    let since = Utc::now() - Duration::days(days);
    let query = format!(
        r#"
query WalletPumpTrades($wallet: String!, $since: DateTime!, $limit: Int!) {{
  Solana(dataset: realtime) {{
    DEXTrades(
      limit: {{ count: $limit }}
      orderBy: {{ descending: Block_Time }}
      where: {{
        Transaction: {{ Result: {{ Success: true }}, Signer: {{ is: $wallet }} }}
        Block: {{ Time: {{ since: $since }} }}
        Trade: {{ Dex: {{ ProgramAddress: {{ is: "{PUMP_PROGRAM_ID}" }} }} }}
      }}
    ) {{
      Block {{ Time }}
      Transaction {{ Signature Signer }}
      Trade {{
        Buy {{
          Amount
          Account {{ Address }}
          Currency {{ MintAddress Symbol Name }}
        }}
        Sell {{
          Amount
          Account {{ Address }}
          Currency {{ MintAddress Symbol Name }}
        }}
      }}
    }}
  }}
}}
"#
    );

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
        .user_agent("strategy-replication-dataset-mvp/0.1")
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
                    Ok(value) => {
                        response_value = Some(value);
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
            "{last_error}. Try a smaller query, for example: cargo run -- build-strategy-dataset {wallet} --days 7 --limit 200"
        )
    })?;

    if let Some(errors) = response.get("errors") {
        return Err(format!("Bitquery returned errors: {errors}").into());
    }

    let rows = response
        .pointer("/data/Solana/DEXTrades")
        .and_then(Value::as_array)
        .ok_or("Bitquery response did not contain data.Solana.DEXTrades")?;

    let mut trades: Vec<DatasetTrade> = rows
        .iter()
        .filter_map(|row| parse_bitquery_trade(wallet, row))
        .collect();
    trades.sort_by_key(|trade| trade.timestamp);
    Ok(trades)
}

fn parse_bitquery_trade(wallet: &str, row: &Value) -> Option<DatasetTrade> {
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

    Some(DatasetTrade {
        tx_hash,
        wallet: wallet.to_string(),
        timestamp,
        mint: mint.to_string(),
        side,
        sol_amount,
        token_amount,
        price_sol: safe_div(sol_amount, token_amount),
    })
}

fn read_csv_trades(wallet: &str, path: &str) -> Result<Vec<DatasetTrade>, Box<dyn Error>> {
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
        let Some(timestamp) = get("timestamp").and_then(parse_time) else {
            continue;
        };
        let sol_amount = get("sol_amount")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0.0);
        let token_amount = get("token_amount")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0.0);

        trades.push(DatasetTrade {
            tx_hash: get("tx_hash").unwrap_or("csv").to_string(),
            wallet: get("wallet").unwrap_or(wallet).to_string(),
            timestamp,
            mint: get("mint").unwrap_or("unknown").to_string(),
            side,
            sol_amount,
            token_amount,
            price_sol: get("price_sol")
                .and_then(|value| value.parse().ok())
                .unwrap_or_else(|| safe_div(sol_amount, token_amount)),
        });
    }

    trades.sort_by_key(|trade| trade.timestamp);
    Ok(trades)
}

fn aggregate_positions(wallet: &str, trades: &[DatasetTrade]) -> Vec<DatasetPosition> {
    let mut by_mint: HashMap<String, Vec<&DatasetTrade>> = HashMap::new();
    for trade in trades {
        by_mint.entry(trade.mint.clone()).or_default().push(trade);
    }

    let mut positions = Vec::new();
    for (mint, mut rows) in by_mint {
        rows.sort_by_key(|trade| trade.timestamp);
        let buys: Vec<&&DatasetTrade> = rows
            .iter()
            .filter(|trade| trade.side == Side::Buy)
            .collect();
        if buys.is_empty() {
            continue;
        }

        let sells: Vec<&&DatasetTrade> = rows
            .iter()
            .filter(|trade| trade.side == Side::Sell)
            .collect();
        let first_buy = buys[0];
        let last_sell_time = sells.last().map(|trade| trade.timestamp);
        let total_buy_sol: f64 = buys.iter().map(|trade| trade.sol_amount).sum();
        let total_sell_sol: f64 = sells.iter().map(|trade| trade.sol_amount).sum();
        let bought_tokens: f64 = buys.iter().map(|trade| trade.token_amount).sum();
        let sold_tokens: f64 = sells.iter().map(|trade| trade.token_amount).sum();
        let average_buy_price_sol = safe_div(
            buys.iter().map(|trade| trade.price_sol).sum::<f64>(),
            buys.len() as f64,
        );
        let is_closed = !sells.is_empty() && sold_tokens >= bought_tokens * 0.95;
        let holding_seconds = last_sell_time.map(|sell_time| {
            sell_time
                .signed_duration_since(first_buy.timestamp)
                .num_seconds()
        });
        let realized_pnl_sol = total_sell_sol - total_buy_sol;

        positions.push(DatasetPosition {
            wallet: wallet.to_string(),
            mint,
            first_buy_tx: first_buy.tx_hash.clone(),
            first_buy_wallet: first_buy.wallet.clone(),
            first_buy_time: first_buy.timestamp,
            last_sell_time,
            buy_count: buys.len(),
            sell_count: sells.len(),
            total_buy_sol,
            total_sell_sol,
            average_buy_price_sol,
            realized_pnl_sol,
            realized_roi: safe_div(realized_pnl_sol, total_buy_sol),
            holding_seconds,
            is_closed,
        });
    }

    positions.sort_by_key(|position| position.first_buy_time);
    positions
}

fn select_positions(
    positions: &[DatasetPosition],
    profit_samples: usize,
    loss_samples: usize,
) -> Vec<SelectedPosition> {
    let mut profits: Vec<DatasetPosition> = positions
        .iter()
        .filter(|position| position.is_closed && position.realized_pnl_sol > 0.0)
        .cloned()
        .collect();
    let mut losses: Vec<DatasetPosition> = positions
        .iter()
        .filter(|position| position.is_closed && position.realized_pnl_sol < 0.0)
        .cloned()
        .collect();

    profits.sort_by_key(|position| std::cmp::Reverse(position.first_buy_time));
    losses.sort_by_key(|position| std::cmp::Reverse(position.first_buy_time));

    let mut selected = Vec::new();
    selected.extend(
        profits
            .into_iter()
            .take(profit_samples)
            .map(|position| SelectedPosition {
                position,
                sample_group: "profit".to_string(),
                kline_file: None,
                exclude_reason: String::new(),
            }),
    );
    selected.extend(
        losses
            .into_iter()
            .take(loss_samples)
            .map(|position| SelectedPosition {
                position,
                sample_group: "loss".to_string(),
                kline_file: None,
                exclude_reason: String::new(),
            }),
    );

    selected.sort_by_key(|position| position.position.first_buy_time);
    selected
}

fn fetch_and_save_gmgn_kline(
    mint: &str,
    resolution: &str,
    from: i64,
    to: i64,
    file_path: &Path,
) -> Result<(), Box<dyn Error>> {
    env::var("GMGN_API_KEY").map_err(|_| "GMGN_API_KEY is required for --kline-source gmgn")?;

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
            fs::write(file_path, serde_json::to_string_pretty(&value)?)?;
            return Ok(());
        }

        last_error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if attempt < 3 {
            std::thread::sleep(StdDuration::from_secs(attempt * 3));
        }
    }

    Err(format!("gmgn-cli failed for {}: {last_error}", short_mint(mint)).into())
}

fn sample_klines_for_position(
    mint: &str,
    resolution: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<SampleKlineCandle> {
    let step_seconds = resolution_to_seconds(resolution).unwrap_or(60);
    let mut timestamp = from;
    let mut index = 0_i64;
    let mut price = 0.000001_f64;
    let mut candles = Vec::new();

    while timestamp <= to {
        let drift = if index % 9 < 5 { 1.04 } else { 0.97 };
        let open = price;
        let close = price * drift;
        let high = open.max(close) * 1.03;
        let low = open.min(close) * 0.97;
        candles.push(SampleKlineCandle {
            mint: mint.to_string(),
            timestamp,
            open,
            high,
            low,
            close,
            volume: 1000.0 + (index % 7) as f64 * 350.0,
            resolution: resolution.to_string(),
            source: "sample".to_string(),
        });
        price = close;
        timestamp += Duration::seconds(step_seconds);
        index += 1;
    }

    candles
}

fn write_positions_csv(path: &Path, positions: &[DatasetPosition]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from(
        "wallet,mint,first_buy_tx,first_buy_wallet,first_buy_time,last_sell_time,buy_count,sell_count,total_buy_sol,total_sell_sol,average_buy_price_sol,realized_pnl_sol,realized_roi,holding_seconds,is_closed\n",
    );

    for position in positions {
        push_csv_row(
            &mut csv,
            &[
                &position.wallet,
                &position.mint,
                &position.first_buy_tx,
                &position.first_buy_wallet,
                &position.first_buy_time.to_rfc3339(),
                &format_time(position.last_sell_time),
                &position.buy_count.to_string(),
                &position.sell_count.to_string(),
                &format!("{:.9}", position.total_buy_sol),
                &format!("{:.9}", position.total_sell_sol),
                &format!("{:.12}", position.average_buy_price_sol),
                &format!("{:.9}", position.realized_pnl_sol),
                &format!("{:.6}", position.realized_roi),
                &position
                    .holding_seconds
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                if position.is_closed { "true" } else { "false" },
            ],
        );
    }

    fs::write(path, csv)?;
    Ok(())
}

fn write_selected_positions_csv(
    path: &Path,
    selected: &[SelectedPosition],
) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from(
        "wallet,mint,sample_group,first_buy_tx,first_buy_time,last_sell_time,realized_pnl_sol,realized_roi,holding_seconds,kline_file,exclude_reason\n",
    );

    for selected_position in selected {
        let position = &selected_position.position;
        push_csv_row(
            &mut csv,
            &[
                &position.wallet,
                &position.mint,
                &selected_position.sample_group,
                &position.first_buy_tx,
                &position.first_buy_time.to_rfc3339(),
                &format_time(position.last_sell_time),
                &format!("{:.9}", position.realized_pnl_sol),
                &format!("{:.6}", position.realized_roi),
                &position
                    .holding_seconds
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                selected_position.kline_file.as_deref().unwrap_or(""),
                &selected_position.exclude_reason,
            ],
        );
    }

    fs::write(path, csv)?;
    Ok(())
}

fn write_dataset_summary(
    path: &Path,
    wallet: &str,
    trades_source: &str,
    kline_source: &str,
    resolution: &str,
    positions: &[DatasetPosition],
    selected: &[SelectedPosition],
) -> Result<(), Box<dyn Error>> {
    let closed_count = positions
        .iter()
        .filter(|position| position.is_closed)
        .count();
    let profit_count = positions
        .iter()
        .filter(|position| position.is_closed && position.realized_pnl_sol > 0.0)
        .count();
    let loss_count = positions
        .iter()
        .filter(|position| position.is_closed && position.realized_pnl_sol < 0.0)
        .count();
    let selected_profit_count = selected
        .iter()
        .filter(|position| position.sample_group == "profit")
        .count();
    let selected_loss_count = selected
        .iter()
        .filter(|position| position.sample_group == "loss")
        .count();
    let kline_count = selected
        .iter()
        .filter(|position| position.kline_file.is_some())
        .count();
    let skipped_count = selected.len().saturating_sub(kline_count);

    let content = format!(
        r#"# Strategy Replication Dataset Summary

wallet: `{wallet}`

## Sources

- trades source: `{trades_source}`
- kline source: `{kline_source}`
- resolution: `{resolution}`

## Position Counts

- total positions: {}
- closed positions: {closed_count}
- profitable closed positions: {profit_count}
- losing closed positions: {loss_count}

## Selected Samples

- profit samples: {selected_profit_count}
- loss samples: {selected_loss_count}
- kline files saved: {kline_count}
- kline downloads skipped/failed: {skipped_count}

## Files

- `positions.csv`: complete aggregated position list
- `selected_positions.csv`: balanced research sample
- `klines/`: raw K-line JSON files for selected positions
- `features/`: reserved for feature extraction output
- `reports/`: reserved for comparison and rule candidate reports

## Notes

This command only builds the dataset. It does not yet decide whether the strategy is profitable or safe to copy.
"#,
        positions.len()
    );

    fs::write(path, content)?;
    Ok(())
}

fn push_csv_row(csv: &mut String, values: &[&str]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            csv.push(',');
        }
        csv.push_str(&csv_escape(value));
    }
    csv.push('\n');
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn sample_trades(wallet: &str) -> Vec<DatasetTrade> {
    let base = Utc::now() - Duration::hours(12);
    let mut trades = Vec::new();

    for index in 0..6 {
        let mint = format!("SampleProfitMint{:02}", index + 1);
        let buy_time = base + Duration::minutes(index * 20);
        let sell_time = buy_time + Duration::minutes(3 + index);
        let buy_sol = 0.2 + index as f64 * 0.03;
        let sell_sol = buy_sol * (1.3 + index as f64 * 0.08);
        trades.push(sample_trade(
            wallet,
            &mint,
            Side::Buy,
            buy_time,
            buy_sol,
            1_000_000.0,
        ));
        trades.push(sample_trade(
            wallet,
            &mint,
            Side::Sell,
            sell_time,
            sell_sol,
            1_000_000.0,
        ));
    }

    for index in 0..6 {
        let mint = format!("SampleLossMint{:02}", index + 1);
        let buy_time = base + Duration::minutes(160 + index * 18);
        let sell_time = buy_time + Duration::minutes(1 + index);
        let buy_sol = 0.25 + index as f64 * 0.02;
        let sell_sol = buy_sol * (0.65 - index as f64 * 0.03);
        trades.push(sample_trade(
            wallet,
            &mint,
            Side::Buy,
            buy_time,
            buy_sol,
            1_000_000.0,
        ));
        trades.push(sample_trade(
            wallet,
            &mint,
            Side::Sell,
            sell_time,
            sell_sol.max(0.01),
            1_000_000.0,
        ));
    }

    trades.sort_by_key(|trade| trade.timestamp);
    trades
}

fn sample_trade(
    wallet: &str,
    mint: &str,
    side: Side,
    timestamp: DateTime<Utc>,
    sol_amount: f64,
    token_amount: f64,
) -> DatasetTrade {
    DatasetTrade {
        tx_hash: format!(
            "sample-{}-{}",
            match side {
                Side::Buy => "buy",
                Side::Sell => "sell",
            },
            mint
        ),
        wallet: wallet.to_string(),
        timestamp,
        mint: mint.to_string(),
        side,
        sol_amount,
        token_amount,
        price_sol: safe_div(sol_amount, token_amount),
    }
}

fn kline_file_name(mint: &str, resolution: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> String {
    format!(
        "{}_{}_{}_{}.json",
        sanitize_path_component(mint),
        sanitize_path_component(resolution),
        from.timestamp(),
        to.timestamp()
    )
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn format_time(value: Option<DateTime<Utc>>) -> String {
    value.map(|time| time.to_rfc3339()).unwrap_or_default()
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .ok()
}

fn as_f64(value: &Value) -> Option<f64> {
    if let Some(value) = value.as_f64() {
        Some(value)
    } else if let Some(value) = value.as_str() {
        value.parse().ok()
    } else {
        None
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

fn short_mint(mint: &str) -> String {
    if mint.len() <= 12 {
        mint.to_string()
    } else {
        format!("{}...{}", &mint[..6], &mint[mint.len() - 4..])
    }
}

fn print_help() {
    println!(
        r#"strategy replication dataset MVP

Usage:
  cargo run -- build-strategy-dataset <wallet> [options]

Options:
  --trades-source bitquery|csv|sample   default: bitquery
  --trades-file trades.csv              required when --trades-source csv
  --kline-source gmgn|sample            default: gmgn
  --days 30
  --limit 2000
  --profit-samples 50
  --loss-samples 50
  --resolution 1m
  --pre-minutes 20
  --post-minutes 20
  --out data/strategy_research/wallets/<wallet>

Examples:
  cargo run -- build-strategy-dataset 55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr \
    --trades-source bitquery \
    --kline-source gmgn \
    --profit-samples 50 \
    --loss-samples 50 \
    --resolution 1m \
    --pre-minutes 20 \
    --post-minutes 20

  cargo run -- build-strategy-dataset test-wallet --trades-source sample --kline-source sample

Environment:
  export BITQUERY_TOKEN=<your token>
  export GMGN_API_KEY=<your key>
"#
    );
}
