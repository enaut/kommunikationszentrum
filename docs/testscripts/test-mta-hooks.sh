#!/bin/bash

echo "Testing MTA Hook endpoints..."

# Test 1: Connect stage
echo "Test 1: Connect stage"
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d '{
    "context": {
      "stage": "connect",
      "client": {
        "ip": "192.168.1.100",
        "port": 12345,
        "ptr": null,
        "helo": null,
        "activeConnections": 1
      },
      "server": {
        "name": "Test MTA",
        "port": 25,
        "ip": "192.168.1.1"
      },
      "protocol": {
        "version": 1
      }
    },
    "envelope": null,
    "message": null
  }'

echo -e "\n\n"

# Test 2: EHLO stage
echo "Test 2: EHLO stage"
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d '{
    "context": {
      "stage": "ehlo",
      "client": {
        "ip": "192.168.1.100",
        "port": 12345,
        "ptr": "client.example.org",
        "helo": "client.example.org",
        "activeConnections": 1
      },
      "server": {
        "name": "Test MTA",
        "port": 25,
        "ip": "192.168.1.1"
      },
      "protocol": {
        "version": 1
      }
    },
    "envelope": null,
    "message": null
  }'

echo -e "\n\n"

# Test 3: MAIL FROM stage
echo "Test 3: MAIL FROM stage"
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d '{
    "context": {
      "stage": "mail",
      "client": {
        "ip": "192.168.1.100",
        "port": 12345,
        "ptr": "client.example.org",
        "helo": "client.example.org",
        "activeConnections": 1
      },
      "server": {
        "name": "Test MTA",
        "port": 25,
        "ip": "192.168.1.1"
      },
      "protocol": {
        "version": 1
      }
    },
    "envelope": {
      "from": {
        "address": "sender@example.org"
      },
      "to": []
    },
    "message": null
  }'

echo -e "\n\n"

# Test 4: RCPT TO stage
echo "Test 4: RCPT TO stage"
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d '{
    "context": {
      "stage": "rcpt",
      "client": {
        "ip": "192.168.1.100",
        "port": 12345,
        "ptr": "client.example.org",
        "helo": "client.example.org",
        "activeConnections": 1
      },
      "server": {
        "name": "Test MTA",
        "port": 25,
        "ip": "192.168.1.1"
      },
      "protocol": {
        "version": 1
      }
    },
    "envelope": {
      "from": {
        "address": "sender@example.org"
      },
      "to": [
        {
          "address": "category@kommunikationszentrum.org"
        }
      ]
    },
    "message": null
  }'

echo -e "\n\n"

# Test 5: DATA stage
echo "Test 5: DATA stage"
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d '{
    "context": {
      "stage": "data",
      "client": {
        "ip": "192.168.1.100",
        "port": 12345,
        "ptr": "client.example.org",
        "helo": "client.example.org",
        "activeConnections": 1
      },
      "server": {
        "name": "Test MTA",
        "port": 25,
        "ip": "192.168.1.1"
      },
      "protocol": {
        "version": 1
      },
      "queue": {
        "id": "TEST123456"
      }
    },
    "envelope": {
      "from": {
        "address": "sender@example.org"
      },
      "to": [
        {
          "address": "category@kommunikationszentrum.org"
        }
      ]
    },
    "message": {
      "headers": [
        ["Date", " Mon, 01 Jan 2024 12:00:00 +0000\r\n"],
        ["From", " Sender <sender@example.org>\r\n"],
        ["Subject", " Test Message for Category\r\n"],
        ["To", " category@kommunikationszentrum.org\r\n"],
        ["Message-Id", " <TEST.123456@example.org>\r\n"],
        ["MIME-Version", " 1.0\r\n"],
        ["Content-Type", " text/plain; charset=utf-8\r\n"]
      ],
      "contents": "This is a test message for the kommunikationszentrum.\r\n\r\nIt should be processed by SpacetimeDB and logged appropriately.\r\n\r\nBest regards,\r\nTest Sender\r\n",
      "size": 150
    }
  }'

echo -e "\n\nDone! Check SpacetimeDB logs for MTA processing results."
