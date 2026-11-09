# Location Map Preview Compatibility Fixture

This mock-only fixture demonstrates a static `map-preview` card fed by an opaque Host location handle. It never includes precise coordinates, a real address, local file paths, credentials, or tokens.

## Run

```bash
cargo run -p dock-cli -- validate examples/fixtures/location-map-preview
cargo run -p dock-cli -- inspect examples/fixtures/location-map-preview
cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview
```

## Expected Evidence

- Expected report summary: `expected-test-skill.json`
- Render IR snapshot: `testdata/render-ir/location-map-preview.prepareLocationMap.json`
- Fixture set: `location-map-preview`
- Component path: `components/location-map-preview/index`
- Primary API: `prepareLocationMap`

## Risk Boundary

`prepareLocationMap` is an L4 fixture because a production implementation would require Host location provider consent and audit. The fixture only passes `location_handle_demo_001` and a mock region label; it deliberately avoids latitude and longitude values.
