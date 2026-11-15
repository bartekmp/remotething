# Test keys

These key pairs exist only for the tests in this directory. They protect nothing; never use them anywhere else.

| File | Used as | Passphrase |
| --- | --- | --- |
| `host_key` | Host key of the in-process test server | none |
| `client_key` | A user's key the test server accepts | none |
| `locked_key` | An encrypted user key | `open sesame` |
| `client_key.ppk` | The same key as `client_key`, in PuTTY's format (version 3) | none |
| `locked_key.ppk` | The same key, encrypted in PuTTY's format (version 2) | `open sesame` |
