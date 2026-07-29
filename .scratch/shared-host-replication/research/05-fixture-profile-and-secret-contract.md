# Fixture Profile and secret contract

Ticket: [05 – Decide the Fixture Profile and secret contract](../issues/05-decide-fixture-profile-and-secret-contract.md)

## Decision

The Learning Client accepts one optional, non-sensitive profile selector:

```text
learning_client --fixture-profile pair-a
learning_client --fixture-profile pair-b
```

Without the option, the existing single-client `Miaztest` Reference Realm
configuration remains unchanged. The selector is intentionally a closed
two-value set, not a path, account, Character, or environment-variable
override. The future Dual-Client Orchestrator invokes the two declared values;
ordinary users do not supply credentials through the command line.

## Profile mapping

| Profile token | Sanitized Character identity | Private account file | Private password file |
| --- | --- | --- | --- |
| `pair-a` | `Miazpaira` | `infra/azerothcore/secrets/fixture-pair-a-account` | `infra/azerothcore/secrets/fixture-pair-a-password` |
| `pair-b` | `Miazpairb` | `infra/azerothcore/secrets/fixture-pair-b-account` | `infra/azerothcore/secrets/fixture-pair-b-password` |

Both resolve to the fixed loopback Reference Realm identity and endpoints
already validated by `ClientConfig`. The mapping belongs in a private
`learning_client` CLI/configuration adapter. It passes the selected
`CredentialPaths` into the existing `ClientConfig` construction and then calls
the existing zeroizing loader. `client_session` gains no account-manager,
profile registry, credential getter, or public credential-selection API.

## Secret ownership and handling

- The two file pairs are ignored regular files owned by the local Reference
  Realm fixture-reset workflow, not by the client or the future orchestrator.
  `reset-state` creates/restores them with mode `0600`; clients only read them
  through the existing ownership, regular-file, symlink, size, encoding, and
  identity-exposure checks.
- The files contain the separate Pair A/B account and password values. Those
  values never occur in command-line arguments, environment variables,
  profile tokens, diagnostics, events, sidecars, screenshots, fixture dumps,
  or Git.
- The profile token and configured Character identity are non-sensitive. They
  may appear in sanitized diagnostics and acceptance sidecars to distinguish
  the two client roles. A token never substitutes for Realm GUID identity.
- Existing `fixture-account` and `fixture-password` remain the default
  single-client files. They are not aliases for either pair and are never
  silently reused by a selected Pair Profile.

## Parsing and failure contract

- `--fixture-profile` requires exactly one following ASCII token and may occur
  once. Duplicate selectors, a missing value, or any token other than
  `pair-a`/`pair-b` fail before Bevy or a session is created.
- The error is a generic configuration failure such as `unsupported fixture
  profile`; it never echoes the supplied token, a path, an account, or a
  password. Credential-file failures retain the existing redacted
  `ConfigError` categories.
- A selected profile does not prove that its peer is available, acquire a
  process-wide lock, or prohibit a duplicate launch. Ticket 10 owns
  concurrent-process admission and detects duplicate Fixture Profiles before
  it starts either client.
- The selector can accompany existing non-secret proof options. It changes
  only the configuration chosen before those modes start; it grants no new
  network, render, or secret-output capability.

## Explicit deferrals

- Arbitrary profile names, custom profile files, environment overrides, or
  command-line endpoint/Character/credential configuration.
- Secret provisioning, pair Pdump generation, and `reset-state` implementation
  (Ticket 12); loopback process topology and locking (Ticket 10).
- Name-query display metadata, Realm GUID selection, LAN/Windows exposure, and
  a general account manager.

## Verification required by later implementation

1. CLI parser tests accept each exact profile once and reject missing,
   duplicate, and unknown values without echoing supplied text.
2. Configuration tests prove `pair-a` and `pair-b` select distinct fixed
   Character identities and distinct private credential paths while retaining
   the current loopback and build validation.
3. File tests prove missing, symlinked, insecure, malformed, or identity-leaking
   pair credentials fail through the existing redacted configuration boundary.
4. Sidecar/format tests prove profile token and Character identity are allowed,
   while both selected credential values and paths are absent.
5. The later paired reset and orchestrator gates prove the files are provisioned
   once per reset and no duplicate Profile launches reach the Realm.
