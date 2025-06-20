# SpacetimeDB Webhook Setup

Diese Lösung ermöglicht es Ihnen, HTTP-Webhooks zu empfangen und diese an SpacetimeDB weiterzuleiten.

## Architektur

1. **SpacetimeDB Module** (`/server`): Enthält die Geschäftslogik und Datenbank-Tabellen
2. **Webhook Proxy** (`/webhook-proxy`): HTTP-Server, der Webhooks empfängt und an SpacetimeDB weiterleitet

## Setup

### 1. SpacetimeDB Module kompilieren und starten

```bash
cd server
cargo build --target wasm32-unknown-unknown --release
spacetime start
spacetime publish spacetime-module target/wasm32-unknown-unknown/release/spacetime_module.wasm
```

### 2. Webhook Proxy starten

```bash
cd webhook-proxy
cargo run
```

Der Webhook Proxy läuft dann auf `http://localhost:3001/hook`

## Verwendung

### Webhook senden

```bash
curl -X POST http://localhost:3001/hook \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Test message",
    "sender": "test@example.com"
  }'
```

### Mit SpacetimeDB CLI interagieren

```bash
# Alle Webhook-Logs anzeigen
spacetime call kommunikationszentrum get_webhook_logs

# Alle Personen anzeigen
spacetime call kommunikationszentrum say_hello
```

## Datenstrukturen

### WebhookPayload (Input)
```json
{
  "message": "string",
  "sender": "string"
}
```

### SpacetimeDB Tabellen
- `person`: Speichert Personennamen
- `webhook_log`: Speichert alle eingehenden Webhook-Daten

## Alternative: Direkte SpacetimeDB SDK Verwendung

Falls Sie den HTTP-Proxy nicht verwenden möchten, können Sie auch direkt mit dem SpacetimeDB SDK arbeiten:

```rust
use spacetimedb_sdk::*;

// Direkt Reducer aufrufen
client.call_reducer("handle_webhook", vec![json_payload.into()]).await?;
```
