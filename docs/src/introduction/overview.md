# Overview

The **Kommunikationszentrum** is a email management system designed for the SoLaWi (Solidarische Landwirtschaft / Community Supported Agriculture) project. It provides intelligent email routing, subscription management.

## What is the Kommunikationszentrum?

The Kommunikationszentrum acts as an intelligent email gateway that:

- **Filters and routes emails** based on categories and user subscriptions
- **Manages user subscriptions** to different mailing lists/categories
- **Provides spam protection** through IP blocking and validation
- **Integrates with existing systems** like Stalwart MTA and Django user management
- **Offers a web interface** for subscription and administrative management

```d2
# Globale Konfiguration
direction: right

# Externe und globale Komponenten
Internet {
  Externe Sender: "Externe Sender\n(web.de, posteo.de...)" {
  }
  MX: "DNS-Server\nMX,DKIM,DMARC,SPF" {
  }
  Externe Empfänger: "Externe Empfänger\n(web.de, posteo.de...)" {
  }
  smtp2go: "smtp2go\n(o.ä.)\n(Mails senden)" {
  }
}

Mail-Infrastruktur {
Stalwart: "Stalwart\n(Mails senden)" {
}

RocksDB

S3: "S3\n(Mailspeicher)" {
}
Stalwart <-> RocksDB: "Metadaten"
S3 <-> Stalwart: "Anhänge und Emails" {
  style.stroke-dash: 2
}
}

# Mailinglisten
Kommunikationszentrum: "Kommunikationszentrum" {

  Server: "Server\n(läuft in Spacetimedb)"{}

  Sender

  Admin
  
  Admin <-> Server: Nutzer konfigurieren
  Sender -> Server: verschickt zu sendende mails
}

# Lokale Mailclients Gruppe
Mailclients: "Mailclients" {
  
  Thunderbird: "z.B. Thunderbird\n(Lokaler Mailclient)" {
  }
  Balwark: "Balwark\n(Webmailclient)" {
  }
}

# Contabo Server Gruppe
Contabo: "Contabo (Server)" {
  Django: "Django\n(Webportal wir.solawis.de)" {
  sync_users_to_spacetime: "sync_users_to_spacetime\n(mitgliederverwaltung.management.commands)"
  mailing
  django-oauth
  }
}

# Verbindungen und Beziehungen
Internet.Externe Sender -> Mail-Infrastruktur.Stalwart:" Senden an Mailinglisten"

Internet.MX -> Mail-Infrastruktur.Stalwart: "Verweist und Signiert"{
  style.stroke-dash: 5
}


Mailclients.Thunderbird <-> Mail-Infrastruktur.Stalwart: "IMAP/SMTP"
Mailclients.Balwark <-> Mail-Infrastruktur.Stalwart: "JMAP"

Mail-Infrastruktur.Stalwart -> Internet.smtp2go: "sendet via relay\n(Spamvermeidung)"
Mail-Infrastruktur.Stalwart -> Internet.Externe Empfänger: "Sendet direkt"
Internet.smtp2go -> Internet.Externe Empfänger:"liefert aus"

Mail-Infrastruktur.Stalwart -> Kommunikationszentrum.Server: "Mailinglistenmail\nper webhook/HTTP"
Kommunikationszentrum.Sender -> Mail-Infrastruktur.Stalwart: "Sendet via SMTP\n(direkt)"
Kommunikationszentrum.Sender -> Internet.smtp2go: "Sendet via SMTP\n(relay)"

Contabo.Django.mailing -> Mail-Infrastruktur.Stalwart: "Mailversand"
Contabo.Django.signale -> Kommunikationszentrum.Server: "Synchronisiert\nNutzer und\nListen"
Contabo.Django.sync_users_to_spacetime -> Kommunikationszentrum.Server: "Synchronisiert\nNutzer und\nListen"
Contabo.Django.django-oauth <-> Kommunikationszentrum.Admin: "OAuth2.0\nAuthentifizierung"
```

## Key Features

### 🔐 **Authentication & Authorization**
- OAuth integration with Django solawispielplatz
- JWT-based authentication for secure API access
- Role-based permissions (admin vs. regular users)
- Seamless user synchronization between systems

### 📧 **Email Processing**
- Real-time MTA hook processing from Stalwart email server
- Category-based email routing (e.g., `news@solawi.org`, `events@solawi.org`)
- Comprehensive logging of all email transactions

### 🛡️ **Spam Protection**
- IP-based blocking system with configurable rules
- Email format validation at multiple stages
- Sender verification and subscription checks

### 👥 **User Management**
- Self-service subscription management interface
- Admin interface for user and category administration
- Automatic user synchronization from Django
- Real-time updates via WebSocket connections

### 📊 **Monitoring & Logging**
- Detailed audit logs for all MTA operations
- Connection-level and message-level logging

## Use Cases

The Kommunikationszentrum is designed for organizations that need:

1. **Mailing List Management**: Organizations with multiple email categories (news, events, announcements) where users can selectively subscribe
2. **Spam Protection**: Advanced filtering beyond basic MTA capabilities
3. **User Integration**: Seamless integration with existing user management systems
4. **Audit Compliance**: Detailed logging and monitoring of email operations
5. **Self-Service**: Allow users to manage their own subscriptions without admin intervention

## Technology Stack

### **Backend Components**
- **SpacetimeDB**: Modern database with real-time capabilities and rust/WASM modules

### **Frontend Components**
- **Dioxus**: Rust-based WebAssembly frontend framework
- **WebAssembly**: Near-native performance in the browser
- **Bootstrap**: Responsive UI components

### **Integration Technologies**
- **OAuth 2.0 / OpenID Connect**: Secure authentication
- **JWT**: Stateless authentication tokens
- **JSON**: Data exchange format
- **HTTP Webhooks**: Real-time event processing

## Architecture Principles

The Kommunikationszentrum follows these key principles:

### **Modularity**
Each component has a single responsibility:
- **SpacetimeDB Server**: Data storage and business logic
- **Admin Interface**: User interaction
- **Django Integration**: User management and authentication

## Target Audience

### **End Users**
Community members who want to:
- Manage their email subscriptions
- Subscribe/unsubscribe from categories
- View their subscription status

### **Administrators**
System administrators who:
- Manage user accounts and permissions
- Configure email categories

### **Developers**
Technical team members who:
- Deploy and maintain the system
- Integrate with other services
- Extend functionality
- Debug issues

## Getting Started

For a quick start, see the [Quick Start Guide](../introduction/quick-start.md) which will get you up and running with a development environment in minutes.

For detailed setup instructions, proceed to the [Setup & Installation](../setup/prerequisites.md) section.

To understand the system architecture in detail, continue to the [Architecture](../core/architecture.md) chapter.
