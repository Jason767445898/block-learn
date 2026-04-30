use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
struct PositionFeature {
    sample_group: String,
    realized_pnl_sol: f64,
    realized_roi: f64,
    holding_seconds: i64,
    entry_label: String,
    pre_5m_return: Option<f64>,
    pre_20m_return: Option<f64>,
    pre_5m_volume_spike: Option<f64>,
    entry_range_position: Option<f64>,
    max_runup_during_holding: Option<f64>,
    max_drawdown_during_holding: Option<f64>,
    exit_efficiency: Option<f64>,
}

#[derive(Debug, Clone)]
struct CandidateRule {
    name: String,
    rule_type: String,
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone)]
enum Condition {
    Min {
        field: Field,
        label: &'static str,
        threshold: f64,
    },
    Max {
        field: Field,
        label: &'static str,
        threshold: f64,
    },
    Label {
        field: LabelField,
        label: &'static str,
        value: String,
    },
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Pre5mReturn,
    Pre20mReturn,
    Pre5mVolumeSpike,
    EntryRangePosition,
}

#[derive(Debug, Clone, Copy)]
enum LabelField {
    EntryLabel,
}

#[derive(Debug)]
struct RuleEvaluation {
    name: String,
    rule_type: String,
    expression: String,
    matched: usize,
    profit_count: usize,
    loss_count: usize,
    win_rate: f64,
    lift_vs_baseline: f64,
    avg_roi: f64,
    avg_pnl_sol: f64,
    median_holding_seconds: Option<i64>,
    avg_max_runup: Option<f64>,
    avg_max_drawdown: Option<f64>,
    avg_exit_efficiency: Option<f64>,
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return Ok(());
    }

    let dataset_dir = get_arg(args, "--dataset")
        .or_else(|| args.first().cloned())
        .ok_or("--dataset <path> is required")?;
    let min_matches = parse_arg(args, "--min-matches", 8_usize);
    let top = parse_arg(args, "--top", 25_usize);
    let dataset_dir = PathBuf::from(dataset_dir);
    let feature_path = dataset_dir.join("features").join("position_features.csv");
    let report_dir = dataset_dir.join("reports");
    fs::create_dir_all(&report_dir)?;

    let positions = read_position_features(&feature_path)?;
    if positions.is_empty() {
        return Err(format!("no position features found: {}", feature_path.display()).into());
    }

    let baseline_win_rate = safe_div(
        positions
            .iter()
            .filter(|position| position.sample_group == "profit")
            .count() as f64,
        positions.len() as f64,
    );
    let rules = build_candidate_rules(&positions);
    let mut evaluations: Vec<RuleEvaluation> = rules
        .iter()
        .filter_map(|rule| evaluate_rule(rule, &positions, baseline_win_rate))
        .filter(|evaluation| evaluation.matched >= min_matches)
        .collect();

    evaluations.sort_by(|a, b| {
        b.win_rate
            .total_cmp(&a.win_rate)
            .then_with(|| b.matched.cmp(&a.matched))
            .then_with(|| b.avg_roi.total_cmp(&a.avg_roi))
    });

    let selected: Vec<&RuleEvaluation> = evaluations.iter().take(top).collect();
    write_rule_candidates_csv(&report_dir.join("rule_candidates.csv"), &evaluations)?;
    write_rule_candidates_md(
        &report_dir.join("rule_candidates.md"),
        &positions,
        baseline_win_rate,
        min_matches,
        &selected,
        &label_distribution(&positions),
    )?;

    println!("strategy rule candidate MVP");
    println!("dataset: {}", dataset_dir.display());
    println!("positions: {}", positions.len());
    println!("baseline win rate: {:.1}%", baseline_win_rate * 100.0);
    println!("candidate rules: {}", evaluations.len());
    println!("shown top: {}", selected.len());
    println!();
    println!("written");
    println!("- {}", report_dir.join("rule_candidates.csv").display());
    println!("- {}", report_dir.join("rule_candidates.md").display());

    Ok(())
}

