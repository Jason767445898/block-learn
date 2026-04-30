use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
struct SelectedPosition {
    wallet: String,
    mint: String,
    sample_group: String,
    first_buy_tx: String,
    first_buy_time: DateTime<Utc>,
    last_sell_time: DateTime<Utc>,
    realized_pnl_sol: f64,
    realized_roi: f64,
    holding_seconds: i64,
    kline_file: String,
    exclude_reason: String,
}

#[derive(Debug, Clone)]
struct PositionDetails {
    buy_count: usize,
    sell_count: usize,
    total_buy_sol: f64,
    total_sell_sol: f64,
    average_buy_price_sol: f64,
}

#[derive(Debug, Clone)]
struct Candle {
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Debug)]
struct FeatureRow {
    position: SelectedPosition,
    details: Option<PositionDetails>,
    entry: EntryFeatures,
    holding: HoldingFeatures,
    exit: ExitFeatures,
}

#[derive(Debug)]
struct EntryFeatures {
    pre_1m_return: Option<f64>,
    pre_3m_return: Option<f64>,
    pre_5m_return: Option<f64>,
    pre_10m_return: Option<f64>,
    pre_20m_return: Option<f64>,
    pre_5m_volume_spike: Option<f64>,
    pre_20m_volume_spike: Option<f64>,
    entry_range_position: Option<f64>,
    break_previous_high: bool,
    distance_to_20m_high: Option<f64>,
    distance_to_20m_low: Option<f64>,
    consecutive_green_candles: usize,
    consecutive_red_candles: usize,
    volatility_20m: Option<f64>,
    label: String,
}

#[derive(Debug)]
struct HoldingFeatures {
    max_runup_during_holding: Option<f64>,
    max_drawdown_during_holding: Option<f64>,
    time_to_max_runup: Option<i64>,
    time_to_max_drawdown: Option<i64>,
    highest_price_before_exit: Option<f64>,
    lowest_price_before_exit: Option<f64>,
    holding_return_path: Option<f64>,
    add_count: usize,
    avg_add_interval_seconds: Option<f64>,
    largest_add_size_sol: Option<f64>,
    label: String,
}

#[derive(Debug)]
struct ExitFeatures {
    pre_exit_1m_return: Option<f64>,
    pre_exit_3m_return: Option<f64>,
    pre_exit_5m_return: Option<f64>,
    exit_range_position: Option<f64>,
    exit_efficiency: Option<f64>,
    drawdown_from_peak_before_exit: Option<f64>,
    sell_after_breakdown: bool,
    post_exit_5m_return: Option<f64>,
    post_exit_20m_return: Option<f64>,
    missed_profit_after_exit: Option<f64>,
    loss_avoided_after_exit: Option<f64>,
    label: String,
}

#[derive(Debug, Default)]
struct GroupSummary {
    count: usize,
    avg_pre_5m_return: Option<f64>,
    avg_pre_20m_return: Option<f64>,
    avg_pre_5m_volume_spike: Option<f64>,
    avg_entry_range_position: Option<f64>,
    avg_max_runup: Option<f64>,
    avg_max_drawdown: Option<f64>,
    avg_holding_seconds: Option<f64>,
    avg_exit_efficiency: Option<f64>,
    avg_post_exit_20m_return: Option<f64>,
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return Ok(());
    }

    let dataset_dir = get_arg(args, "--dataset")
        .or_else(|| args.first().cloned())
        .ok_or("--dataset <path> is required")?;
    let dataset_dir = PathBuf::from(dataset_dir);
    let selected_path = dataset_dir.join("selected_positions.csv");
    let positions_path = dataset_dir.join("positions.csv");
    let feature_dir = dataset_dir.join("features");
    let report_dir = dataset_dir.join("reports");

    fs::create_dir_all(&feature_dir)?;
    fs::create_dir_all(&report_dir)?;

    let selected = read_selected_positions(&selected_path)?;
    let details = if positions_path.exists() {
        read_position_details(&positions_path)?
    } else {
        HashMap::new()
    };

    let mut rows = Vec::new();
    let mut skipped = 0usize;

    for position in selected {
        if !position.exclude_reason.is_empty() || position.kline_file.is_empty() {
            skipped += 1;
            continue;
        }

        let kline_path = resolve_dataset_path(&dataset_dir, &position.kline_file);
        let candles = read_kline_file(&kline_path)?;
        if candles.is_empty() {
            skipped += 1;
            continue;
        }

        let entry = build_entry_features(&position, &candles);
        let holding = build_holding_features(&position, &candles, details.get(&position.mint));
        let exit = build_exit_features(&position, &candles);

        rows.push(FeatureRow {
            details: details.get(&position.mint).cloned(),
            position,
            entry,
            holding,
            exit,
        });
    }

    write_entry_features(&feature_dir.join("entry_features.csv"), &rows)?;
    write_holding_features(&feature_dir.join("holding_features.csv"), &rows)?;
    write_exit_features(&feature_dir.join("exit_features.csv"), &rows)?;
    write_position_features(&feature_dir.join("position_features.csv"), &rows)?;
    write_feature_comparison(&report_dir.join("feature_comparison.md"), &rows, skipped)?;

    let profit_count = rows
        .iter()
        .filter(|row| row.position.sample_group == "profit")
        .count();
    let loss_count = rows
        .iter()
        .filter(|row| row.position.sample_group == "loss")
        .count();

    println!("strategy feature extraction MVP");
    println!("dataset: {}", dataset_dir.display());
    println!("positions analyzed: {}", rows.len());
    println!("samples: {} profit / {} loss", profit_count, loss_count);
    println!("skipped: {skipped}");
    println!();
    println!("written");
    println!("- {}", feature_dir.join("entry_features.csv").display());
    println!("- {}", feature_dir.join("holding_features.csv").display());
    println!("- {}", feature_dir.join("exit_features.csv").display());
    println!("- {}", feature_dir.join("position_features.csv").display());
    println!("- {}", report_dir.join("feature_comparison.md").display());

    Ok(())
}

