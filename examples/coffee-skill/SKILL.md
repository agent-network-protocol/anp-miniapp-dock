# Coffee Order Skill

This Skill supports a minimal coffee ordering flow:

1. `searchDrinks` finds matching drinks.
2. `confirmOrder` creates a pending order from a selected drink.
3. `payOrder` completes a mock payment after user confirmation.

All data in this fixture is mock-only. Do not place real DID credentials, capability tokens, merchant secrets, or user data in this example.

## Localhost HTTP Demo

When API arguments include `serverUrl` or `remoteBaseUrl`, this Skill uses `wx.login()` and `wx.request()` to call a localhost coffee service. The default demo service lives in `examples/coffee-fastapi-server` and exposes `/api/login`, `/api/drinks`, `/api/order/confirm`, and `/api/order/pay`. Without a server URL, the APIs keep returning local mock data for focused runtime tests.

The remote HTTP flow uses container-managed DID auth:

- `wx.login()` is treated as the DID-auth boundary and only confirms that host-managed authentication succeeded.
- `/api/login` is kept as a compatibility account binding/status endpoint, not as a credential delivery path.
- Subsequent `wx.request()` calls do not set bearer headers in Skill JS; the host/container attaches request credentials.
- API `_meta` declares `authBoundary: "container-managed"` and `tokenVisibleToSkill: false`.
