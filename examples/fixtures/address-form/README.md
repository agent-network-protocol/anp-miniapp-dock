# Address Form Compatibility Fixture

This mock-only fixture demonstrates a form card that stays behind the `wx.chooseAddress` Host boundary. It uses an opaque address handle and never contains a real recipient address, phone number, token, credential, or local file path.

## Run

```bash
cargo run -p dock-cli -- validate examples/fixtures/address-form
cargo run -p dock-cli -- inspect examples/fixtures/address-form
cargo run -p dock-cli -- test-skill examples/fixtures/address-form
```

## Expected Evidence

- Expected report summary: `expected-test-skill.json`
- Render IR snapshot: `testdata/render-ir/address-form.prepareAddressForm.json`
- Fixture set: `address-form`
- Component path: `components/address-form/index`
- Primary API: `prepareAddressForm`

## Risk Boundary

`prepareAddressForm` is an L4 fixture because a production implementation would require Host consent, provider review, and audit. The fixture only passes `addr_handle_demo_001`; it is a stable opaque mock handle used for regression tests, not user data.
