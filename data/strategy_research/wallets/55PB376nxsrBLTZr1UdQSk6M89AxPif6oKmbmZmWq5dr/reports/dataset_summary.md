# Strategy Replication Dataset Summary

wallet: `55PB376nxsrBLTZr1UdQSk6M89AxPif6oKmbmZmWq5dr`

## Sources

- trades source: `bitquery`
- kline source: `gmgn`
- resolution: `1m`

## Position Counts

- total positions: 137
- closed positions: 135
- profitable closed positions: 59
- losing closed positions: 76

## Selected Samples

- profit samples: 50
- loss samples: 50
- kline files saved: 100
- kline downloads skipped/failed: 0

## Files

- `positions.csv`: complete aggregated position list
- `selected_positions.csv`: balanced research sample
- `klines/`: raw K-line JSON files for selected positions
- `features/`: reserved for feature extraction output
- `reports/`: reserved for comparison and rule candidate reports

## Notes

This command only builds the dataset. It does not yet decide whether the strategy is profitable or safe to copy.