fn build_candidate_rules(positions: &[PositionFeature]) -> Vec<CandidateRule> {
    let mut rules = Vec::new();
    let return_thresholds = [-0.5, -0.25, 0.0, 0.1, 0.25, 0.5, 1.0];
    let volume_thresholds = [1.0, 1.5, 2.0, 3.0, 5.0, 10.0];
    let range_min_thresholds = [0.4, 0.5, 0.65, 0.75, 0.85];
    let range_max_thresholds = [0.25, 0.35, 0.5];

    for threshold in return_thresholds {
        rules.push(CandidateRule::single_min(
            "pre_5m_return",
            Field::Pre5mReturn,
            threshold,
        ));
        rules.push(CandidateRule::single_min(
            "pre_20m_return",
            Field::Pre20mReturn,
            threshold,
        ));
    }

    for threshold in volume_thresholds {
        rules.push(CandidateRule::single_min(
            "pre_5m_volume_spike",
            Field::Pre5mVolumeSpike,
            threshold,
        ));
    }

    for threshold in range_min_thresholds {
        rules.push(CandidateRule::single_min(
            "entry_range_position",
            Field::EntryRangePosition,
            threshold,
        ));
    }

    for threshold in range_max_thresholds {
        rules.push(CandidateRule {
            name: format!("entry_range_position <= {threshold:.2}"),
            rule_type: "entry_filter".to_string(),
            conditions: vec![Condition::Max {
                field: Field::EntryRangePosition,
                label: "entry_range_position",
                threshold,
            }],
        });
    }

    for volume in volume_thresholds {
        for range in range_min_thresholds {
            rules.push(CandidateRule {
                name: format!("volume >= {volume:.2}x AND range >= {range:.2}"),
                rule_type: "entry_filter_combo".to_string(),
                conditions: vec![
                    Condition::Min {
                        field: Field::Pre5mVolumeSpike,
                        label: "pre_5m_volume_spike",
                        threshold: volume,
                    },
                    Condition::Min {
                        field: Field::EntryRangePosition,
                        label: "entry_range_position",
                        threshold: range,
                    },
                ],
            });
        }
    }

    for ret in return_thresholds {
        for volume in volume_thresholds {
            rules.push(CandidateRule {
                name: format!("pre_5m_return >= {ret:.2} AND volume >= {volume:.2}x"),
                rule_type: "entry_filter_combo".to_string(),
                conditions: vec![
                    Condition::Min {
                        field: Field::Pre5mReturn,
                        label: "pre_5m_return",
                        threshold: ret,
                    },
                    Condition::Min {
                        field: Field::Pre5mVolumeSpike,
                        label: "pre_5m_volume_spike",
                        threshold: volume,
                    },
                ],
            });
        }
    }

    for ret in return_thresholds {
        for range in range_min_thresholds {
            rules.push(CandidateRule {
                name: format!("pre_5m_return >= {ret:.2} AND range >= {range:.2}"),
                rule_type: "entry_filter_combo".to_string(),
                conditions: vec![
                    Condition::Min {
                        field: Field::Pre5mReturn,
                        label: "pre_5m_return",
                        threshold: ret,
                    },
                    Condition::Min {
                        field: Field::EntryRangePosition,
                        label: "entry_range_position",
                        threshold: range,
                    },
                ],
            });
        }
    }

    for ret in [0.0, 0.1, 0.25, 0.5] {
        for volume in [1.5, 2.0, 3.0, 5.0] {
            for range in [0.5, 0.65, 0.75] {
                rules.push(CandidateRule {
                    name: format!(
                        "pre_5m_return >= {ret:.2} AND volume >= {volume:.2}x AND range >= {range:.2}"
                    ),
                    rule_type: "entry_filter_combo".to_string(),
                    conditions: vec![
                        Condition::Min {
                            field: Field::Pre5mReturn,
                            label: "pre_5m_return",
                            threshold: ret,
                        },
                        Condition::Min {
                            field: Field::Pre5mVolumeSpike,
                            label: "pre_5m_volume_spike",
                            threshold: volume,
                        },
                        Condition::Min {
                            field: Field::EntryRangePosition,
                            label: "entry_range_position",
                            threshold: range,
                        },
                    ],
                });
            }
        }
    }

    let mut labels: Vec<String> = positions
        .iter()
        .map(|position| position.entry_label.clone())
        .collect();
    labels.sort();
    labels.dedup();

    for label in labels {
        rules.push(CandidateRule {
            name: format!("entry_label == {label}"),
            rule_type: "entry_label".to_string(),
            conditions: vec![Condition::Label {
                field: LabelField::EntryLabel,
                label: "entry_label",
                value: label,
            }],
        });
    }

    rules
}

impl CandidateRule {
    fn single_min(name: &'static str, field: Field, threshold: f64) -> Self {
        Self {
            name: format!("{name} >= {threshold:.2}"),
            rule_type: "entry_filter".to_string(),
            conditions: vec![Condition::Min {
                field,
                label: name,
                threshold,
            }],
        }
    }
}

