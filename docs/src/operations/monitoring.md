# Monitoring

The Kommunikationszentrum infrastructure monitors mail transport, HTTP hook delivery, database activity, and system health through Prometheus metrics, OpenTelemetry distributed tracing, and SpacetimeDB audit log tables.

---

## Stalwart MTA Metrics (Prometheus)

Stalwart exposes a built-in Prometheus collector endpoint on its HTTP listener port (`http://<stalwart-host>:8093/metrics/prometheus`).

![Stalwart Prometheus Metrics Collector](../email/img/stalwart-metrics-prometheus.png)

### Collector Configuration

1. In the Stalwart Web Admin UI, navigate to **Settings → Telemetry / Metrics → Prometheus Collector**.
2. Set **Collector** to `Enabled`.
3. Set **Username** and **Secret** if basic authentication is needed (or `No secret` for internal network scraping).

### Prometheus Scrape Configuration

The repository includes a ready-to-use Prometheus scrape configuration in `grafana/prometheus.yaml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']

  - job_name: 'stalwart'
    metrics_path: '/metrics/prometheus'
    metric_relabel_configs:
      - source_labels: [__name__]
        regex: '(.*)'
        replacement: 'stalwart_$1'
        target_label: __name__
        action: replace
    static_configs:
      - targets: ['host.containers.internal:8093']
```

Key Stalwart metrics exposed include:
- `stalwart_smtp_connections_total`: Inbound SMTP connection counts.
- `stalwart_smtp_messages_received_total`: Total accepted messages.
- `stalwart_mta_hook_requests_total`: Total webhook requests sent to SpacetimeDB.
- `stalwart_mta_hook_duration_seconds`: Hook round-trip latency.

---

## Distributed Tracing (OpenTelemetry)

Stalwart supports exporting trace spans and structured logs over gRPC to an OpenTelemetry collector (e.g. Jaeger or Grafana Tempo):

![Stalwart OpenTelemetry Tracer Setup](../email/img/stalwart-trace-setup.png)

1. In the Stalwart Web Admin UI, navigate to **Settings → Telemetry / Tracing → Tracer**.
2. Configure:
   - **Tracer type**: `Open Telemetry (gRPC)`
   - **Endpoint**: Collector URL (e.g. `https://tracing.example.com/v1/otel`)
   - **Export logs**: Enabled
   - **Export spans**: Enabled
   - **Throttle**: `1 Seconds`
   - **Timeout**: `10 Seconds`
   - **Authentication**: `Anonymous` (or Bearer / Basic)
   - **Enable this tracer**: Enabled
   - **Logging level**: `Trace` (or `Info` for production)

---

## SpacetimeDB MTA Audit Logs

SpacetimeDB persists audit records for every MTA connection attempt and message processed:

| Table | Stage | Content |
|---|---|---|
| `mta_connection_log` | `CONNECT`, `EHLO`, `MAIL`, `RCPT`, `AUTH` | Client IP, reverse PTR, HELO argument, sender address, recipient category match, acceptance/rejection action, timestamp. |
| `mta_message_log` | `DATA` | Message ID, sender, matched categories, subscriber status, delivery disposition (accepted / quarantined / rejected), timestamp. |

### Inspecting Logs via CLI

Dump recent MTA logs to the SpacetimeDB console:

```bash
# Invoke the debug dump reducer
spacetime call kommunikation dump_mta_logs_to_server_logs

# Query the connection log table directly
spacetime sql kommunikation "SELECT * FROM mta_connection_log ORDER BY timestamp DESC LIMIT 10"

# Query the message log table
spacetime sql kommunikation "SELECT * FROM mta_message_log ORDER BY timestamp DESC LIMIT 10"
```
