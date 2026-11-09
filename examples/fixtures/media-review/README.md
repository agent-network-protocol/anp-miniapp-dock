# Media Review Compatibility Fixture

This mock-only fixture demonstrates image/file `format` handling, opaque media handles, static image preview, and static canvas preview. It never reads a local file and never includes file contents, tokens, credentials, or local absolute paths.

## Run

```bash
cargo run -p dock-cli -- validate examples/fixtures/media-review
cargo run -p dock-cli -- inspect examples/fixtures/media-review
cargo run -p dock-cli -- test-skill examples/fixtures/media-review
```

## Expected Evidence

- Expected report summary: `expected-test-skill.json`
- Render IR snapshot: `testdata/render-ir/media-review.reviewMedia.json`
- Fixture set: `media-review`
- Component path: `components/media-review/index`
- Primary API: `reviewMedia`

## Risk Boundary

`reviewMedia` is an L4 fixture because a production implementation would require a Host media/file provider, consent, and audit. The fixture uses opaque `image_handle_demo_001` and `file_handle_demo_001` values plus `example.invalid` preview URLs only.
