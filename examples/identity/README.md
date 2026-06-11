# Local DID Identity

`dock-cli run-demo` uses this directory by default when no credential flags or
`ANP_DOCK_*` credential environment variables are set.

Expected local files:

```text
did_document.json
key-1-private.pem
```

Keep these files local. Do not commit DID documents, private keys, capability
tokens, merchant secrets, or real user data into this repository.
