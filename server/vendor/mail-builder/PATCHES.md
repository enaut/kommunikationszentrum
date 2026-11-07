# Local SpacetimeDB compatibility patch

This vendored copy is based on `mail-builder` 2.0.1 (Apache-2.0 OR MIT, see `LICENSE-APACHE` and `LICENSE-MIT`).

The upstream crate depends on `web-time` for `wasm32-unknown-unknown`, which pulls in `wasm-bindgen`. SpacetimeDB modules reject `wasm-bindgen` dependencies, so the `web-time` target dependency was removed and the crate uses `std::time` on the module target. Pipeline messages provide explicit Date and Message-ID headers, and avoid browser-specific runtime APIs. Re-evaluate this patch when upgrading `mail-builder`.