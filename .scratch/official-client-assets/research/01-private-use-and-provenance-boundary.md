# Private-use and provenance boundary

## Conclusion

There is no evidence-based basis to approve either downloaded folder as an
authorized **User-Supplied Client Asset Set** for an independent importer.
Technical compatibility and a user's possession of client files do not create a
right to reproduce, transform, or use those assets in a separate application.

Private, local-only handling is an important harm-reduction rule, but it is not
an express permission. The current integration route must therefore remain
**permission-gated**: no importer, cache generator, or rendered Blizzard
content may be implemented or run until the user can establish a directly
applicable authorization or deliberately redraws the destination to use
project-owned content only.

This is a product-risk assessment based on published terms, not legal advice.
Only qualified counsel can assess applicable law, the user's exact historical
license, and any enforceability or exception questions.

## Evidence

### The local folders cannot establish provenance

Both inspected folders include an old World of Warcraft EULA stating that the
software is licensed, not sold; that unexpressly authorized use, reproduction,
modification, or distribution is prohibited; and that the client is licensed
for non-commercial entertainment in conjunction with the Service. Their
private-server `realmlist.wtf` files and the Thera custom MPQ overlays establish
configuration history, not authorization or a chain of title.

Neither folder contains an original-media transfer record, purchase record,
Battle.net entitlement record, or written Blizzard permission. File hashes,
embedded copyright notices, and an unmodified core archive stack can establish
format identity only; they cannot show how the copy was acquired or what rights
attach to it.

### Published Blizzard terms do not supply the missing permission

Blizzard's current [End User License Agreement](https://www.blizzard.com/en-us/legal/08b946df-660a-40e4-a072-1fbde65173b1/blizzard-end-user-license-agreement)
describes the Platform and Games as licensed rather than sold, limits use to
personal and non-commercial entertainment, prohibits copying/reproduction and
derivative works except where the agreement provides otherwise, and reserves
ownership of Platform content. It also describes a narrow original-media
transfer mechanism rather than a general right to transfer copies.

The official [legal index](https://www.blizzard.com/legal/) lists the EULA and
separate policies, but this research found no published asset-importer license
for World of Warcraft. Absence from that index is not proof that no private
permission can be granted; it means public terms do not give this project an
affirmative basis to assume one.

## Required authorization task

Before technical work handles Blizzard asset bytes, the user must complete
**Establish asset-use authorization**:

1. Retain private evidence showing how the candidate source was lawfully
   acquired and that the user controls any applicable license. Do not commit it
   or provide it to the repository.
2. Obtain written authorization from Blizzard that explicitly covers a local,
   non-distributed independent reader/importer and any required transient or
   cached transformations, or obtain qualified legal advice that supports the
   exact intended activity in the user's jurisdiction.
3. Record only the resulting yes/no decision and the permitted boundaries in
   the tracker; never upload purchase records, client files, or correspondence.

The agent must not contact Blizzard, a lawyer, or any third party without
separate user authorization.

## Controls if authorization is obtained

Authorization should be narrowed into the importer contract as follows:

- Require an explicit user-selected source root; never download, discover
  network shares, or bundle source data.
- Keep raw archives and generated cache outside Git, build artifacts, logs,
  screenshots, test fixtures, and evidence bundles.
- Store only a non-sensitive source fingerprint and permitted-version marker;
  avoid full paths and asset names in telemetry or diagnostic output.
- Make cache deletion and re-import explicit, local operations.
- Fail closed when the source has custom overlays or lacks the approved
  authorization/version/locale markers.

These controls constrain exposure after authorization; they do not substitute
for it.
