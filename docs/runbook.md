# Runbook

Operational procedures. Authored in full at milestone M13, when the components
they describe exist. This file is a placeholder that lists the procedures the
final runbook must cover, so the structure is fixed early.

Planned contents:

- Cold start procedure (bring up Redis and TimescaleDB, start the engine in
  dry-run, verify book sync and detection rate, then the gated path to live).
- Key rotation (Ed25519 API keys, encrypted-at-rest material, zero downtime).
- WAL rotation and archival (daily UTC-midnight rotation, retention, offload).
- TimescaleDB retention policies.
- Incident response for common failures: WebSocket flapping, Binance rate limit
  exceeded, unexpected inventory imbalance, sequence-gap storms.
- Using the three-channel kill switch (UI, CLI, HTTP `/panic`).
