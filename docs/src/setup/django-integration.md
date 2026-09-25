# Django Integration

## 3. User Synchronization Flow

```d2
{{#include event-flow-user-sync.d2}}
```

**Identity derivation:** The account's SpacetimeDB `Identity` is computed deterministically
from the OIDC issuer URL and the user's `external_id` (OIDC `sub` claim):

```rust
let identity = Identity::from_claims(OIDC_ISSUER_URL, &data.external_id);
```

This means the identity stored in `account` will match the identity that the user's browser
presents when it connects via the Admin UI OAuth flow — no additional mapping is needed.
