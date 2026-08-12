# ZCode (Z.ai) Data Source

ccusage reads ZCode model usage from its local SQLite database and estimates
costs from token counts. The database is opened read-only; no ZCode data is
changed.

## Commands

```bash
ccusage zcode daily
ccusage zcode monthly
ccusage zcode session --json
```

ZCode is also included automatically in unified `ccusage daily`, `ccusage
monthly`, and `ccusage session` reports when its database is present.

## Data location

By default ccusage reads:

```text
~/.zcode/cli/db/db.sqlite
```

Set `ZCODE_HOME` to use another ZCode home directory or comma-separated archive
homes. Each path must contain `cli/db/db.sqlite`.

```bash
ZCODE_HOME="$HOME/.zcode,/archive/zcode" ccusage zcode daily
```

## Token accounting and pricing

Only completed `model_usage` rows are counted. ZCode timestamps are Unix
milliseconds. `input_tokens` contains both cached and fresh prompt tokens, so
ccusage calculates fresh input as:

```text
input_tokens - cache_read_input_tokens - cache_creation_input_tokens
```

Cache reads and cache creation are reported separately and priced at the
cached-input rate. ZCode reasoning tokens are already included in
`output_tokens`, so they are not counted a second time. ZCode does not store USD
costs, so `auto` and `calculate`
use the shared pricing engine; `display` reports zero. This applies to all
`model_id` values, including custom providers: models without pricing data are
still reported with zero cost. GLM-5.2 has an embedded fallback price of $1.40
input, $0.26 cached input, and $4.40 output per million tokens, so `--offline`
works too.

## Troubleshooting

If no data appears, ensure `~/.zcode/cli/db/db.sqlite` exists, or set
`ZCODE_HOME` to the ZCode home directory rather than the SQLite file itself.