fn evaluate_rule(
    rule: &CandidateRule,
    positions: &[PositionFeature],
    baseline_win_rate: f64,
) -> Option<RuleEvaluation> {
    let matches: Vec<&PositionFeature> = positions
        .iter()
        .filter(|position| rule.matches(position))
        .collect();
    if matches.is_empty() {
        return None;
    }

    let matched = matches.len();
    let profit_count = matches
        .iter()
        .filter(|position| position.sample_group == "profit")
        .count();
    let loss_count = matches
        .iter()
        .filter(|position| position.sample_group == "loss")
        .count();
    let win_rate = safe_div(profit_count as f64, matched as f64);
    let avg_roi = safe_div(
        matches
            .iter()
            .map(|position| position.realized_roi)
            .sum::<f64>(),
        matched as f64,
    );
    let avg_pnl_sol = safe_div(
        matches
            .iter()
            .map(|position| position.realized_pnl_sol)
            .sum::<f64>(),
        matched as f64,
    );
    let mut holding_seconds: Vec<i64> = matches
        .iter()
        .map(|position| position.holding_seconds)
        .collect();

    Some(RuleEvaluation {
        name: rule.name.clone(),
        rule_type: rule.rule_type.clone(),
        expression: rule.expression(),
        matched,
        profit_count,
        loss_count,
        win_rate,
        lift_vs_baseline: win_rate - baseline_win_rate,
        avg_roi,
        avg_pnl_sol,
        median_holding_seconds: median_i64(&mut holding_seconds),
        avg_max_runup: avg(matches
            .iter()
            .map(|position| position.max_runup_during_holding)),
        avg_max_drawdown: avg(matches
            .iter()
            .map(|position| position.max_drawdown_during_holding)),
        avg_exit_efficiency: avg(matches.iter().map(|position| position.exit_efficiency)),
    })
}

impl CandidateRule {
    fn matches(&self, position: &PositionFeature) -> bool {
        self.conditions
            .iter()
            .all(|condition| condition.matches(position))
    }

    fn expression(&self) -> String {
        self.conditions
            .iter()
            .map(Condition::expression)
            .collect::<Vec<_>>()
            .join(" AND ")
    }
}

impl Condition {
    fn matches(&self, position: &PositionFeature) -> bool {
        match self {
            Condition::Min {
                field, threshold, ..
            } => field
                .value(position)
                .is_some_and(|value| value >= *threshold),
            Condition::Max {
                field, threshold, ..
            } => field
                .value(position)
                .is_some_and(|value| value <= *threshold),
            Condition::Label { field, value, .. } => field.value(position) == value,
        }
    }

    fn expression(&self) -> String {
        match self {
            Condition::Min {
                label, threshold, ..
            } => format!("{label} >= {threshold:.4}"),
            Condition::Max {
                label, threshold, ..
            } => format!("{label} <= {threshold:.4}"),
            Condition::Label { label, value, .. } => format!("{label} == {value}"),
        }
    }
}

impl Field {
    fn value(self, position: &PositionFeature) -> Option<f64> {
        match self {
            Field::Pre5mReturn => position.pre_5m_return,
            Field::Pre20mReturn => position.pre_20m_return,
            Field::Pre5mVolumeSpike => position.pre_5m_volume_spike,
            Field::EntryRangePosition => position.entry_range_position,
        }
    }
}

impl LabelField {
    fn value(self, position: &PositionFeature) -> &str {
        match self {
            LabelField::EntryLabel => &position.entry_label,
        }
    }
}

fn write_rule_candidates_csv(
    path: &Path,
    evaluations: &[RuleEvaluation],
) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from(
        "rank,name,rule_type,expression,matched,profit_count,loss_count,win_rate,lift_vs_baseline,avg_roi,avg_pnl_sol,median_holding_seconds,avg_max_runup,avg_max_drawdown,avg_exit_efficiency\n",
    );

    for (index, evaluation) in evaluations.iter().enumerate() {
        push_csv_row(
            &mut csv,
            &[
                &(index + 1).to_string(),
                &evaluation.name,
                &evaluation.rule_type,
                &evaluation.expression,
                &evaluation.matched.to_string(),
                &evaluation.profit_count.to_string(),
                &evaluation.loss_count.to_string(),
                &fmt_f64(Some(evaluation.win_rate)),
                &fmt_f64(Some(evaluation.lift_vs_baseline)),
                &fmt_f64(Some(evaluation.avg_roi)),
                &fmt_f64(Some(evaluation.avg_pnl_sol)),
                &evaluation
                    .median_holding_seconds
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                &fmt_f64(evaluation.avg_max_runup),
                &fmt_f64(evaluation.avg_max_drawdown),
                &fmt_f64(evaluation.avg_exit_efficiency),
            ],
        );
    }

    fs::write(path, csv)?;
    Ok(())
}

