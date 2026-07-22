# Local Reference Realm secrets

Run `../realm init-secrets` from this directory's parent. It creates these ignored files with mode `0600`:

- `database-password`
- `database-root-password`
- `fixture-account`
- `fixture-password`

The committed Compose model contains only secret filenames. Never commit their contents.
