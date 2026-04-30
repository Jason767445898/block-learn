use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
struct WalletTrade {
    tx_hash: String,
    timestamp: DateTime<Utc>,
    mint: String,
    side: Side,
    sol_amount: f64,
    token_amount: f64,
}

#[derive(Debug, Clone)]
struct Position {
    wallet: String,
    mint: String,
    first_buy_tx: String,
    first_buy_time: DateTime<Utc>,
    last_sell_time: DateTime<Utc>,
    buy_count: usize,
    sell_count: usize,
    total_buy_sol: f64,
    total_sell_sol: f64,
    realized_pnl_sol: f64,
    realized_roi: f64,
    holding_seconds: i64,
}

#[derive(Debug, Clone)]
struct SelectedPosition {
    position: Position,
    sample_group: String,
    activity_file: String,
    exclude_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActivityBucket {
    mint: String,
    timestamp: DateTime<Utc>,
    buy_count: usize,
    sell_count: usize,
    buy_volume_sol: f64,
    sell_volume_sol: f64,
    unique_buyers: usize,
    unique_sellers: usize,
    large_buy_count: usize,
    large_sell_count: usize,
    large_buy_volume_sol: f64,
    large_sell_volume_sol: f64,
    top_buyer_volume_share: f64,
    new_wallet_buy_count: usize,
    smart_wallet_buy_count: usize,
    source: String,
}

#[derive(Debug, Clone)]
struct ActivityFeature {
    wallet: String,
    mint: String,
    sample_group: String,
    first_buy_time: DateTime<Utc>,
    last_sell_time: DateTime<Utc>,
    realized_pnl_sol: f64,
    realized_roi: f64,
    holding_seconds: i64,
    pre_1m_net_buy_volume_sol: Option<f64>,
    pre_3m_net_buy_volume_sol: Option<f64>,
    pre_5m_net_buy_volume_sol: Option<f64>,
    pre_10m_net_buy_volume_sol: Option<f64>,
    pre_5m_buy_sell_ratio: Option<f64>,
    pre_5m_unique_buyers: Option<f64>,
    pre_5m_unique_buyers_growth: Option<f64>,
    pre_5m_large_buy_count: Option<f64>,
    pre_5m_large_buy_volume_sol: Option<f64>,
    pre_5m_large_buy_share: Option<f64>,
    pre_5m_top_buyer_volume_share: Option<f64>,
    pre_5m_smart_wallet_buy_count: Option<f64>,
    pre_5m_new_wallet_buy_count: Option<f64>,
    activity_acceleration_1m_vs_5m: Option<f64>,
    holding_net_buy_volume_sol: Option<f64>,
    holding_buy_sell_ratio: Option<f64>,
    pre_exit_5m_sell_pressure: Option<f64>,
    post_exit_20m_net_buy_volume_sol: Option<f64>,
    activity_label: String,
    activity_file: String,
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Pre5mNetBuy,
    Pre5mBuySellRatio,
    Pre5mUniqueBuyers,
    Pre5mUniqueBuyersGrowth,
    Pre5mLargeBuyCount,
    Pre5mLargeBuyShare,
    Pre5mTopBuyerShareMax,
    Pre5mSmartWalletBuyCount,
    Acceleration,
}

#[derive(Debug)]
struct RuleEvaluation {
    expression: String,
    matched: usize,
    profit_count: usize,
    loss_count: usize,
    win_rate: f64,
    lift_vs_baseline: f64,
    avg_roi: f64,
    avg_pnl_sol: f64,
    avg_holding_seconds: f64,
}

pub async fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str) {
        Some("build") => build_dataset(&args[1..]),
        Some("extract") => extract_features(&args[1..]),
        Some("rules") => generate_rules(&args[1..]),
        Some("all") => run_all(&args[1..]),
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn run_all(args: &[String]) -> Result<(), Box<dyn Error>> {
    let wallet = args
        .first()
        .cloned()
        .unwrap_or_else(|| "sample-gmgn-wallet".to_string());
    let out = get_arg(args, "--out").unwrap_or_else(|| {
        format!(
            "data/gmgn_reverse/wallets/{}",
            sanitize_path_component(&wallet)
        )
    });
    let mut build_args = vec![wallet.clone(), "--out".to_string(), out.clone()];
    build_args.extend_from_slice(args);
    build_dataset(&build_args)?;
    extract_features(&["--dataset".to_string(), out.clone()])?;
    generate_rules(&["--dataset".to_string(), out])?;
    Ok(())
}

fn build_dataset(args: &[String]) -> Result<(), Box<dyn Error>> {
    let wallet = args
        .first()
        .cloned()
        .ok_or("wallet is required: cargo run -- gmgn-reverse build <wallet>")?;
    let wallet_trades_source =
        get_arg(args, "--wallet-trades-source").unwrap_or_else(|| "sample".to_string());
    let activity_source = get_arg(args, "--activity-source").unwrap_or_else(|| "sample".to_string());
    let days = parse_arg(args, "--days", 30_i64);
    let profit_samples = parse_arg(args, "--profit-samples", 50_usize);
    let loss_samples = parse_arg(args, "--loss-samples", 50_usize);
    let pre_minutes = parse_arg(args, "--pre-minutes", 60_i64);
    let post_minutes = parse_arg(args, "--post-minutes", 60_i64);
    let large_trade_sol_threshold = parse_arg(args, "--large-trade-sol-threshold", 2.0_f64);
    let out_dir = PathBuf::from(get_arg(args, "--out").unwrap_or_else(|| {
        format!(
            "data/gmgn_reverse/wallets/{}",
            sanitize_path_component(&wallet)
        )
    }));

    let trades = match wallet_trades_source.as_str() {
        "csv" => {
            let file = get_arg(args, "--wallet-trades-file")
                .ok_or("--wallet-trades-file is required when --wallet-trades-source csv")?;
            read_wallet_trades_csv(&wallet, &file)?
        }
        "gmgn-json" => {
            let file = get_arg(args, "--wallet-trades-file")
                .ok_or("--wallet-trades-file is required when --wallet-trades-source gmgn-json")?;
            read_wallet_trades_json(&wallet, &file)?
        }
        "sample" => sample_wallet_trades(&wallet),
        other => return Err(format!("unknown --wallet-trades-source: {other}").into()),
    };

    let cutoff = Utc::now() - Duration::days(days);
    let recent_trades: Vec<WalletTrade> = trades
        .into_iter()
        .filter(|trade| trade.timestamp >= cutoff || wallet_trades_source == "sample")
        .collect();
    let positions = aggregate_positions(&wallet, &recent_trades);
    let mut selected = select_positions(&positions, profit_samples, loss_samples);

    let activity_dir = out_dir.join("activity");
    let feature_dir = out_dir.join("features");
    let report_dir = out_dir.join("reports");
    fs::create_dir_all(&activity_dir)?;
    fs::create_dir_all(&feature_dir)?;
    fs::create_dir_all(&report_dir)?;

    for selected_position in &mut selected {
        let from = selected_position.position.first_buy_time - Duration::minutes(pre_minutes);
        let to = selected_position.position.last_sell_time + Duration::minutes(post_minutes);
        let file_name = activity_file_name(&selected_position.position.mint, from, to);
        let relative_file = PathBuf::from("activity").join(&file_name);
        let file_path = activity_dir.join(&file_name);

        if file_path.exists() {
            selected_position.activity_file = path_to_string(&relative_file);
            continue;
        }

        match activity_source.as_str() {
            "sample" => {
                let buckets = sample_activity_buckets(
                    &selected_position.position,
                    &selected_position.sample_group,
                    from,
                    to,
                    large_trade_sol_threshold,
                );
                fs::write(&file_path, serde_json::to_string_pretty(&buckets)?)?;
                selected_position.activity_file = path_to_string(&relative_file);
            }
            "csv-dir" => {
                let dir = get_arg(args, "--activity-dir")
                    .ok_or("--activity-dir is required when --activity-source csv-dir")?;
                let source_file = PathBuf::from(dir).join(format!(
                    "{}.csv",
                    sanitize_path_component(&selected_position.position.mint)
                ));
                if source_file.exists() {
                    let buckets = read_activity_csv(&source_file, &selected_position.position.mint)?;
                    fs::write(&file_path, serde_json::to_string_pretty(&buckets)?)?;
                    selected_position.activity_file = path_to_string(&relative_file);
                } else {
                    selected_position.exclude_reason =
                        format!("activity_csv_not_found: {}", source_file.display());
                }
            }
            "json-dir" => {
                let dir = get_arg(args, "--activity-dir")
                    .ok_or("--activity-dir is required when --activity-source json-dir")?;
                let source_file = PathBuf::from(dir).join(format!(
                    "{}.json",
                    sanitize_path_component(&selected_position.position.mint)
                ));
                if source_file.exists() {
                    let buckets = read_activity_json(&source_file, &selected_position.position.mint)?;
                    fs::write(&file_path, serde_json::to_string_pretty(&buckets)?)?;
                    selected_position.activity_file = path_to_string(&relative_file);
                } else {
                    selected_position.exclude_reason =
                        format!("activity_json_not_found: {}", source_file.display());
                }
            }
            "none" => {
                selected_position.exclude_reason = "activity_source_none".to_string();
            }
            other => return Err(format!("unknown --activity-source: {other}").into()),
        }
    }

    write_positions_csv(&out_dir.join("positions.csv"), &positions)?;
    write_selected_positions_csv(&out_dir.join("selected_positions.csv"), &selected)?;
    write_dataset_summary(
        &report_dir.join("dataset_summary.md"),
        &wallet,
        &wallet_trades_source,
        &activity_source,
        &positions,
        &selected,
    )?;

    println!("gmgn reverse dataset");
    println!("wallet: {wallet}");
    println!("out: {}", out_dir.display());
    println!("trades: {}", recent_trades.len());
    println!("positions: {}", positions.len());
    println!(
        "selected: {} profit / {} loss",
        selected.iter().filter(|p| p.sample_group == "profit").count(),
        selected.iter().filter(|p| p.sample_group == "loss").count()
    );
    println!(
        "activity: {} saved / {} skipped",
        selected.iter().filter(|p| !p.activity_file.is_empty()).count(),
        selected.iter().filter(|p| p.activity_file.is_empty()).count()
    );
    Ok(())
}

fn extract_features(args: &[String]) -> Result<(), Box<dyn Error>> {
    let dataset_dir = PathBuf::from(
        get_arg(args, "--dataset")
            .or_else(|| args.first().cloned())
            .ok_or("--dataset <path> is required")?,
    );
    let selected = read_selected_positions(&dataset_dir.join("selected_positions.csv"))?;
    let feature_dir = dataset_dir.join("features");
    let report_dir = dataset_dir.join("reports");
    fs::create_dir_all(&feature_dir)?;
    fs::create_dir_all(&report_dir)?;

    let mut rows = Vec::new();
    let mut skipped = 0usize;
    for position in selected {
        if !position.exclude_reason.is_empty() || position.activity_file.is_empty() {
            skipped += 1;
            continue;
        }
        let activity_path = resolve_dataset_path(&dataset_dir, &position.activity_file);
        let buckets = read_activity_json(&activity_path, &position.position.mint)?;
        if buckets.is_empty() {
            skipped += 1;
            continue;
        }
        rows.push(build_activity_feature(&position, &buckets));
    }

    write_activity_features_csv(&feature_dir.join("activity_entry_features.csv"), &rows)?;
    write_activity_features_csv(&feature_dir.join("enriched_position_features.csv"), &rows)?;
    write_activity_feature_comparison(
        &report_dir.join("activity_feature_comparison.md"),
        &rows,
        skipped,
    )?;

    println!("gmgn reverse activity features");
    println!("dataset: {}", dataset_dir.display());
    println!("positions analyzed: {}", rows.len());
    println!("skipped: {skipped}");
    Ok(())
}

fn generate_rules(args: &[String]) -> Result<(), Box<dyn Error>> {
    let dataset_dir = PathBuf::from(
        get_arg(args, "--dataset")
            .or_else(|| args.first().cloned())
            .ok_or("--dataset <path> is required")?,
    );
    let min_matches = parse_arg(args, "--min-matches", 6_usize);
    let top = parse_arg(args, "--top", 25_usize);
    let feature_path = dataset_dir.join("features").join("enriched_position_features.csv");
    let report_dir = dataset_dir.join("reports");
    fs::create_dir_all(&report_dir)?;
    let features = read_activity_features_csv(&feature_path)?;
    if features.is_empty() {
        return Err(format!("no features found: {}", feature_path.display()).into());
    }

    let baseline = safe_div(
        features
            .iter()
            .filter(|row| row.sample_group == "profit")
            .count() as f64,
        features.len() as f64,
    );
    let mut evaluations = Vec::new();
    for rule in build_rules() {
        if let Some(evaluation) = evaluate_rule(&features, &rule, baseline) {
            if evaluation.matched >= min_matches {
                evaluations.push(evaluation);
            }
        }
    }
    evaluations.sort_by(|a, b| {
        b.win_rate
            .total_cmp(&a.win_rate)
            .then_with(|| b.matched.cmp(&a.matched))
            .then_with(|| b.avg_roi.total_cmp(&a.avg_roi))
    });
    write_rule_candidates_csv(&report_dir.join("activity_rule_candidates.csv"), &evaluations)?;
    write_rule_candidates_md(
        &report_dir.join("activity_rule_candidates.md"),
        &features,
        baseline,
        min_matches,
        top,
        &evaluations,
    )?;
    write_final_report(
        &report_dir.join("gmgn_reverse_analysis_report.md"),
        &features,
        baseline,
        &evaluations,
        top,
    )?;

    println!("gmgn reverse activity rules");
    println!("dataset: {}", dataset_dir.display());
    println!("features: {}", features.len());
    println!("candidate rules: {}", evaluations.len());
    Ok(())
}

fn aggregate_positions(wallet: &str, trades: &[WalletTrade]) -> Vec<Position> {
    let mut by_mint: HashMap<String, Vec<&WalletTrade>> = HashMap::new();
    for trade in trades {
        by_mint.entry(trade.mint.clone()).or_default().push(trade);
    }

    let mut positions = Vec::new();
    for (mint, mut rows) in by_mint {
        rows.sort_by_key(|trade| trade.timestamp);
        let buys: Vec<&&WalletTrade> = rows.iter().filter(|trade| trade.side == Side::Buy).collect();
        let sells: Vec<&&WalletTrade> = rows.iter().filter(|trade| trade.side == Side::Sell).collect();
        if buys.is_empty() || sells.is_empty() {
            continue;
        }
        let first_buy = buys[0];
        let last_sell = sells[sells.len() - 1];
        let total_buy_sol: f64 = buys.iter().map(|trade| trade.sol_amount).sum();
        let total_sell_sol: f64 = sells.iter().map(|trade| trade.sol_amount).sum();
        let bought_tokens: f64 = buys.iter().map(|trade| trade.token_amount).sum();
        let sold_tokens: f64 = sells.iter().map(|trade| trade.token_amount).sum();
        if sold_tokens < bought_tokens * 0.90 {
            continue;
        }
        let realized_pnl_sol = total_sell_sol - total_buy_sol;
        let holding_seconds = last_sell
            .timestamp
            .signed_duration_since(first_buy.timestamp)
            .num_seconds();
        positions.push(Position {
            wallet: wallet.to_string(),
            mint,
            first_buy_tx: first_buy.tx_hash.clone(),
            first_buy_time: first_buy.timestamp,
            last_sell_time: last_sell.timestamp,
            buy_count: buys.len(),
            sell_count: sells.len(),
            total_buy_sol,
            total_sell_sol,
            realized_pnl_sol,
            realized_roi: safe_div(realized_pnl_sol, total_buy_sol),
            holding_seconds,
        });
    }
    positions.sort_by_key(|position| position.first_buy_time);
    positions
}

fn select_positions(
    positions: &[Position],
    profit_samples: usize,
    loss_samples: usize,
) -> Vec<SelectedPosition> {
    let mut profits: Vec<Position> = positions
        .iter()
        .filter(|position| position.realized_pnl_sol > 0.0)
        .cloned()
        .collect();
    let mut losses: Vec<Position> = positions
        .iter()
        .filter(|position| position.realized_pnl_sol < 0.0)
        .cloned()
        .collect();
    profits.sort_by_key(|position| std::cmp::Reverse(position.first_buy_time));
    losses.sort_by_key(|position| std::cmp::Reverse(position.first_buy_time));

    let mut selected = Vec::new();
    selected.extend(profits.into_iter().take(profit_samples).map(|position| SelectedPosition {
        position,
        sample_group: "profit".to_string(),
        activity_file: String::new(),
        exclude_reason: String::new(),
    }));
    selected.extend(losses.into_iter().take(loss_samples).map(|position| SelectedPosition {
        position,
        sample_group: "loss".to_string(),
        activity_file: String::new(),
        exclude_reason: String::new(),
    }));
    selected.sort_by_key(|position| position.position.first_buy_time);
    selected
}

fn build_activity_feature(position: &SelectedPosition, buckets: &[ActivityBucket]) -> ActivityFeature {
    let entry = position.position.first_buy_time;
    let exit = position.position.last_sell_time;
    let pre_1 = window_sum(buckets, entry - Duration::minutes(1), entry);
    let pre_3 = window_sum(buckets, entry - Duration::minutes(3), entry);
    let pre_5 = window_sum(buckets, entry - Duration::minutes(5), entry);
    let pre_10 = window_sum(buckets, entry - Duration::minutes(10), entry);
    let pre_10_5 = window_sum(buckets, entry - Duration::minutes(10), entry - Duration::minutes(5));
    let holding = window_sum(buckets, entry, exit);
    let pre_exit = window_sum(buckets, exit - Duration::minutes(5), exit);
    let post_exit = window_sum(buckets, exit, exit + Duration::minutes(20));

    let pre_5_net = pre_5.net_buy_volume_sol();
    let pre_5_buy_sell_ratio = ratio(pre_5.buy_volume_sol, pre_5.sell_volume_sol);
    let pre_5_large_buy_share = ratio(pre_5.large_buy_volume_sol, pre_5.buy_volume_sol);
    let unique_growth = if pre_10_5.unique_buyers > 0 {
        Some(safe_div(
            pre_5.unique_buyers as f64 - pre_10_5.unique_buyers as f64,
            pre_10_5.unique_buyers as f64,
        ))
    } else {
        None
    };
    let acceleration = ratio(pre_1.net_buy_volume_sol(), safe_div(pre_5.net_buy_volume_sol(), 5.0));
    let sell_pressure = ratio(pre_exit.sell_volume_sol, pre_exit.buy_volume_sol);
    let activity_label = classify_activity(pre_5.net_buy_volume_sol(), pre_5_buy_sell_ratio, pre_5.large_buy_count);

    ActivityFeature {
        wallet: position.position.wallet.clone(),
        mint: position.position.mint.clone(),
        sample_group: position.sample_group.clone(),
        first_buy_time: entry,
        last_sell_time: exit,
        realized_pnl_sol: position.position.realized_pnl_sol,
        realized_roi: position.position.realized_roi,
        holding_seconds: position.position.holding_seconds,
        pre_1m_net_buy_volume_sol: Some(pre_1.net_buy_volume_sol()),
        pre_3m_net_buy_volume_sol: Some(pre_3.net_buy_volume_sol()),
        pre_5m_net_buy_volume_sol: Some(pre_5_net),
        pre_10m_net_buy_volume_sol: Some(pre_10.net_buy_volume_sol()),
        pre_5m_buy_sell_ratio: pre_5_buy_sell_ratio,
        pre_5m_unique_buyers: Some(pre_5.unique_buyers as f64),
        pre_5m_unique_buyers_growth: unique_growth,
        pre_5m_large_buy_count: Some(pre_5.large_buy_count as f64),
        pre_5m_large_buy_volume_sol: Some(pre_5.large_buy_volume_sol),
        pre_5m_large_buy_share: pre_5_large_buy_share,
        pre_5m_top_buyer_volume_share: average_top_buyer_share(buckets, entry - Duration::minutes(5), entry),
        pre_5m_smart_wallet_buy_count: Some(pre_5.smart_wallet_buy_count as f64),
        pre_5m_new_wallet_buy_count: Some(pre_5.new_wallet_buy_count as f64),
        activity_acceleration_1m_vs_5m: acceleration,
        holding_net_buy_volume_sol: Some(holding.net_buy_volume_sol()),
        holding_buy_sell_ratio: ratio(holding.buy_volume_sol, holding.sell_volume_sol),
        pre_exit_5m_sell_pressure: sell_pressure,
        post_exit_20m_net_buy_volume_sol: Some(post_exit.net_buy_volume_sol()),
        activity_label,
        activity_file: position.activity_file.clone(),
    }
}

#[derive(Default)]
struct ActivitySum {
    buy_count: usize,
    sell_count: usize,
    buy_volume_sol: f64,
    sell_volume_sol: f64,
    unique_buyers: usize,
    unique_sellers: usize,
    large_buy_count: usize,
    large_sell_count: usize,
    large_buy_volume_sol: f64,
    large_sell_volume_sol: f64,
    new_wallet_buy_count: usize,
    smart_wallet_buy_count: usize,
}

impl ActivitySum {
    fn net_buy_volume_sol(&self) -> f64 {
        self.buy_volume_sol - self.sell_volume_sol
    }
}

fn window_sum(buckets: &[ActivityBucket], from: DateTime<Utc>, to: DateTime<Utc>) -> ActivitySum {
    let mut sum = ActivitySum::default();
    for bucket in buckets
        .iter()
        .filter(|bucket| bucket.timestamp >= from && bucket.timestamp <= to)
    {
        sum.buy_count += bucket.buy_count;
        sum.sell_count += bucket.sell_count;
        sum.buy_volume_sol += bucket.buy_volume_sol;
        sum.sell_volume_sol += bucket.sell_volume_sol;
        sum.unique_buyers += bucket.unique_buyers;
        sum.unique_sellers += bucket.unique_sellers;
        sum.large_buy_count += bucket.large_buy_count;
        sum.large_sell_count += bucket.large_sell_count;
        sum.large_buy_volume_sol += bucket.large_buy_volume_sol;
        sum.large_sell_volume_sol += bucket.large_sell_volume_sol;
        sum.new_wallet_buy_count += bucket.new_wallet_buy_count;
        sum.smart_wallet_buy_count += bucket.smart_wallet_buy_count;
    }
    sum
}

fn average_top_buyer_share(
    buckets: &[ActivityBucket],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Option<f64> {
    let values: Vec<f64> = buckets
        .iter()
        .filter(|bucket| bucket.timestamp >= from && bucket.timestamp <= to)
        .map(|bucket| bucket.top_buyer_volume_share)
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn build_rules() -> Vec<Vec<(Field, f64)>> {
    let mut rules = Vec::new();
    for threshold in [-2.0, 0.0, 1.0, 3.0, 5.0, 10.0] {
        rules.push(vec![(Field::Pre5mNetBuy, threshold)]);
    }
    for threshold in [1.0, 1.5, 2.0, 3.0, 5.0] {
        rules.push(vec![(Field::Pre5mBuySellRatio, threshold)]);
    }
    for threshold in [3.0, 5.0, 8.0, 12.0, 20.0] {
        rules.push(vec![(Field::Pre5mUniqueBuyers, threshold)]);
    }
    for threshold in [1.0, 2.0, 3.0, 5.0] {
        rules.push(vec![(Field::Pre5mLargeBuyCount, threshold)]);
    }
    for threshold in [0.15, 0.25, 0.40, 0.60] {
        rules.push(vec![(Field::Pre5mLargeBuyShare, threshold)]);
    }
    for threshold in [0.35, 0.50, 0.65] {
        rules.push(vec![(Field::Pre5mTopBuyerShareMax, threshold)]);
    }
    for net in [0.0, 1.0, 3.0, 5.0] {
        for ratio_threshold in [1.5, 2.0, 3.0] {
            rules.push(vec![
                (Field::Pre5mNetBuy, net),
                (Field::Pre5mBuySellRatio, ratio_threshold),
            ]);
        }
    }
    for net in [1.0, 3.0, 5.0] {
        for buyers in [5.0, 8.0, 12.0] {
            rules.push(vec![(Field::Pre5mNetBuy, net), (Field::Pre5mUniqueBuyers, buyers)]);
        }
    }
    for large in [1.0, 2.0, 3.0] {
        for concentration in [0.50, 0.65] {
            rules.push(vec![
                (Field::Pre5mLargeBuyCount, large),
                (Field::Pre5mTopBuyerShareMax, concentration),
            ]);
        }
    }
    for smart in [1.0, 2.0] {
        for accel in [1.5, 2.0, 3.0] {
            rules.push(vec![(Field::Pre5mSmartWalletBuyCount, smart), (Field::Acceleration, accel)]);
        }
    }
    for growth in [0.25, 0.50, 1.0] {
        for ratio_threshold in [1.5, 2.0] {
            rules.push(vec![
                (Field::Pre5mUniqueBuyersGrowth, growth),
                (Field::Pre5mBuySellRatio, ratio_threshold),
            ]);
        }
    }
    rules
}

fn evaluate_rule(
    features: &[ActivityFeature],
    rule: &[(Field, f64)],
    baseline: f64,
) -> Option<RuleEvaluation> {
    let matched: Vec<&ActivityFeature> = features
        .iter()
        .filter(|feature| rule.iter().all(|condition| matches_condition(feature, *condition)))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let profit_count = matched
        .iter()
        .filter(|feature| feature.sample_group == "profit")
        .count();
    let loss_count = matched.len().saturating_sub(profit_count);
    let win_rate = safe_div(profit_count as f64, matched.len() as f64);
    Some(RuleEvaluation {
        expression: rule_expression(rule),
        matched: matched.len(),
        profit_count,
        loss_count,
        win_rate,
        lift_vs_baseline: win_rate - baseline,
        avg_roi: matched.iter().map(|feature| feature.realized_roi).sum::<f64>() / matched.len() as f64,
        avg_pnl_sol: matched.iter().map(|feature| feature.realized_pnl_sol).sum::<f64>() / matched.len() as f64,
        avg_holding_seconds: matched
            .iter()
            .map(|feature| feature.holding_seconds as f64)
            .sum::<f64>()
            / matched.len() as f64,
    })
}

fn matches_condition(feature: &ActivityFeature, condition: (Field, f64)) -> bool {
    let (field, threshold) = condition;
    match field {
        Field::Pre5mNetBuy => feature.pre_5m_net_buy_volume_sol.is_some_and(|v| v >= threshold),
        Field::Pre5mBuySellRatio => feature.pre_5m_buy_sell_ratio.is_some_and(|v| v >= threshold),
        Field::Pre5mUniqueBuyers => feature.pre_5m_unique_buyers.is_some_and(|v| v >= threshold),
        Field::Pre5mUniqueBuyersGrowth => feature.pre_5m_unique_buyers_growth.is_some_and(|v| v >= threshold),
        Field::Pre5mLargeBuyCount => feature.pre_5m_large_buy_count.is_some_and(|v| v >= threshold),
        Field::Pre5mLargeBuyShare => feature.pre_5m_large_buy_share.is_some_and(|v| v >= threshold),
        Field::Pre5mTopBuyerShareMax => feature.pre_5m_top_buyer_volume_share.is_some_and(|v| v <= threshold),
        Field::Pre5mSmartWalletBuyCount => feature.pre_5m_smart_wallet_buy_count.is_some_and(|v| v >= threshold),
        Field::Acceleration => feature.activity_acceleration_1m_vs_5m.is_some_and(|v| v >= threshold),
    }
}

fn rule_expression(rule: &[(Field, f64)]) -> String {
    rule.iter()
        .map(|(field, threshold)| match field {
            Field::Pre5mNetBuy => format!("pre_5m_net_buy_volume_sol >= {threshold:.2}"),
            Field::Pre5mBuySellRatio => format!("pre_5m_buy_sell_ratio >= {threshold:.2}"),
            Field::Pre5mUniqueBuyers => format!("pre_5m_unique_buyers >= {threshold:.0}"),
            Field::Pre5mUniqueBuyersGrowth => {
                format!("pre_5m_unique_buyers_growth >= {threshold:.2}")
            }
            Field::Pre5mLargeBuyCount => format!("pre_5m_large_buy_count >= {threshold:.0}"),
            Field::Pre5mLargeBuyShare => format!("pre_5m_large_buy_share >= {threshold:.2}"),
            Field::Pre5mTopBuyerShareMax => {
                format!("pre_5m_top_buyer_volume_share <= {threshold:.2}")
            }
            Field::Pre5mSmartWalletBuyCount => {
                format!("pre_5m_smart_wallet_buy_count >= {threshold:.0}")
            }
            Field::Acceleration => format!("activity_acceleration_1m_vs_5m >= {threshold:.2}"),
        })
        .collect::<Vec<String>>()
        .join(" AND ")
}

fn read_wallet_trades_csv(_wallet: &str, path: &str) -> Result<Vec<WalletTrade>, Box<dyn Error>> {
    let rows = read_csv_rows(Path::new(path))?;
    let mut trades = Vec::new();
    for row in rows {
        let side = match row.get("side").map(|v| v.to_ascii_lowercase()).as_deref() {
            Some("buy") => Side::Buy,
            Some("sell") => Side::Sell,
            _ => continue,
        };
        let Some(timestamp) = row.get("timestamp").and_then(|value| parse_time(value)) else {
            continue;
        };
        let sol_amount = row.get("sol_amount").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let token_amount = row
            .get("token_amount")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        trades.push(WalletTrade {
            tx_hash: row.get("tx_hash").cloned().unwrap_or_else(|| "csv".to_string()),
            timestamp,
            mint: row.get("mint").cloned().unwrap_or_else(|| "unknown".to_string()),
            side,
            sol_amount,
            token_amount,
        });
    }
    trades.sort_by_key(|trade| trade.timestamp);
    Ok(trades)
}

fn read_wallet_trades_json(wallet: &str, path: &str) -> Result<Vec<WalletTrade>, Box<dyn Error>> {
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let rows = json_rows(&value).ok_or("wallet trade JSON has no array rows")?;
    let mut trades = Vec::new();
    for row in rows {
        if let Some(trade) = parse_wallet_trade_json_row(wallet, row) {
            trades.push(trade);
        }
    }
    trades.sort_by_key(|trade| trade.timestamp);
    Ok(trades)
}

fn parse_wallet_trade_json_row(_wallet: &str, row: &Value) -> Option<WalletTrade> {
    let timestamp = string_at(row, &["timestamp", "time", "block_time", "created_at"]).and_then(parse_time)?;
    let side_value = string_at(row, &["side", "event", "trade_type"])?.to_ascii_lowercase();
    let side = if side_value.contains("buy") {
        Side::Buy
    } else if side_value.contains("sell") {
        Side::Sell
    } else {
        return None;
    };
    let mint = string_at(row, &["mint", "address", "token_address", "base_address"])?.to_string();
    let sol_amount = number_at(row, &["sol_amount", "quote_amount", "amount_usd", "volume_sol"]).unwrap_or(0.0);
    let token_amount = number_at(row, &["token_amount", "base_amount", "amount"]).unwrap_or(0.0);
    Some(WalletTrade {
        tx_hash: string_at(row, &["tx_hash", "hash", "signature"])
            .unwrap_or("gmgn-json")
            .to_string(),
        timestamp,
        mint,
        side,
        sol_amount,
        token_amount,
    })
}

fn read_activity_csv(path: &Path, mint: &str) -> Result<Vec<ActivityBucket>, Box<dyn Error>> {
    let rows = read_csv_rows(path)?;
    let mut buckets = Vec::new();
    for row in rows {
        let Some(timestamp) = row.get("timestamp").and_then(|value| parse_time(value)) else {
            continue;
        };
        buckets.push(ActivityBucket {
            mint: row.get("mint").cloned().unwrap_or_else(|| mint.to_string()),
            timestamp,
            buy_count: csv_usize(&row, "buy_count"),
            sell_count: csv_usize(&row, "sell_count"),
            buy_volume_sol: csv_f64(&row, "buy_volume_sol"),
            sell_volume_sol: csv_f64(&row, "sell_volume_sol"),
            unique_buyers: csv_usize(&row, "unique_buyers"),
            unique_sellers: csv_usize(&row, "unique_sellers"),
            large_buy_count: csv_usize(&row, "large_buy_count"),
            large_sell_count: csv_usize(&row, "large_sell_count"),
            large_buy_volume_sol: csv_f64(&row, "large_buy_volume_sol"),
            large_sell_volume_sol: csv_f64(&row, "large_sell_volume_sol"),
            top_buyer_volume_share: csv_f64(&row, "top_buyer_volume_share"),
            new_wallet_buy_count: csv_usize(&row, "new_wallet_buy_count"),
            smart_wallet_buy_count: csv_usize(&row, "smart_wallet_buy_count"),
            source: row.get("source").cloned().unwrap_or_else(|| "gmgn-csv".to_string()),
        });
    }
    buckets.sort_by_key(|bucket| bucket.timestamp);
    Ok(buckets)
}

fn read_activity_json(path: &Path, mint: &str) -> Result<Vec<ActivityBucket>, Box<dyn Error>> {
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let rows = json_rows(&value).ok_or_else(|| format!("activity JSON has no rows: {}", path.display()))?;
    let mut buckets = Vec::new();
    for row in rows {
        if let Some(bucket) = parse_activity_json_row(row, mint) {
            buckets.push(bucket);
        }
    }
    buckets.sort_by_key(|bucket| bucket.timestamp);
    Ok(buckets)
}

fn parse_activity_json_row(row: &Value, mint: &str) -> Option<ActivityBucket> {
    let timestamp = string_at(row, &["timestamp", "time", "bucket_time", "created_at"])
        .and_then(parse_time)
        .or_else(|| unix_at(row, &["timestamp", "time", "block_time"]))?;
    Some(ActivityBucket {
        mint: string_at(row, &["mint", "address", "token_address"])
            .unwrap_or(mint)
            .to_string(),
        timestamp,
        buy_count: number_at(row, &["buy_count", "buys"]).unwrap_or(0.0) as usize,
        sell_count: number_at(row, &["sell_count", "sells"]).unwrap_or(0.0) as usize,
        buy_volume_sol: number_at(row, &["buy_volume_sol", "buy_volume", "buy_vol"]).unwrap_or(0.0),
        sell_volume_sol: number_at(row, &["sell_volume_sol", "sell_volume", "sell_vol"]).unwrap_or(0.0),
        unique_buyers: number_at(row, &["unique_buyers", "buyers"]).unwrap_or(0.0) as usize,
        unique_sellers: number_at(row, &["unique_sellers", "sellers"]).unwrap_or(0.0) as usize,
        large_buy_count: number_at(row, &["large_buy_count"]).unwrap_or(0.0) as usize,
        large_sell_count: number_at(row, &["large_sell_count"]).unwrap_or(0.0) as usize,
        large_buy_volume_sol: number_at(row, &["large_buy_volume_sol"]).unwrap_or(0.0),
        large_sell_volume_sol: number_at(row, &["large_sell_volume_sol"]).unwrap_or(0.0),
        top_buyer_volume_share: number_at(row, &["top_buyer_volume_share"]).unwrap_or(0.0),
        new_wallet_buy_count: number_at(row, &["new_wallet_buy_count"]).unwrap_or(0.0) as usize,
        smart_wallet_buy_count: number_at(row, &["smart_wallet_buy_count"]).unwrap_or(0.0) as usize,
        source: string_at(row, &["source"]).unwrap_or("gmgn-json").to_string(),
    })
}

fn sample_wallet_trades(_wallet: &str) -> Vec<WalletTrade> {
    let base = Utc::now() - Duration::days(2);
    let mut trades = Vec::new();
    for i in 0..24 {
        let profit = i % 3 != 0;
        let mint = format!("SampleMint{:02}", i);
        let first_buy = base + Duration::minutes(i as i64 * 70);
        let buy_sol = 1.0 + (i % 5) as f64 * 0.35;
        let sell_sol = if profit { buy_sol * 1.35 } else { buy_sol * 0.72 };
        trades.push(WalletTrade {
            tx_hash: format!("sample-buy-{i}"),
            timestamp: first_buy,
            mint: mint.clone(),
            side: Side::Buy,
            sol_amount: buy_sol,
            token_amount: 1_000_000.0,
        });
        if i % 4 == 0 {
            trades.push(WalletTrade {
                tx_hash: format!("sample-add-{i}"),
                timestamp: first_buy + Duration::seconds(45),
                mint: mint.clone(),
                side: Side::Buy,
                sol_amount: buy_sol * 0.40,
                token_amount: 350_000.0,
            });
        }
        trades.push(WalletTrade {
            tx_hash: format!("sample-sell-{i}"),
            timestamp: first_buy + Duration::seconds(if profit { 180 } else { 95 }),
            mint,
            side: Side::Sell,
            sol_amount: sell_sol,
            token_amount: if i % 4 == 0 { 1_350_000.0 } else { 1_000_000.0 },
        });
    }
    trades.sort_by_key(|trade| trade.timestamp);
    trades
}

fn sample_activity_buckets(
    position: &Position,
    sample_group: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    large_trade_sol_threshold: f64,
) -> Vec<ActivityBucket> {
    let mut timestamp = from;
    let mut buckets = Vec::new();
    let mut index = 0usize;
    while timestamp <= to {
        let minutes_to_entry = position
            .first_buy_time
            .signed_duration_since(timestamp)
            .num_minutes();
        let near_entry = (0..=5).contains(&minutes_to_entry);
        let strong = sample_group == "profit" && near_entry;
        let weak = sample_group == "loss" && near_entry;
        let buy_volume = if strong {
            2.0 + minutes_to_entry.max(0) as f64 * 0.8
        } else if weak {
            0.6 + (index % 3) as f64 * 0.2
        } else {
            0.3 + (index % 4) as f64 * 0.12
        };
        let sell_volume = if strong { 0.35 } else if weak { 0.8 } else { 0.25 };
        let large_buy_count = if buy_volume >= large_trade_sol_threshold { 1 } else { 0 };
        buckets.push(ActivityBucket {
            mint: position.mint.clone(),
            timestamp,
            buy_count: if strong { 12 } else if weak { 4 } else { 2 },
            sell_count: if strong { 3 } else if weak { 5 } else { 2 },
            buy_volume_sol: buy_volume,
            sell_volume_sol: sell_volume,
            unique_buyers: if strong { 10 } else if weak { 3 } else { 2 },
            unique_sellers: if strong { 3 } else if weak { 4 } else { 2 },
            large_buy_count,
            large_sell_count: 0,
            large_buy_volume_sol: if large_buy_count > 0 { buy_volume * 0.55 } else { 0.0 },
            large_sell_volume_sol: 0.0,
            top_buyer_volume_share: if strong { 0.34 } else if weak { 0.72 } else { 0.45 },
            new_wallet_buy_count: if strong { 4 } else if weak { 1 } else { 0 },
            smart_wallet_buy_count: if strong && index % 2 == 0 { 1 } else { 0 },
            source: "sample".to_string(),
        });
        timestamp += Duration::minutes(1);
        index += 1;
    }
    buckets
}

fn write_positions_csv(path: &Path, positions: &[Position]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from("wallet,mint,first_buy_tx,first_buy_time,last_sell_time,buy_count,sell_count,total_buy_sol,total_sell_sol,realized_pnl_sol,realized_roi,holding_seconds\n");
    for position in positions {
        push_csv_row(&mut csv, &[
            &position.wallet,
            &position.mint,
            &position.first_buy_tx,
            &position.first_buy_time.to_rfc3339(),
            &position.last_sell_time.to_rfc3339(),
            &position.buy_count.to_string(),
            &position.sell_count.to_string(),
            &fmt_f64(Some(position.total_buy_sol)),
            &fmt_f64(Some(position.total_sell_sol)),
            &fmt_f64(Some(position.realized_pnl_sol)),
            &fmt_f64(Some(position.realized_roi)),
            &position.holding_seconds.to_string(),
        ]);
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_selected_positions_csv(path: &Path, positions: &[SelectedPosition]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from("wallet,mint,sample_group,first_buy_tx,first_buy_time,last_sell_time,realized_pnl_sol,realized_roi,holding_seconds,activity_file,exclude_reason\n");
    for selected in positions {
        push_csv_row(&mut csv, &[
            &selected.position.wallet,
            &selected.position.mint,
            &selected.sample_group,
            &selected.position.first_buy_tx,
            &selected.position.first_buy_time.to_rfc3339(),
            &selected.position.last_sell_time.to_rfc3339(),
            &fmt_f64(Some(selected.position.realized_pnl_sol)),
            &fmt_f64(Some(selected.position.realized_roi)),
            &selected.position.holding_seconds.to_string(),
            &selected.activity_file,
            &selected.exclude_reason,
        ]);
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_activity_features_csv(path: &Path, rows: &[ActivityFeature]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from("wallet,mint,sample_group,first_buy_time,last_sell_time,realized_pnl_sol,realized_roi,holding_seconds,pre_1m_net_buy_volume_sol,pre_3m_net_buy_volume_sol,pre_5m_net_buy_volume_sol,pre_10m_net_buy_volume_sol,pre_5m_buy_sell_ratio,pre_5m_unique_buyers,pre_5m_unique_buyers_growth,pre_5m_large_buy_count,pre_5m_large_buy_volume_sol,pre_5m_large_buy_share,pre_5m_top_buyer_volume_share,pre_5m_smart_wallet_buy_count,pre_5m_new_wallet_buy_count,activity_acceleration_1m_vs_5m,holding_net_buy_volume_sol,holding_buy_sell_ratio,pre_exit_5m_sell_pressure,post_exit_20m_net_buy_volume_sol,activity_label,activity_file\n");
    for row in rows {
        push_csv_row(&mut csv, &[
            &row.wallet,
            &row.mint,
            &row.sample_group,
            &row.first_buy_time.to_rfc3339(),
            &row.last_sell_time.to_rfc3339(),
            &fmt_f64(Some(row.realized_pnl_sol)),
            &fmt_f64(Some(row.realized_roi)),
            &row.holding_seconds.to_string(),
            &fmt_f64(row.pre_1m_net_buy_volume_sol),
            &fmt_f64(row.pre_3m_net_buy_volume_sol),
            &fmt_f64(row.pre_5m_net_buy_volume_sol),
            &fmt_f64(row.pre_10m_net_buy_volume_sol),
            &fmt_f64(row.pre_5m_buy_sell_ratio),
            &fmt_f64(row.pre_5m_unique_buyers),
            &fmt_f64(row.pre_5m_unique_buyers_growth),
            &fmt_f64(row.pre_5m_large_buy_count),
            &fmt_f64(row.pre_5m_large_buy_volume_sol),
            &fmt_f64(row.pre_5m_large_buy_share),
            &fmt_f64(row.pre_5m_top_buyer_volume_share),
            &fmt_f64(row.pre_5m_smart_wallet_buy_count),
            &fmt_f64(row.pre_5m_new_wallet_buy_count),
            &fmt_f64(row.activity_acceleration_1m_vs_5m),
            &fmt_f64(row.holding_net_buy_volume_sol),
            &fmt_f64(row.holding_buy_sell_ratio),
            &fmt_f64(row.pre_exit_5m_sell_pressure),
            &fmt_f64(row.post_exit_20m_net_buy_volume_sol),
            &row.activity_label,
            &row.activity_file,
        ]);
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_dataset_summary(
    path: &Path,
    wallet: &str,
    wallet_trades_source: &str,
    activity_source: &str,
    positions: &[Position],
    selected: &[SelectedPosition],
) -> Result<(), Box<dyn Error>> {
    let content = format!(
        "# GMGN Reverse Dataset Summary\n\nwallet: `{wallet}`\n\n## Sources\n\n- wallet trades source: `{wallet_trades_source}`\n- activity source: `{activity_source}`\n- Bitquery API: not used\n\n## Counts\n\n- positions: {}\n- profitable positions: {}\n- losing positions: {}\n- selected positions: {}\n- activity files saved: {}\n- activity skipped: {}\n\n## Files\n\n- `positions.csv`\n- `selected_positions.csv`\n- `activity/`\n- `features/`\n- `reports/`\n",
        positions.len(),
        positions.iter().filter(|position| position.realized_pnl_sol > 0.0).count(),
        positions.iter().filter(|position| position.realized_pnl_sol < 0.0).count(),
        selected.len(),
        selected.iter().filter(|position| !position.activity_file.is_empty()).count(),
        selected.iter().filter(|position| position.activity_file.is_empty()).count(),
    );
    fs::write(path, content)?;
    Ok(())
}

fn write_activity_feature_comparison(
    path: &Path,
    rows: &[ActivityFeature],
    skipped: usize,
) -> Result<(), Box<dyn Error>> {
    let profit = summarize_features(rows, "profit");
    let loss = summarize_features(rows, "loss");
    let content = format!(
        "# GMGN Activity Feature Comparison\n\npositions analyzed: {}\nskipped: {skipped}\n\n## Profit Group\n\n{}\n\n## Loss Group\n\n{}\n\n## Interpretation\n\n入场侧只能使用 `pre_*` 字段和 `activity_acceleration_1m_vs_5m`。`holding_*`、`pre_exit_*`、`post_exit_*` 只用于解释持仓和卖出行为。\n",
        rows.len(),
        profit,
        loss
    );
    fs::write(path, content)?;
    Ok(())
}

fn summarize_features(rows: &[ActivityFeature], group: &str) -> String {
    let group_rows: Vec<&ActivityFeature> = rows.iter().filter(|row| row.sample_group == group).collect();
    if group_rows.is_empty() {
        return "- count: 0".to_string();
    }
    format!(
        "- count: {}\n- avg pre_5m_net_buy_volume_sol: {}\n- avg pre_5m_buy_sell_ratio: {}\n- avg pre_5m_unique_buyers: {}\n- avg pre_5m_large_buy_count: {}\n- avg pre_5m_large_buy_share: {}\n- avg pre_5m_top_buyer_volume_share: {}\n- avg activity_acceleration_1m_vs_5m: {}",
        group_rows.len(),
        fmt_f64(avg_opt(&group_rows, |row| row.pre_5m_net_buy_volume_sol)),
        fmt_f64(avg_opt(&group_rows, |row| row.pre_5m_buy_sell_ratio)),
        fmt_f64(avg_opt(&group_rows, |row| row.pre_5m_unique_buyers)),
        fmt_f64(avg_opt(&group_rows, |row| row.pre_5m_large_buy_count)),
        fmt_f64(avg_opt(&group_rows, |row| row.pre_5m_large_buy_share)),
        fmt_f64(avg_opt(&group_rows, |row| row.pre_5m_top_buyer_volume_share)),
        fmt_f64(avg_opt(&group_rows, |row| row.activity_acceleration_1m_vs_5m)),
    )
}

fn write_rule_candidates_csv(path: &Path, rows: &[RuleEvaluation]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from("expression,matched,profit_count,loss_count,win_rate,lift_vs_baseline,avg_roi,avg_pnl_sol,avg_holding_seconds\n");
    for row in rows {
        push_csv_row(&mut csv, &[
            &row.expression,
            &row.matched.to_string(),
            &row.profit_count.to_string(),
            &row.loss_count.to_string(),
            &fmt_f64(Some(row.win_rate)),
            &fmt_f64(Some(row.lift_vs_baseline)),
            &fmt_f64(Some(row.avg_roi)),
            &fmt_f64(Some(row.avg_pnl_sol)),
            &fmt_f64(Some(row.avg_holding_seconds)),
        ]);
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_rule_candidates_md(
    path: &Path,
    features: &[ActivityFeature],
    baseline: f64,
    min_matches: usize,
    top: usize,
    rows: &[RuleEvaluation],
) -> Result<(), Box<dyn Error>> {
    let mut content = format!(
        "# GMGN Activity Rule Candidates\n\npositions: {}\nbaseline win rate: {:.2}%\nmin matches: {min_matches}\n\n",
        features.len(),
        baseline * 100.0
    );
    content.push_str("| rank | rule | matches | profit/loss | win rate | avg ROI | avg PnL SOL |\n");
    content.push_str("|---:|---|---:|---:|---:|---:|---:|\n");
    for (index, row) in rows.iter().take(top).enumerate() {
        content.push_str(&format!(
            "| {} | `{}` | {} | {}/{} | {:.2}% | {:.2}% | {:.4} |\n",
            index + 1,
            row.expression,
            row.matched,
            row.profit_count,
            row.loss_count,
            row.win_rate * 100.0,
            row.avg_roi * 100.0,
            row.avg_pnl_sol,
        ));
    }
    fs::write(path, content)?;
    Ok(())
}

fn write_final_report(
    path: &Path,
    features: &[ActivityFeature],
    baseline: f64,
    rows: &[RuleEvaluation],
    top: usize,
) -> Result<(), Box<dyn Error>> {
    let best = rows.first();
    let rating = best.map(|row| {
        if row.matched >= 20 && row.win_rate >= baseline + 0.20 {
            "A"
        } else if row.matched >= 10 && row.win_rate >= baseline + 0.10 {
            "B"
        } else if row.matched >= 6 && row.win_rate > baseline {
            "C"
        } else {
            "D"
        }
    }).unwrap_or("D");
    let mut content = format!(
        "# GMGN 钱包交易活动逆向分析报告\n\n## 数据集概览\n\n- positions analyzed: {}\n- baseline win rate: {:.2}%\n- Bitquery API: not used\n- reproducibility rating: `{rating}`\n\n## 核心候选规则\n\n",
        features.len(),
        baseline * 100.0
    );
    for (index, row) in rows.iter().take(top.min(5)).enumerate() {
        content.push_str(&format!(
            "{}. `{}`：matches {}, win rate {:.2}%, avg ROI {:.2}%\n",
            index + 1,
            row.expression,
            row.matched,
            row.win_rate * 100.0,
            row.avg_roi * 100.0,
        ));
    }
    content.push_str("\n## 说明\n\n这些规则只使用买入前可见的 GMGN 交易活动字段。持仓和卖出字段用于解释钱包行为，暂不作为实时入场条件。\n");
    fs::write(path, content)?;
    Ok(())
}

fn read_selected_positions(path: &Path) -> Result<Vec<SelectedPosition>, Box<dyn Error>> {
    let rows = read_csv_rows(path)?;
    let mut positions = Vec::new();
    for row in rows {
        let Some(first_buy_time) = row.get("first_buy_time").and_then(|value| parse_time(value)) else {
            continue;
        };
        let Some(last_sell_time) = row.get("last_sell_time").and_then(|value| parse_time(value)) else {
            continue;
        };
        positions.push(SelectedPosition {
            position: Position {
                wallet: row.get("wallet").cloned().unwrap_or_default(),
                mint: row.get("mint").cloned().unwrap_or_default(),
                first_buy_tx: row.get("first_buy_tx").cloned().unwrap_or_default(),
                first_buy_time,
                last_sell_time,
                buy_count: 0,
                sell_count: 0,
                total_buy_sol: 0.0,
                total_sell_sol: 0.0,
                realized_pnl_sol: csv_f64(&row, "realized_pnl_sol"),
                realized_roi: csv_f64(&row, "realized_roi"),
                holding_seconds: row.get("holding_seconds").and_then(|v| v.parse().ok()).unwrap_or(0),
            },
            sample_group: row.get("sample_group").cloned().unwrap_or_default(),
            activity_file: row.get("activity_file").cloned().unwrap_or_default(),
            exclude_reason: row.get("exclude_reason").cloned().unwrap_or_default(),
        });
    }
    Ok(positions)
}

fn read_activity_features_csv(path: &Path) -> Result<Vec<ActivityFeature>, Box<dyn Error>> {
    let rows = read_csv_rows(path)?;
    let mut features = Vec::new();
    for row in rows {
        let Some(first_buy_time) = row.get("first_buy_time").and_then(|value| parse_time(value)) else {
            continue;
        };
        let Some(last_sell_time) = row.get("last_sell_time").and_then(|value| parse_time(value)) else {
            continue;
        };
        features.push(ActivityFeature {
            wallet: row.get("wallet").cloned().unwrap_or_default(),
            mint: row.get("mint").cloned().unwrap_or_default(),
            sample_group: row.get("sample_group").cloned().unwrap_or_default(),
            first_buy_time,
            last_sell_time,
            realized_pnl_sol: csv_f64(&row, "realized_pnl_sol"),
            realized_roi: csv_f64(&row, "realized_roi"),
            holding_seconds: row.get("holding_seconds").and_then(|v| v.parse().ok()).unwrap_or(0),
            pre_1m_net_buy_volume_sol: csv_opt_f64(&row, "pre_1m_net_buy_volume_sol"),
            pre_3m_net_buy_volume_sol: csv_opt_f64(&row, "pre_3m_net_buy_volume_sol"),
            pre_5m_net_buy_volume_sol: csv_opt_f64(&row, "pre_5m_net_buy_volume_sol"),
            pre_10m_net_buy_volume_sol: csv_opt_f64(&row, "pre_10m_net_buy_volume_sol"),
            pre_5m_buy_sell_ratio: csv_opt_f64(&row, "pre_5m_buy_sell_ratio"),
            pre_5m_unique_buyers: csv_opt_f64(&row, "pre_5m_unique_buyers"),
            pre_5m_unique_buyers_growth: csv_opt_f64(&row, "pre_5m_unique_buyers_growth"),
            pre_5m_large_buy_count: csv_opt_f64(&row, "pre_5m_large_buy_count"),
            pre_5m_large_buy_volume_sol: csv_opt_f64(&row, "pre_5m_large_buy_volume_sol"),
            pre_5m_large_buy_share: csv_opt_f64(&row, "pre_5m_large_buy_share"),
            pre_5m_top_buyer_volume_share: csv_opt_f64(&row, "pre_5m_top_buyer_volume_share"),
            pre_5m_smart_wallet_buy_count: csv_opt_f64(&row, "pre_5m_smart_wallet_buy_count"),
            pre_5m_new_wallet_buy_count: csv_opt_f64(&row, "pre_5m_new_wallet_buy_count"),
            activity_acceleration_1m_vs_5m: csv_opt_f64(&row, "activity_acceleration_1m_vs_5m"),
            holding_net_buy_volume_sol: csv_opt_f64(&row, "holding_net_buy_volume_sol"),
            holding_buy_sell_ratio: csv_opt_f64(&row, "holding_buy_sell_ratio"),
            pre_exit_5m_sell_pressure: csv_opt_f64(&row, "pre_exit_5m_sell_pressure"),
            post_exit_20m_net_buy_volume_sol: csv_opt_f64(&row, "post_exit_20m_net_buy_volume_sol"),
            activity_label: row.get("activity_label").cloned().unwrap_or_default(),
            activity_file: row.get("activity_file").cloned().unwrap_or_default(),
        });
    }
    Ok(features)
}

fn read_csv_rows(path: &Path) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let mut lines = content.lines();
    let header = lines.next().ok_or_else(|| format!("CSV is empty: {}", path.display()))?;
    let columns: Vec<String> = parse_csv_line(header);
    let mut rows = Vec::new();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let values = parse_csv_line(line);
        let mut row = HashMap::new();
        for (index, column) in columns.iter().enumerate() {
            row.insert(column.clone(), values.get(index).cloned().unwrap_or_default());
        }
        rows.push(row);
    }
    Ok(rows)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                values.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    values.push(current.trim().to_string());
    values
}

fn push_csv_row(output: &mut String, values: &[&str]) {
    output.push_str(
        &values
            .iter()
            .map(|value| csv_escape(value))
            .collect::<Vec<String>>()
            .join(","),
    );
    output.push('\n');
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn json_rows(value: &Value) -> Option<&Vec<Value>> {
    value
        .as_array()
        .or_else(|| value.get("data").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/list").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/history").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/rows").and_then(Value::as_array))
        .or_else(|| value.get("rows").and_then(Value::as_array))
        .or_else(|| value.get("result").and_then(Value::as_array))
}

fn string_at<'a>(value: &'a Value, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| value.get(*name).and_then(Value::as_str))
}

fn number_at(value: &Value, names: &[&str]) -> Option<f64> {
    names.iter().find_map(|name| {
        value
            .get(*name)
            .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
    })
}

fn unix_at(value: &Value, names: &[&str]) -> Option<DateTime<Utc>> {
    names.iter().find_map(|name| {
        let raw = value.get(*name)?.as_i64().or_else(|| value.get(*name)?.as_str()?.parse().ok())?;
        DateTime::from_timestamp(if raw > 10_000_000_000 { raw / 1000 } else { raw }, 0)
    })
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

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|time| time.and_utc())
        })
}

fn csv_f64(row: &HashMap<String, String>, name: &str) -> f64 {
    row.get(name).and_then(|value| value.parse().ok()).unwrap_or(0.0)
}

fn csv_opt_f64(row: &HashMap<String, String>, name: &str) -> Option<f64> {
    row.get(name)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse().ok())
}

fn csv_usize(row: &HashMap<String, String>, name: &str) -> usize {
    row.get(name).and_then(|value| value.parse().ok()).unwrap_or(0)
}

fn ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if denominator.abs() > f64::EPSILON {
        Some(numerator / denominator)
    } else if numerator > 0.0 {
        Some(numerator)
    } else {
        None
    }
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() > f64::EPSILON {
        numerator / denominator
    } else {
        0.0
    }
}

fn avg_opt<F>(rows: &[&ActivityFeature], getter: F) -> Option<f64>
where
    F: Fn(&ActivityFeature) -> Option<f64>,
{
    let values: Vec<f64> = rows.iter().filter_map(|row| getter(row)).collect();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn fmt_f64(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.6}"))
        .unwrap_or_default()
}

fn classify_activity(net_buy: f64, buy_sell_ratio: Option<f64>, large_buy_count: usize) -> String {
    if net_buy > 5.0 && buy_sell_ratio.unwrap_or(0.0) >= 2.0 && large_buy_count > 0 {
        "strong_accumulation".to_string()
    } else if net_buy > 0.0 && buy_sell_ratio.unwrap_or(0.0) >= 1.2 {
        "positive_flow".to_string()
    } else if net_buy < 0.0 {
        "sell_pressure".to_string()
    } else {
        "mixed_activity".to_string()
    }
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

fn activity_file_name(mint: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> String {
    format!(
        "{}__1m__{}__{}.json",
        sanitize_path_component(mint),
        from.timestamp(),
        to.timestamp()
    )
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn resolve_dataset_path(dataset_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        dataset_dir.join(path)
    }
}

fn print_help() {
    println!(
        r#"GMGN wallet reverse analysis without Bitquery

Commands:
  cargo run -- gmgn-reverse all <wallet>
  cargo run -- gmgn-reverse build <wallet> [options]
  cargo run -- gmgn-reverse extract --dataset <path>
  cargo run -- gmgn-reverse rules --dataset <path>

Build options:
  --wallet-trades-source sample|csv|gmgn-json   default: sample
  --wallet-trades-file <path>                   required for csv/gmgn-json
  --activity-source sample|csv-dir|json-dir|none default: sample
  --activity-dir <path>                         required for csv-dir/json-dir
  --out <path>                                  default: data/gmgn_reverse/wallets/<wallet>
  --days <n>                                    default: 30
  --profit-samples <n>                          default: 50
  --loss-samples <n>                            default: 50
  --pre-minutes <n>                             default: 60
  --post-minutes <n>                            default: 60
  --large-trade-sol-threshold <n>               default: 2.0

CSV wallet trades columns:
  timestamp,tx_hash,wallet,mint,side,sol_amount,token_amount,price_sol

Activity CSV columns:
  timestamp,mint,buy_count,sell_count,buy_volume_sol,sell_volume_sol,unique_buyers,unique_sellers,large_buy_count,large_sell_count,large_buy_volume_sol,large_sell_volume_sol,top_buyer_volume_share,new_wallet_buy_count,smart_wallet_buy_count
"#
    );
}