fn build_entry_features(position: &SelectedPosition, candles: &[Candle]) -> EntryFeatures {
    let Some(entry_index) = candle_index_at_or_before(candles, position.first_buy_time) else {
        return empty_entry_features("no_entry_candle");
    };
    let entry = &candles[entry_index];
    let pre_start = entry_index.saturating_sub(20);
    let history = &candles[pre_start..=entry_index];
    let previous = if entry_index > pre_start {
        &candles[pre_start..entry_index]
    } else {
        &candles[pre_start..pre_start]
    };

    let pre_1m_return = return_before(candles, entry_index, 1);
    let pre_3m_return = return_before(candles, entry_index, 3);
    let pre_5m_return = return_before(candles, entry_index, 5);
    let pre_10m_return = return_before(candles, entry_index, 10);
    let pre_20m_return = return_before(candles, entry_index, 20);
    let pre_5m_volume_spike = volume_spike(candles, entry_index, 5);
    let pre_20m_volume_spike = volume_spike(candles, entry_index, 20);
    let low_20m = history
        .iter()
        .map(|candle| candle.low)
        .fold(f64::INFINITY, f64::min);
    let high_20m = history.iter().map(|candle| candle.high).fold(0.0, f64::max);
    let previous_high = previous
        .iter()
        .map(|candle| candle.high)
        .fold(0.0, f64::max);
    let entry_range_position = range_position(entry.close, low_20m, high_20m);
    let break_previous_high = previous_high > 0.0 && entry.high > previous_high;
    let distance_to_20m_high = if high_20m > 0.0 {
        Some(safe_div(entry.close - high_20m, high_20m))
    } else {
        None
    };
    let distance_to_20m_low = if low_20m.is_finite() && low_20m > 0.0 {
        Some(safe_div(entry.close - low_20m, low_20m))
    } else {
        None
    };
    let consecutive_green_candles = consecutive_candles_before(candles, entry_index, true);
    let consecutive_red_candles = consecutive_candles_before(candles, entry_index, false);
    let volatility_20m = if low_20m.is_finite() && low_20m > 0.0 {
        Some(safe_div(high_20m - low_20m, low_20m))
    } else {
        None
    };

    let label = classify_entry(
        pre_5m_return,
        pre_20m_return,
        pre_5m_volume_spike,
        entry_range_position,
        break_previous_high,
        consecutive_red_candles,
    );

    EntryFeatures {
        pre_1m_return,
        pre_3m_return,
        pre_5m_return,
        pre_10m_return,
        pre_20m_return,
        pre_5m_volume_spike,
        pre_20m_volume_spike,
        entry_range_position,
        break_previous_high,
        distance_to_20m_high,
        distance_to_20m_low,
        consecutive_green_candles,
        consecutive_red_candles,
        volatility_20m,
        label,
    }
}

