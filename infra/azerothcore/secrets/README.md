# Local Reference Realm secrets

Run `../realm init-secrets` from this directory's parent. It creates these ignored files with mode `0600`:

- `database-password`
- `database-root-password`
- `fixture-account`
- `fixture-password`
- `fixture-pair-a-account`
- `fixture-pair-a-password`
- `fixture-pair-b-account`
- `fixture-pair-b-password`

The committed Compose model contains only secret filenames. Never commit their contents.
