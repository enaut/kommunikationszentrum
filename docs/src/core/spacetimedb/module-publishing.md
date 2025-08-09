# Module Publishing

This chapter explains how to build, publish, and manage the SpacetimeDB module for the Kommunikationszentrum.

## Prerequisites

Before publishing the module, ensure you have:

- **SpacetimeDB CLI** installed and configured
- **Rust toolchain** with `wasm32-unknown-unknown` target
- **Access** to the SpacetimeDB instance (running on port 3000)

## Building the Module

- **Navigate to server directory**: `cd server`
- **Build WASM module**: `cargo build --target wasm32-unknown-unknown --release`
  - Compiled module location: `target/wasm32-unknown-unknown/release/server.wasm`
- **Verify build**: `ls -la target/wasm32-unknown-unknown/release/server.wasm`

## Publishing the Module

Initial Publication:
- **First-time publication**: `spacetime publish --project-path server kommunikation`
  - `--project-path server`: Server directory containing the module
  - `kommunikation`: Database name where module will be published

Schema Updates:
- **With schema changes**: `spacetime publish --project-path server kommunikation -c`
  - The `-c` flag clears the database and recreates it with new schema
  - **⚠️ Warning**: `-c` flag deletes all existing data

Regular Updates:
- **Code-only changes**: `spacetime publish --project-path server kommunikation`

## Starting SpacetimeDB

- **Start server**: `spacetime start` (runs on `localhost:3000`)

## Checking Publication Status

- **Module information**: `spacetime describe kommunikation`
- **List databases**: `spacetime list`
- **View logs**: `spacetime logs kommunikation`

## Development Workflow

Typical Development Cycle:
1. **Make Changes**: Edit reducers or schema in `server/src/lib.rs`
2. **Build**: `cargo build --target wasm32-unknown-unknown --release`
3. **Publish**: `spacetime publish --project-path server kommunikation`
4. **Test**: Use `spacetime call` or test via webhook proxy
5. **Debug**: Check logs with `spacetime logs kommunikation`

Schema Migration Workflow:
1. **Backup Data** (if needed): `spacetime sql kommunikation "SELECT * FROM account" > account_backup.json`
2. **Update Schema**: Modify table definitions in `lib.rs`
3. **Build and Publish**: `cargo build --target wasm32-unknown-unknown --release` then `spacetime publish --project-path server kommunikation -c`
4. **Restore Data** (if applicable): Re-sync users from Django or restore from backup

## Testing the Module


- **Test category creation**: `spacetime call kommunikation add_message_category "Test News" "test@solawi.org" "Test category"`
- **Test user sync**: `spacetime call kommunikation sync_user "upsert" '{"mitgliedsnr": 999, "name": "Test User", "email": "test@example.org", "is_active": true}'`
- **View logs**: `spacetime call kommunikation get_mta_logs`


- **Test MTA hooks**: `cd server/docs && ./test-mta-hooks.sh`

## Common Issues and Solutions

- **Error**: `error: failed to run custom build command for 'spacetimedb-sdk'`
- **Solution**: Ensure WASM target is installed: `rustup target add wasm32-unknown-unknown`


- **Error**: `Connection refused (os error 61)`
  - **Solution**: Ensure SpacetimeDB is running: `spacetime start`
- **Error**: `Database schema mismatch`
  - **Solution**: Use `-c` flag to reset database: `spacetime publish --project-path server kommunikation -c`


## Debugging and Troubleshooting

- **Follow logs**: `spacetime logs kommunikation --follow`

- **Count records**: `spacetime sql kommunikation "SELECT COUNT(*) FROM account"`
- **View categories**: `spacetime sql kommunikation "SELECT * FROM message_categories LIMIT 10"`
- **Recent connections**: `spacetime sql kommunikation "SELECT * FROM mta_connection_log ORDER BY timestamp DESC LIMIT 5"`

This covers the complete module publishing workflow for the SpacetimeDB component of the Kommunikationszentrum.
