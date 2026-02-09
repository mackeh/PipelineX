# Threshold Alerts

PipelineX dashboard includes a threshold-based alert system for Phase 3 platform monitoring.

## Supported Metrics

- `avg_duration_sec`
- `failure_rate_pct`
- `monthly_opportunity_cost_usd`

## Operators

- `gt`
- `gte`
- `lt`
- `lte`

## API

```bash
# List rules
curl -s http://localhost:3000/api/alerts

# Create rule
curl -s -X POST http://localhost:3000/api/alerts \
  -H "content-type: application/json" \
  -d '{
    "name": "Failure rate above 15%",
    "metric": "failure_rate_pct",
    "operator": "gte",
    "threshold": 15
  }'

# Evaluate rules against cached history snapshots
curl -s http://localhost:3000/api/alerts/evaluate

# Delete rule
curl -s -X DELETE "http://localhost:3000/api/alerts?id=<rule-id>"
```

## Cost-Based Evaluation Defaults

The `monthly_opportunity_cost_usd` metric uses:

- `PIPELINEX_ALERT_RUNS_PER_MONTH` (default `500`)
- `PIPELINEX_ALERT_DEVELOPER_HOURLY_RATE` (default `150`)

You can override both per-evaluation:

`GET /api/alerts/evaluate?runsPerMonth=800&developerHourlyRate=175`
