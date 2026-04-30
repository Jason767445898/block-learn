# Strategy Feature Comparison

## Dataset

- analyzed positions: 100
- skipped positions: 0
- profit samples analyzed: 50
- loss samples analyzed: 50

## Group Averages

| metric | profit | loss |
|---|---:|---:|
| pre 5m return | 60.25% | 8.70% |
| pre 20m return | 75.01% | 14.42% |
| pre 5m volume spike | 14.20x | 2.33x |
| entry range position | 0.7355 | 0.5565 |
| max runup during holding | 57.20% | 46.60% |
| max drawdown during holding | -40.10% | -30.61% |
| holding seconds | 77.1400 | 46.2200 |
| exit efficiency | 0.7984 | 0.6968 |
| post exit 20m return | -14.42% | -7.16% |

## Reading Notes

- `entry_range_position` close to `1.0` means the wallet entered near the local 20m high.
- `volume_spike` above `2.0` means the entry candle volume was more than twice the recent average.
- `max_runup_during_holding` shows whether the trade quickly had enough upside to justify holding.
- `max_drawdown_during_holding` shows how much adverse movement the wallet tolerated.
- `exit_efficiency` close to `1.0` means it sold near the local high during its holding window.

This report is descriptive. Candidate trading rules should only be created after checking the CSV rows and testing them out of sample.