fn build_holding_features(
    position: &SelectedPosition,
    candles: &[Candle],
    details: Option<&PositionDetails>,
) -> HoldingFeatures {
    let Some(entry_index) = candle_index_at_or_before(candles, position.first_buy_time) else {
        return empty_holding_features("no_entry_candle");
    };
    let Some(exit_index) = candle_index_at_or_before(candles, position.last_sell_time) else {
        return empty_holding_features("no_exit_candle");
    };
    let entry = &candles[entry_index];
    let holding = &candles[entry_index..=exit_index.max(entry_index)];
    let entry_price = entry.close;
    let highest = holding.iter().map(|candle| candle.high).fold(0.0, f64::max);
    let lowest = holding
        .iter()
        .map(|candle| candle.low)
        .fold(f64::INFINITY, f64::min);
    let max_runup = if entry_price > 0.0 {
        Some(safe_div(highest - entry_price, entry_price))
    } else {
        None
    };
    let max_drawdown = if entry_price > 0.0 && lowest.is_finite() {
        Some(safe_div(lowest - entry_price, entry_price))
    } else {
        None
    };
    let time_to_max_runup = holding
        .iter()
        .max_by(|a, b| a.high.total_cmp(&b.high))
        .map(|candle| {
            candle
                .timestamp
                .signed_duration_since(position.first_buy_time)
                .num_seconds()
        });
    let time_to_max_drawdown =
        holding
            .iter()
            .min_by(|a, b| a.low.total_cmp(&b.low))
            .map(|candle| {
                candle
                    .timestamp
                    .signed_duration_since(position.first_buy_time)
                    .num_seconds()
            });
    let exit_close = candles[exit_index].close;
    let holding_return_path = if entry_price > 0.0 {
        Some(safe_div(exit_close - entry_price, entry_price))
    } else {
        None
    };
    let buy_count = details.map(|details| details.buy_count).unwrap_or(1);
    let add_count = buy_count.saturating_sub(1);
    let avg_add_interval_seconds = if add_count > 0 {
        Some(position.holding_seconds as f64 / add_count as f64)
    } else {
        None
    };
    let largest_add_size_sol = details
        .filter(|details| details.buy_count > 0)
        .map(|details| details.total_buy_sol / details.buy_count as f64);
    let label = classify_holding(max_runup, max_drawdown, add_count, position.holding_seconds);

    HoldingFeatures {
        max_runup_during_holding: max_runup,
        max_drawdown_during_holding: max_drawdown,
        time_to_max_runup,
        time_to_max_drawdown,
        highest_price_before_exit: Some(highest),
        lowest_price_before_exit: if lowest.is_finite() {
            Some(lowest)
        } else {
            None
        },
        holding_return_path,
        add_count,
        avg_add_interval_seconds,
        largest_add_size_sol,
        label,
    }
}

fn build_exit_features(position: &SelectedPosition, candles: &[Candle]) -> ExitFeatures {
    let Some(entry_index) = candle_index_at_or_before(candles, position.first_buy_time) else {
        return empty_exit_features("no_entry_candle");
    };
    let Some(exit_index) = candle_index_at_or_before(candles, position.last_sell_time) else {
        return empty_exit_features("no_exit_candle");
    };
    let exit = &candles[exit_index];
    let pre_start = exit_index.saturating_sub(20);
    let pre_exit = &candles[pre_start..=exit_index];
    let holding = &candles[entry_index..=exit_index.max(entry_index)];
    let post_end = (exit_index + 20).min(candles.len().saturating_sub(1));
    let post_exit = &candles[exit_index..=post_end];

    let pre_exit_1m_return = return_before(candles, exit_index, 1);
    let pre_exit_3m_return = return_before(candles, exit_index, 3);
    let pre_exit_5m_return = return_before(candles, exit_index, 5);
    let pre_low = pre_exit
        .iter()
        .map(|candle| candle.low)
        .fold(f64::INFINITY, f64::min);
    let pre_high = pre_exit
        .iter()
        .map(|candle| candle.high)
        .fold(0.0, f64::max);
    let exit_range_position = range_position(exit.close, pre_low, pre_high);
    let highest = holding.iter().map(|candle| candle.high).fold(0.0, f64::max);
    let exit_efficiency = if highest > 0.0 {
        Some((exit.close / highest).clamp(0.0, 1.0))
    } else {
        None
    };
    let drawdown_from_peak_before_exit = if highest > 0.0 {
        Some(safe_div(exit.close - highest, highest))
    } else {
        None
    };
    let prior_low = if exit_index > pre_start {
        candles[pre_start..exit_index]
            .iter()
            .map(|candle| candle.low)
            .fold(f64::INFINITY, f64::min)
    } else {
        f64::INFINITY
    };
    let sell_after_breakdown = prior_low.is_finite() && exit.close < prior_low;
    let post_exit_5m_return = return_after(candles, exit_index, 5);
    let post_exit_20m_return = return_after(candles, exit_index, 20);
    let post_high = post_exit
        .iter()
        .map(|candle| candle.high)
        .fold(0.0, f64::max);
    let post_low = post_exit
        .iter()
        .map(|candle| candle.low)
        .fold(f64::INFINITY, f64::min);
    let missed_profit_after_exit = if exit.close > 0.0 && post_high > exit.close {
        Some(safe_div(post_high - exit.close, exit.close))
    } else {
        Some(0.0)
    };
    let loss_avoided_after_exit =
        if exit.close > 0.0 && post_low.is_finite() && post_low < exit.close {
            Some(safe_div(exit.close - post_low, exit.close))
        } else {
            Some(0.0)
        };
    let label = classify_exit(
        position.realized_pnl_sol,
        position.holding_seconds,
        exit_efficiency,
        drawdown_from_peak_before_exit,
        post_exit_20m_return,
        sell_after_breakdown,
    );

    ExitFeatures {
        pre_exit_1m_return,
        pre_exit_3m_return,
        pre_exit_5m_return,
        exit_range_position,
        exit_efficiency,
        drawdown_from_peak_before_exit,
        sell_after_breakdown,
        post_exit_5m_return,
        post_exit_20m_return,
        missed_profit_after_exit,
        loss_avoided_after_exit,
        label,
    }
}