fn write_rule_candidates_md(
    path: &Path,
    positions: &[PositionFeature],
    baseline_win_rate: f64,
    min_matches: usize,
    selected: &[&RuleEvaluation],
    label_counts: &BTreeMap<String, (usize, usize)>,
) -> Result<(), Box<dyn Error>> {
    let mut content = String::new();
    content.push_str("# Strategy Rule Candidates\n\n");
    content.push_str("## Dataset\n\n");
    content.push_str(&format!("- positions: {}\n", positions.len()));
    content.push_str(&format!(
        "- baseline win rate in balanced sample: {:.2}%\n",
        baseline_win_rate * 100.0
    ));
    content.push_str(&format!("- minimum matches per rule: {min_matches}\n\n"));
    content.push_str("## Top Candidate Entry Rules\n\n");
    content.push_str("| rank | rule | matches | win rate | avg ROI | avg PnL SOL | median hold | avg runup | avg drawdown | exit efficiency |\n");
    content.push_str("|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");

    for (index, evaluation) in selected.iter().enumerate() {
        content.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {:.4} | {} | {} | {} | {} |\n",
            index + 1,
            evaluation.expression,
            evaluation.matched,
            fmt_pct(Some(evaluation.win_rate)),
            fmt_pct(Some(evaluation.avg_roi)),
            evaluation.avg_pnl_sol,
            evaluation
                .median_holding_seconds
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            fmt_pct(evaluation.avg_max_runup),
            fmt_pct(evaluation.avg_max_drawdown),
            fmt_plain(evaluation.avg_exit_efficiency),
        ));
    }

    content.push_str("\n## Entry Label Distribution\n\n");
    content.push_str("| entry label | profit | loss | win rate |\n");
    content.push_str("|---|---:|---:|---:|\n");
    for (label, (profit, loss)) in label_counts {
        let total = profit + loss;
        content.push_str(&format!(
            "| `{label}` | {profit} | {loss} | {} |\n",
            fmt_pct(Some(safe_div(*profit as f64, total as f64)))
        ));
    }

    content.push_str("\n## Important Notes\n\n");
    content.push_str("- These are candidate filters, not a finished trading system.\n");
    content.push_str("- The rules intentionally use entry-side fields only: `pre_5m_return`, `pre_20m_return`, `pre_5m_volume_spike`, and `entry_range_position`.\n");
    content.push_str("- Metrics such as max runup and exit efficiency are included only to understand what happened after entry; do not use them as live entry conditions.\n");
    content.push_str("- Because this is a balanced 50 profit / 50 loss sample, win rate here measures separation power, not the wallet's real-world win rate.\n");
    content.push_str("- Next step: test the best few rules on out-of-sample positions that were not used in this 100-sample dataset.\n");

    fs::write(path, content)?;
    Ok(())
}

fn label_distribution(positions: &[PositionFeature]) -> BTreeMap<String, (usize, usize)> {
    let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for position in positions {
        let entry = counts.entry(position.entry_label.clone()).or_default();
        if position.sample_group == "profit" {
            entry.0 += 1;
        } else if position.sample_group == "loss" {
            entry.1 += 1;
        }
    }
    counts
}

fn read_position_features(path: &Path) -> Result<Vec<PositionFeature>, Box<dyn Error>> {
    let rows = read_csv_rows(path)?;
    let mut positions = Vec::new();

    for row in rows {
        positions.push(PositionFeature {
            sample_group: required(&row, "sample_group")?.to_string(),
            realized_pnl_sol: parse_f64(required(&row, "realized_pnl_sol")?),
            realized_roi: parse_f64(required(&row, "realized_roi")?),
            holding_seconds: row
                .get("holding_seconds")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            entry_label: row.get("entry_label").cloned().unwrap_or_default(),
            pre_5m_return: parse_optional_f64(row.get("pre_5m_return")),
            pre_20m_return: parse_optional_f64(row.get("pre_20m_return")),
            pre_5m_volume_spike: parse_optional_f64(row.get("pre_5m_volume_spike")),
            entry_range_position: parse_optional_f64(row.get("entry_range_position")),
            max_runup_during_holding: parse_optional_f64(row.get("max_runup_during_holding")),
            max_drawdown_during_holding: parse_optional_f64(row.get("max_drawdown_during_holding")),
            exit_efficiency: parse_optional_f64(row.get("exit_efficiency")),
        });
    }

    Ok(positions)
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

fn parse_f64(value: &str) -> f64 {
    value.parse().unwrap_or(0.0)
}

fn parse_optional_f64(value: Option<&String>) -> Option<f64> {
    value
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse().ok())
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

fn median_i64(values: &mut [i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
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

fn fmt_plain(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.4}"))
        .unwrap_or_else(|| "n/a".to_string())
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
        r#"strategy rule candidate MVP

Usage:
  cargo run -- generate-rule-candidates --dataset data/strategy_research/wallets/<wallet>

Options:
  --min-matches 8
  --top 25

Input:
  features/position_features.csv

Output:
  reports/rule_candidates.csv
  reports/rule_candidates.md
"#
    );
}
