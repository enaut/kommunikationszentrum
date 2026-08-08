# Django Integration

## 3. User Synchronization Flow

```d2
{{#include event-flow-user-sync.d2}}
```

**Identity derivation:** The account's SpacetimeDB `Identity` is computed deterministically
from the Django OAuth issuer URL and the user's `mitgliedsnr`:

```rust
let issuer_url = format!("{}{}", DJANGO_OAUTH_BASE_URL, DJANGO_OAUTH_ISSUER_PATH);
let identity = Identity::from_claims(&issuer_url, &mitgliedsnr.to_string());
```

This means the identity stored in `account` will match the identity that the user's browser
presents when it connects via the Admin UI OAuth flow — no additional mapping is needed.
