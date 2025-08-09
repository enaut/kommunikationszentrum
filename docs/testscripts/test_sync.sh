#!/bin/bash

# Test sync_user reducer directly
spacetime call kommunikation sync_user 'upsert' '{"mitgliedsnr": 12345, "name": "Direct Test", "email": "direct@test.com", "is_active": true}'