fn write_entry_features(path: &Path, rows: &[FeatureRow]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from(
        "wallet,mint,sample_group,first_buy_time,realized_pnl_sol,realized_roi,pre_1m_return,pre_3m_return,pre_5m_return,pre_10m_return,pre_20m_return,pre_5m_volume_spike,pre_20m_volume_spike,entry_range_position,break_previous_high,distance_to_20m_high,distance_to_20m_low,consecutive_green_candles,consecutive_red_candles,volatility_20m,entry_label\n",
    );
    for row in rows {
        push_csv_row(
            &mut csv,
            &[
                &row.position.wallet,
                &row.position.mint,
                &row.position.sample_group,
                &row.position.first_buy_time.to_rfc3339(),
                &fmt_f64(Some(row.position.realized_pnl_sol)),
                &fmt_f64(Some(row.position.realized_roi)),
                &fmt_f64(row.entry.pre_1m_return),
                &fmt_f64(row.entry.pre_3m_return),
                &fmt_f64(row.entry.pre_5m_return),
                &fmt_f64(row.entry.pre_10m_return),
                &fmt_f64(row.entry.pre_20m_return),
                &fmt_f64(row.entry.pre_5m_volume_spike),
                &fmt_f64(row.entry.pre_20m_volume_spike),
                &fmt_f64(row.entry.entry_range_position),
                bool_str(row.entry.break_previous_high),
                &fmt_f64(row.entry.distance_to_20m_high),
                &fmt_f64(row.entry.distance_to_20m_low),
                &row.entry.consecutive_green_candles.to_string(),
                &row.entry.consecutive_red_candles.to_string(),
                &fmt_f64(row.entry.volatility_20m),
                &row.entry.label,
            ],
        );
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_holding_features(path: &Path, rows: &[FeatureRow]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from(
        "wallet,mint,sample_group,first_buy_time,last_sell_time,holding_seconds,buy_count,sell_count,total_buy_sol,total_sell_sol,average_buy_price_sol,max_runup_during_holding,max_drawdown_during_holding,time_to_max_runup,time_to_max_drawdown,highest_price_before_exit,lowest_price_before_exit,holding_return_path,add_count,avg_add_interval_seconds,largest_add_size_sol,holding_label\n",
    );
    for row in rows {
        let details = row.details.as_ref();
        push_csv_row(
            &mut csv,
            &[
                &row.position.wallet,
                &row.position.mint,
                &row.position.sample_group,
                &row.position.first_buy_time.to_rfc3339(),
                &row.position.last_sell_time.to_rfc3339(),
                &row.position.holding_seconds.to_string(),
                &details.map(|v| v.buy_count.to_string()).unwrap_or_default(),
                &details
                    .map(|v| v.sell_count.to_string())
                    .unwrap_or_default(),
                &details
                    .map(|v| fmt_f64(Some(v.total_buy_sol)))
                    .unwrap_or_default(),
                &details
                    .map(|v| fmt_f64(Some(v.total_sell_sol)))
                    .unwrap_or_default(),
                &details
                    .map(|v| fmt_f64(Some(v.average_buy_price_sol)))
                    .unwrap_or_default(),
                &fmt_f64(row.holding.max_runup_during_holding),
                &fmt_f64(row.holding.max_drawdown_during_holding),
                &row.holding
                    .time_to_max_runup
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                &row.holding
                    .time_to_max_drawdown
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                &fmt_f64(row.holding.highest_price_before_exit),
                &fmt_f64(row.holding.lowest_price_before_exit),
                &fmt_f64(row.holding.holding_return_path),
                &row.holding.add_count.to_string(),
                &fmt_f64(row.holding.avg_add_interval_seconds),
                &fmt_f64(row.holding.largest_add_size_sol),
                &row.holding.label,
            ],
        );
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_exit_features(path: &Path, rows: &[FeatureRow]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from(
        "wallet,mint,sample_group,last_sell_time,realized_pnl_sol,realized_roi,pre_exit_1m_return,pre_exit_3m_return,pre_exit_5m_return,exit_range_position,exit_efficiency,drawdown_from_peak_before_exit,sell_after_breakdown,post_exit_5m_return,post_exit_20m_return,missed_profit_after_exit,loss_avoided_after_exit,exit_label\n",
    );
    for row in rows {
        push_csv_row(
            &mut csv,
            &[
                &row.position.wallet,
                &row.position.mint,
                &row.position.sample_group,
                &row.position.last_sell_time.to_rfc3339(),
                &fmt_f64(Some(row.position.realized_pnl_sol)),
                &fmt_f64(Some(row.position.realized_roi)),
                &fmt_f64(row.exit.pre_exit_1m_return),
                &fmt_f64(row.exit.pre_exit_3m_return),
                &fmt_f64(row.exit.pre_exit_5m_return),
                &fmt_f64(row.exit.exit_range_position),
                &fmt_f64(row.exit.exit_efficiency),
                &fmt_f64(row.exit.drawdown_from_peak_before_exit),
                bool_str(row.exit.sell_after_breakdown),
                &fmt_f64(row.exit.post_exit_5m_return),
                &fmt_f64(row.exit.post_exit_20m_return),
                &fmt_f64(row.exit.missed_profit_after_exit),
                &fmt_f64(row.exit.loss_avoided_after_exit),
                &row.exit.label,
            ],
        );
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_position_features(path: &Path, rows: &[FeatureRow]) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from(
        "wallet,mint,sample_group,first_buy_tx,first_buy_time,last_sell_time,realized_pnl_sol,realized_roi,holding_seconds,entry_label,holding_label,exit_label,pre_5m_return,pre_20m_return,pre_5m_volume_spike,entry_range_position,max_runup_during_holding,max_drawdown_during_holding,exit_efficiency,post_exit_20m_return,kline_file\n",
    );
    for row in rows {
        push_csv_row(
            &mut csv,
            &[
                &row.position.wallet,
                &row.position.mint,
                &row.position.sample_group,
                &row.position.first_buy_tx,
                &row.position.first_buy_time.to_rfc3339(),
                &row.position.last_sell_time.to_rfc3339(),
                &fmt_f64(Some(row.position.realized_pnl_sol)),
                &fmt_f64(Some(row.position.realized_roi)),
                &row.position.holding_seconds.to_string(),
                &row.entry.label,
                &row.holding.label,
                &row.exit.label,
                &fmt_f64(row.entry.pre_5m_return),
                &fmt_f64(row.entry.pre_20m_return),
                &fmt_f64(row.entry.pre_5m_volume_spike),
                &fmt_f64(row.entry.entry_range_position),
                &fmt_f64(row.holding.max_runup_during_holding),
                &fmt_f64(row.holding.max_drawdown_during_holding),
                &fmt_f64(row.exit.exit_efficiency),
                &fmt_f64(row.exit.post_exit_20m_return),
                &row.position.kline_file,
            ],
        );
    }
    fs::write(path, csv)?;
    Ok(())
}

fn write_feature_comparison(
    path: &Path,
    rows: &[FeatureRow],
    skipped: usize,
) -> Result<(), Box<dyn Error>> {
    let profit = summarize_group(rows, "profit");
    let loss = summarize_group(rows, "loss");
    let content = format!(
        r#"# Strategy Feature Comparison

## Dataset

- analyzed positions: {}
- skipped positions: {skipped}
- profit samples analyzed: {}
- loss samples analyzed: {}

## Group Averages

| metric | profit | loss |
|---|---:|---:|
| pre 5m return | {} | {} |
| pre 20m return | {} | {} |
| pre 5m volume spike | {} | {} |
| entry range position | {} | {} |
| max runup during holding | {} | {} |
| max drawdown during holding | {} | {} |
| holding seconds | {} | {} |
| exit efficiency | {} | {} |
| post exit 20m return | {} | {} |

## Reading Notes

- `entry_range_position` close to `1.0` means the wallet entered near the local 20m high.
- `volume_spike` above `2.0` means the entry candle volume was more than twice the recent average.
- `max_runup_during_holding` shows whether the trade quickly had enough upside to justify holding.
- `max_drawdown_during_holding` shows how much adverse movement the wallet tolerated.
- `exit_efficiency` close to `1.0` means it sold near the local high during its holding window.

This report is descriptive. Candidate trading rules should only be created after checking the CSV rows and testing them out of sample.
"#,
        rows.len(),
        profit.count,
        loss.count,
        fmt_pct(profit.avg_pre_5m_return),
        fmt_pct(loss.avg_pre_5m_return),
        fmt_pct(profit.avg_pre_20m_return),
        fmt_pct(loss.avg_pre_20m_return),
        fmt_ratio(profit.avg_pre_5m_volume_spike),
        fmt_ratio(loss.avg_pre_5m_volume_spike),
        fmt_plain(profit.avg_entry_range_position),
        fmt_plain(loss.avg_entry_range_position),
        fmt_pct(profit.avg_max_runup),
        fmt_pct(loss.avg_max_runup),
        fmt_pct(profit.avg_max_drawdown),
        fmt_pct(loss.avg_max_drawdown),
        fmt_plain(profit.avg_holding_seconds),
        fmt_plain(loss.avg_holding_seconds),
        fmt_plain(profit.avg_exit_efficiency),
        fmt_plain(loss.avg_exit_efficiency),
        fmt_pct(profit.avg_post_exit_20m_return),
        fmt_pct(loss.avg_post_exit_20m_return),
    );

    fs::write(path, content)?;
    Ok(())
}

fn summarize_group(rows: &[FeatureRow], group: &str) -> GroupSummary {
    let group_rows: Vec<&FeatureRow> = rows
        .iter()
        .filter(|row| row.position.sample_group == group)
        .collect();
    GroupSummary {
        count: group_rows.len(),
        avg_pre_5m_return: avg(group_rows.iter().map(|row| row.entry.pre_5m_return)),
        avg_pre_20m_return: avg(group_rows.iter().map(|row| row.entry.pre_20m_return)),
        avg_pre_5m_volume_spike: avg(group_rows.iter().map(|row| row.entry.pre_5m_volume_spike)),
        avg_entry_range_position: avg(group_rows.iter().map(|row| row.entry.entry_range_position)),
        avg_max_runup: avg(group_rows
            .iter()
            .map(|row| row.holding.max_runup_during_holding)),
        avg_max_drawdown: avg(group_rows
            .iter()
            .map(|row| row.holding.max_drawdown_during_holding)),
        avg_holding_seconds: avg(group_rows
            .iter()
            .map(|row| Some(row.position.holding_seconds as f64))),
        avg_exit_efficiency: avg(group_rows.iter().map(|row| row.exit.exit_efficiency)),
        avg_post_exit_20m_return: avg(group_rows.iter().map(|row| row.exit.post_exit_20m_return)),
    }
}

fn read_selected_positions(path: &Path) -> Result<Vec<SelectedPosition>, Box<dyn Error>> {
    let rows = read_csv_rows(path)?;
    let mut positions = Vec::new();

    for row in rows {
        let first_buy_time = parse_time(required(&row, "first_buy_time")?)?;
        let last_sell_time = parse_time(required(&row, "last_sell_time")?)?;
        positions.push(SelectedPosition {
            wallet: required(&row, "wallet")?.to_string(),
            mint: required(&row, "mint")?.to_string(),
            sample_group: required(&row, "sample_group")?.to_string(),
            first_buy_tx: required(&row, "first_buy_tx")?.to_string(),
            first_buy_time,
            last_sell_time,
            realized_pnl_sol: parse_f64(required(&row, "realized_pnl_sol")?),
            realized_roi: parse_f64(required(&row, "realized_roi")?),
            holding_seconds: required(&row, "holding_seconds")?.parse().unwrap_or(0),
            kline_file: row.get("kline_file").cloned().unwrap_or_default(),
            exclude_reason: row.get("exclude_reason").cloned().unwrap_or_default(),
        });
    }

    Ok(positions)
}

fn read_position_details(path: &Path) -> Result<HashMap<String, PositionDetails>, Box<dyn Error>> {
    let rows = read_csv_rows(path)?;
    let mut details = HashMap::new();

    for row in rows {
        let mint = required(&row, "mint")?.to_string();
        details.insert(
            mint,
            PositionDetails {
                buy_count: row
                    .get("buy_count")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                sell_count: row
                    .get("sell_count")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                total_buy_sol: parse_f64(
                    row.get("total_buy_sol").map(String::as_str).unwrap_or(""),
                ),
                total_sell_sol: parse_f64(
                    row.get("total_sell_sol").map(String::as_str).unwrap_or(""),
                ),
                average_buy_price_sol: parse_f64(
                    row.get("average_buy_price_sol")
                        .map(String::as_str)
                        .unwrap_or(""),
                ),
            },
        );
    }

    Ok(details)
}

fn read_kline_file(path: &Path) -> Result<Vec<Candle>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&content)?;
    let rows = value
        .as_array()
        .or_else(|| value.get("list").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/list").and_then(Value::as_array))
        .or_else(|| value.get("data").and_then(Value::as_array))
        .ok_or_else(|| format!("kline JSON has no list/data rows: {}", path.display()))?;

    let mut candles: Vec<Candle> = rows.iter().filter_map(parse_kline_row).collect();
    candles.sort_by_key(|candle| candle.timestamp);
    Ok(candles)
}

fn parse_kline_row(row: &Value) -> Option<Candle> {
    Some(Candle {
        timestamp: row_timestamp(row)?,
        open: row_number(row, &["open", "o"])?,
        high: row_number(row, &["high", "h"])?,
        low: row_number(row, &["low", "l"])?,
        close: row_number(row, &["close", "c"])?,
        volume: row_number(row, &["volume", "vol", "v"]).unwrap_or(0.0),
    })
}

fn read_csv_rows(path: &Path) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let mut lines = content.lines();
    let header = lines.next().ok_or("CSV is empty")?;
    let columns = parse_csv_line(header);
    let mut rows = Vec::new();

    for line in lines.filter(|line| !line.trim().is_empty()) {
        let values = parse_csv_line(line);
        let mut row = HashMap::new();
        for (index, column) in columns.iter().enumerate() {
            row.insert(
                column.clone(),
                values.get(index).cloned().unwrap_or_default(),
            );
        }
        rows.push(row);
    }

    Ok(rows)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

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

fn required<'a>(row: &'a HashMap<String, String>, name: &str) -> Result<&'a str, Box<dyn Error>> {
    row.get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing required column value: {name}").into())
}

fn resolve_dataset_path(dataset_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.exists() {
        path
    } else {
        dataset_dir.join(path)
    }
}

fn candle_index_at_or_before(candles: &[Candle], timestamp: DateTime<Utc>) -> Option<usize> {
    candles
        .iter()
        .enumerate()
        .take_while(|(_, candle)| candle.timestamp <= timestamp)
        .last()
        .map(|(index, _)| index)
        .or(Some(0))
}

fn return_before(candles: &[Candle], index: usize, periods: usize) -> Option<f64> {
    if candles.is_empty() || index == 0 {
        return None;
    }
    let start = index.saturating_sub(periods);
    let start_close = candles[start].close;
    if start_close > 0.0 {
        Some(safe_div(candles[index].close - start_close, start_close))
    } else {
        None
    }
}

fn return_after(candles: &[Candle], index: usize, periods: usize) -> Option<f64> {
    if candles.is_empty() || index >= candles.len() {
        return None;
    }
    let end = (index + periods).min(candles.len().saturating_sub(1));
    let start_close = candles[index].close;
    if start_close > 0.0 {
        Some(safe_div(candles[end].close - start_close, start_close))
    } else {
        None
    }
}

fn volume_spike(candles: &[Candle], index: usize, periods: usize) -> Option<f64> {
    if index == 0 || candles.is_empty() {
        return None;
    }
    let start = index.saturating_sub(periods);
    let previous = &candles[start..index];
    let avg_volume = safe_div(
        previous.iter().map(|candle| candle.volume).sum::<f64>(),
        previous.len() as f64,
    );
    if avg_volume > 0.0 {
        Some(candles[index].volume / avg_volume)
    } else {
        None
    }
}

fn range_position(price: f64, low: f64, high: f64) -> Option<f64> {
    if low.is_finite() && high > low {
        Some(safe_div(price - low, high - low).clamp(0.0, 1.0))
    } else {
        None
    }
}

fn consecutive_candles_before(candles: &[Candle], index: usize, green: bool) -> usize {
    let mut count = 0usize;
    for candle in candles[..=index].iter().rev() {
        let is_green = candle.close >= candle.open;
        if is_green == green {
            count += 1;
        } else {
            break;
        }
    }
    count
}

fn classify_entry(
    pre_5m_return: Option<f64>,
    pre_20m_return: Option<f64>,
    volume_spike: Option<f64>,
    range_position: Option<f64>,
    break_previous_high: bool,
    consecutive_red: usize,
) -> String {
    if break_previous_high && volume_spike.unwrap_or(0.0) >= 1.5 {
        "breakout_volume_entry".to_string()
    } else if pre_5m_return.unwrap_or(0.0) > 0.25
        && volume_spike.unwrap_or(0.0) >= 2.0
        && range_position.unwrap_or(0.0) > 0.65
    {
        "momentum_volume_entry".to_string()
    } else if pre_20m_return.unwrap_or(0.0) < -0.25 && range_position.unwrap_or(1.0) < 0.40 {
        "dip_or_rebound_entry".to_string()
    } else if consecutive_red >= 3 {
        "falling_market_entry".to_string()
    } else if range_position.unwrap_or(0.0) > 0.75 {
        "high_range_entry".to_string()
    } else {
        "unclear_entry".to_string()
    }
}

fn classify_holding(
    max_runup: Option<f64>,
    max_drawdown: Option<f64>,
    add_count: usize,
    holding_seconds: i64,
) -> String {
    if add_count > 0 && max_runup.unwrap_or(0.0) > 0.20 {
        "adds_with_profit_potential".to_string()
    } else if add_count > 0 && max_drawdown.unwrap_or(0.0) < -0.20 {
        "adds_under_drawdown_risk".to_string()
    } else if max_runup.unwrap_or(0.0) > 0.50 && holding_seconds <= 180 {
        "fast_runup_holding".to_string()
    } else if max_drawdown.unwrap_or(0.0) < -0.30 {
        "large_drawdown_tolerated".to_string()
    } else if holding_seconds <= 60 {
        "very_short_holding".to_string()
    } else {
        "normal_holding".to_string()
    }
}

fn classify_exit(
    pnl: f64,
    holding_seconds: i64,
    exit_efficiency: Option<f64>,
    drawdown_from_peak: Option<f64>,
    post_exit_20m_return: Option<f64>,
    sell_after_breakdown: bool,
) -> String {
    if pnl > 0.0 && exit_efficiency.unwrap_or(0.0) >= 0.75 {
        "efficient_take_profit".to_string()
    } else if pnl > 0.0 && drawdown_from_peak.unwrap_or(0.0) < -0.25 {
        "pullback_take_profit".to_string()
    } else if pnl < 0.0 && holding_seconds <= 120 {
        "fast_stop_loss".to_string()
    } else if pnl < 0.0 && sell_after_breakdown {
        "structure_break_stop_loss".to_string()
    } else if post_exit_20m_return.unwrap_or(0.0) > 0.25 {
        "sold_before_continuation".to_string()
    } else if post_exit_20m_return.unwrap_or(0.0) < -0.20 {
        "effective_risk_exit".to_string()
    } else {
        "unclear_exit".to_string()
    }
}

fn empty_entry_features(label: &str) -> EntryFeatures {
    EntryFeatures {
        pre_1m_return: None,
        pre_3m_return: None,
        pre_5m_return: None,
        pre_10m_return: None,
        pre_20m_return: None,
        pre_5m_volume_spike: None,
        pre_20m_volume_spike: None,
        entry_range_position: None,
        break_previous_high: false,
        distance_to_20m_high: None,
        distance_to_20m_low: None,
        consecutive_green_candles: 0,
        consecutive_red_candles: 0,
        volatility_20m: None,
        label: label.to_string(),
    }
}

fn empty_holding_features(label: &str) -> HoldingFeatures {
    HoldingFeatures {
        max_runup_during_holding: None,
        max_drawdown_during_holding: None,
        time_to_max_runup: None,
        time_to_max_drawdown: None,
        highest_price_before_exit: None,
        lowest_price_before_exit: None,
        holding_return_path: None,
        add_count: 0,
        avg_add_interval_seconds: None,
        largest_add_size_sol: None,
        label: label.to_string(),
    }
}

fn empty_exit_features(label: &str) -> ExitFeatures {
    ExitFeatures {
        pre_exit_1m_return: None,
        pre_exit_3m_return: None,
        pre_exit_5m_return: None,
        exit_range_position: None,
        exit_efficiency: None,
        drawdown_from_peak_before_exit: None,
        sell_after_breakdown: false,
        post_exit_5m_return: None,
        post_exit_20m_return: None,
        missed_profit_after_exit: None,
        loss_avoided_after_exit: None,
        label: label.to_string(),
    }
}

fn get_arg(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn row_timestamp(row: &Value) -> Option<DateTime<Utc>> {
    let value = row
        .get("time")
        .or_else(|| row.get("timestamp"))
        .or_else(|| row.get("t"))?;

    if let Some(text) = value.as_str() {
        if let Ok(time) = DateTime::parse_from_rfc3339(text) {
            return Some(time.with_timezone(&Utc));
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

fn row_number(row: &Value, names: &[&str]) -> Option<f64> {
    for name in names {
        if let Some(value) = row.get(*name).and_then(as_f64) {
            return Some(value);
        }
    }
    None
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

fn parse_time(value: &str) -> Result<DateTime<Utc>, Box<dyn Error>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn parse_f64(value: &str) -> f64 {
    value.parse().unwrap_or(0.0)
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

fn avg(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
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

fn fmt_f64(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.9}"))
        .unwrap_or_default()
}

fn fmt_pct(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{:.2}%", value * 100.0))
        .unwrap_or_else(|| "n/a".to_string())
}

fn fmt_ratio(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2}x"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn fmt_plain(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.4}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn bool_str(value: bool) -> &'static str {
    if value { "true" } else { "false" }
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

fn print_help() {
    println!(
        r#"strategy feature extraction MVP

Usage:
  cargo run -- extract-strategy-features --dataset data/strategy_research/wallets/<wallet>

Input:
  selected_positions.csv
  positions.csv
  klines/*.json

Output:
  features/entry_features.csv
  features/holding_features.csv
  features/exit_features.csv
  features/position_features.csv
  reports/feature_comparison.md
"#
    );
}
