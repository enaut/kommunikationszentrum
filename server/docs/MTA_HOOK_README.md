# SpacetimeDB MTA Hook Integration

Diese Implementierung erweitert das Kommunikationszentrum um vollständige Stalwart MTA Hook-Unterstützung.

## Architektur

1. **SpacetimeDB Module** (`/server`): Erweitert um MTA-spezifische Tabellen und Reducer
2. **Webhook Proxy** (`/webhook-proxy`): Ursprünglicher HTTP-zu-SpacetimeDB Proxy
3. **MTA Hook Handler** (`/webhook-proxy/mta-hook`): Spezialisierter Handler für Stalwart MTA Hooks

## MTA Hook Features

### Unterstützte Stages
- **Connect**: IP-Überprüfung und Verbindungsvalidierung
- **EHLO**: HELO/EHLO-Validierung
- **MAIL**: Sender-Validierung
- **RCPT**: Empfänger-/Kategorie-Validierung  
- **DATA**: Vollständige Nachrichtenverarbeitung mit Kategorien und Abonnements
- **AUTH**: Authentifizierung (derzeit accept-all)

### SpacetimeDB Tabellen

#### MTA-spezifische Tabellen
- `mta_connection_log`: Protokolliert alle Verbindungsereignisse
- `mta_message_log`: Protokolliert alle Nachrichten mit Metadaten
- `blocked_ips`: IP-Blacklist-Verwaltung

#### Kategorie-Verwaltung
- `message_categories`: E-Mail-Kategorien (z.B. solawi@example.org)
- `subscriptions`: Benutzer-Abonnements für Kategorien

## Setup

### 1. SpacetimeDB Module kompilieren
```bash
cd server
cargo build --target wasm32-unknown-unknown --release
spacetime start
spacetime publish kommunikationszentrum target/wasm32-unknown-unknown/release/spacetime_module.wasm
```

### 2. Bindings generieren
```bash
cd webhook-proxy
spacetime generate --lang rust --project-path ../server
```

### 3. MTA Hook Handler starten
```bash
cd webhook-proxy
cargo run --bin mta-hook
```

Der MTA Hook Handler läuft auf `http://localhost:3002/mta-hook`

### 4. Optional: Original Webhook Proxy starten
```bash
cd webhook-proxy  
cargo run --bin webhook-proxy
```

## Verwendung

### MTA Hook Testing
```bash
./test-mta-hooks.sh
```

### Kategorien und Abonnements verwalten

```bash
# Neue Kategorie hinzufügen
spacetime call kommunikationszentrum add_message_category "SoLaWi News" "news@solawi.org" "Neuigkeiten der SoLaWi"

# Abonnement hinzufügen
spacetime call kommunikationszentrum add_subscription "mitglied@example.org" 1

# IP blockieren
spacetime call kommunikationszentrum block_ip "192.168.1.100" "Spam source"

# Logs anzeigen
spacetime call kommunikationszentrum get_mta_logs
```

## Stalwart MTA Konfiguration

Konfigurieren Sie Stalwart MTA, um Hooks an den Handler zu senden:

```toml
[session.hook]
url = "http://localhost:3002/mta-hook"
timeout = "30s"
```

## Funktionsweise

### 1. **Connect Stage**
- Überprüft eingehende IP gegen `blocked_ips` Tabelle
- Protokolliert alle Verbindungsversuche
- Akzeptiert oder blockiert basierend auf IP-Status

### 2. **EHLO Stage**
- Validiert HELO/EHLO-Parameter
- Grundlegende Syntax-Überprüfung
- Protokolliert verdächtige HELO-Strings

### 3. **MAIL FROM Stage**
- Validiert Sender-E-Mail-Adresse
- Überprüft grundlegende E-Mail-Syntax
- Kann erweitert werden um Sender-Whitelist/Blacklist

### 4. **RCPT TO Stage**
- Überprüft ob Empfänger-Adresse einer gültigen Kategorie entspricht
- Validiert gegen `message_categories` Tabelle
- Reject für unbekannte Kategorien

### 5. **DATA Stage**
- Vollständige Nachrichtenverarbeitung
- Überprüft Sender-Abonnements für Ziel-Kategorien
- Fügt Verarbeitungs-Header hinzu
- Protokolliert vollständige Nachrichtenmetadaten

## Privacy & Security

- Alle IP-Adressen werden als "[REDACTED]" geloggt
- E-Mail-Adressen und Subjects werden anonymisiert gespeichert
- Sensitive Daten werden nur zur Verarbeitung verwendet, nicht persistent gespeichert
- Vollständige Audit-Logs aller MTA-Aktivitäten

## Erweiterungen

Das System kann einfach erweitert werden um:
- Spam-Filter-Integration
- DKIM/SPF-Validierung
- Rate-Limiting pro Sender
- Automatische Quarantäne-Regeln
- Integration mit externen Authentifizierungssystemen
