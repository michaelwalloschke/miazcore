# Wire fixture manifest format v1

Each `*.manifest` file describes one independently specified hexadecimal wire fixture. The format is UTF-8, one `key=value` record per line, with no quoting or duplicate keys. Version 1 requires exactly these keys:

- `format=miazcore-wire-fixture-v1`
- `build=12340`
- `direction`
- `opcode`
- `semantics`
- `byte_length`
- `sha256`
- `provenance`
- `upstream_pin`
- `payload`

`payload` names a sibling lowercase hexadecimal file. Its decoded byte length and SHA-256 must match the manifest. Fixtures in this directory are synthetic: they contain no live credentials, real session keys, authenticated captures, or server data.

The login transcript uses fixed account `LEARNER`, password `ONLYFORVECTOR`, client entropy `01..20`, server entropy `41..60`, salt `21..40`, generator `7`, and the standard 32-byte Wrath SRP prime. The world transcript uses session key `00..27`, server seed `0x11223344`, client seed `0x55667788`, realm `1`, and one complete synthetic `Miaztest` character record. Values were calculated independently with Python standard-library SHA-1, HMAC, arbitrary-precision integers, manual packet layouts, and a small RC4 reference on 2026-07-22. Production Rust code did not generate the expected bytes.
