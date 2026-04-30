mod strategy_dataset;
mod strategy_features;
mod strategy_rules;
mod wallet_analyzer;
mod gmgn_reverse;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args
        .get(1)
        .is_some_and(|command| command == "build-strategy-dataset")
    {
        strategy_dataset::run(&args[2..]).await
    } else if args
        .get(1)
        .is_some_and(|command| command == "extract-strategy-features")
    {
        strategy_features::run(&args[2..])
    } else if args
        .get(1)
        .is_some_and(|command| command == "generate-rule-candidates")
    {
        strategy_rules::run(&args[2..])
    } else if args.get(1).is_some_and(|command| command == "gmgn-reverse") {
        gmgn_reverse::run(&args[2..]).await
    } else {
        wallet_analyzer::run().await
    }
}
