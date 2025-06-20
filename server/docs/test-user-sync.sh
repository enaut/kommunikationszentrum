#!/bin/bash

# Test script for Django-SpacetimeDB user synchronization
echo "=== Testing Django-SpacetimeDB User Synchronization ==="

# Configuration
WEBHOOK_PROXY_URL="http://localhost:3002"
USER_SYNC_ENDPOINT="$WEBHOOK_PROXY_URL/user-sync"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

# Check if webhook proxy is running
print_status $BLUE "Checking if webhook proxy is running..."
if curl -s "$WEBHOOK_PROXY_URL" > /dev/null 2>&1; then
    print_status $GREEN "✓ Webhook proxy is running"
else
    print_status $RED "✗ Webhook proxy is not running at $WEBHOOK_PROXY_URL"
    print_status $YELLOW "Please start the webhook proxy first:"
    print_status $YELLOW "cd webhook-proxy && cargo run --bin mta_hook"
    exit 1
fi

# Test user upsert
print_status $BLUE "Testing user upsert..."
UPSERT_PAYLOAD='{
    "action": "upsert",
    "user": {
        "mitgliedsnr": 123456,
        "name": "Test User",
        "email": "test@example.com",
        "is_active": true,
        "updated_at": "2024-01-01T12:00:00Z"
    }
}'

echo "Sending upsert request..."
UPSERT_RESPONSE=$(curl -s -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d "$UPSERT_PAYLOAD" \
    "$USER_SYNC_ENDPOINT")

HTTP_CODE="${UPSERT_RESPONSE: -3}"
RESPONSE_BODY="${UPSERT_RESPONSE%???}"

if [ "$HTTP_CODE" = "200" ]; then
    print_status $GREEN "✓ User upsert successful"
    echo "Response: $RESPONSE_BODY"
else
    print_status $RED "✗ User upsert failed (HTTP $HTTP_CODE)"
    echo "Response: $RESPONSE_BODY"
fi

echo

# Test user update
print_status $BLUE "Testing user update..."
UPDATE_PAYLOAD='{
    "action": "upsert",
    "user": {
        "mitgliedsnr": 123456,
        "name": "Updated Test User",
        "email": "updated@example.com",
        "is_active": false,
        "updated_at": "2024-01-02T12:00:00Z"
    }
}'

echo "Sending update request..."
UPDATE_RESPONSE=$(curl -s -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d "$UPDATE_PAYLOAD" \
    "$USER_SYNC_ENDPOINT")

HTTP_CODE="${UPDATE_RESPONSE: -3}"
RESPONSE_BODY="${UPDATE_RESPONSE%???}"

if [ "$HTTP_CODE" = "200" ]; then
    print_status $GREEN "✓ User update successful"
    echo "Response: $RESPONSE_BODY"
else
    print_status $RED "✗ User update failed (HTTP $HTTP_CODE)"
    echo "Response: $RESPONSE_BODY"
fi

echo

# Test user delete
print_status $BLUE "Testing user delete..."
DELETE_PAYLOAD='{
    "action": "delete",
    "user": {
        "mitgliedsnr": 123456,
        "name": null,
        "email": null,
        "is_active": null,
        "updated_at": null
    }
}'

echo "Sending delete request..."
DELETE_RESPONSE=$(curl -s -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d "$DELETE_PAYLOAD" \
    "$USER_SYNC_ENDPOINT")

HTTP_CODE="${DELETE_RESPONSE: -3}"
RESPONSE_BODY="${DELETE_RESPONSE%???}"

if [ "$HTTP_CODE" = "200" ]; then
    print_status $GREEN "✓ User delete successful"
    echo "Response: $RESPONSE_BODY"
else
    print_status $RED "✗ User delete failed (HTTP $HTTP_CODE)"
    echo "Response: $RESPONSE_BODY"
fi

echo

# Test malformed request
print_status $BLUE "Testing malformed request handling..."
MALFORMED_PAYLOAD='{"invalid": "data"}'

echo "Sending malformed request..."
MALFORMED_RESPONSE=$(curl -s -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d "$MALFORMED_PAYLOAD" \
    "$USER_SYNC_ENDPOINT")

HTTP_CODE="${MALFORMED_RESPONSE: -3}"
RESPONSE_BODY="${MALFORMED_RESPONSE%???}"

if [ "$HTTP_CODE" = "400" ] || [ "$HTTP_CODE" = "422" ]; then
    print_status $GREEN "✓ Malformed request properly rejected (HTTP $HTTP_CODE)"
    echo "Response: $RESPONSE_BODY"
else
    print_status $YELLOW "? Malformed request handling: HTTP $HTTP_CODE"
    echo "Response: $RESPONSE_BODY"
fi

echo
print_status $BLUE "=== Test Summary ==="
print_status $GREEN "User synchronization endpoint testing complete!"
print_status $YELLOW "Next steps:"
print_status $YELLOW "1. Test the Django signals by creating/updating users"
print_status $YELLOW "2. Run initial sync: python manage.py sync_users_to_spacetimedb --dry-run"
print_status $YELLOW "3. Check SpacetimeDB logs for sync events"
