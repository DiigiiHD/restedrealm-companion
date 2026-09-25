# Account-paired upload contract, owner pilot

The production website has an owner-only account pairing page and a private Collector intake. The owner PC is paired. A synthetic production batch passed accepted and duplicate checks and was removed. Gameplay upload is available through the companion's Upload now button or after the owner enables its automatic upload checkbox.

## Pairing

1. The person signs in to RestedRealm through the existing Discord or Battle.net flow and generates a 16-character, single-use code on `/account/collector`. It expires in five minutes. The page shows the upload-only scope and connected devices.
2. The person pastes the code into the companion. The companion redeems it over HTTPS and receives a random upload-only credential. It stores the credential in Windows Credential Manager, not in SavedVariables, the SQLite queue or a report. The server stores only a credential hash and an account link. The account page can revoke a device.
3. The checkbox is off by default. Completed saves stay in the durable local queue until Upload now or automatic opt-in. The account page lists connected devices and can revoke one.

## Batch and response

- `POST /api/collector/batches` with `Authorization: Bearer <device credential>`, HTTPS only. No Discord or Battle.net token is sent.
- Version 1 accepts at most 100 observations and 1 MiB of JSON per request. Every record carries Forever product, build and locale. The server limits the request body before parsing.
- Each observation has a stable local sequence and SHA-256 digest of its transmitted JSON. The server recomputes that digest, checks kind, product, build, locale, size, structure and prohibited prose fields, and enforces field-specific limits for projected quest reward claims. It does not prove that the game produced the record.
- The server validates and commits a whole batch or rejects it. A successful response counts accepted and duplicate observations; the companion acknowledges local records only when those counts cover the entire batch. A timeout or ambiguous response leaves them pending. Replaying an accepted batch creates no second observation.
- Keep account and device identifiers private. Raw intake is separate from `gd_rows`, `source_records` and QuestieDB reference. Only structured quest choice item IDs, XP and coin enter the first per-field evidence table. A public quest page may show a scoped corroboration panel after two distinct accounts agree on a build and character context without a conflict. This remains a report, not verified game truth.
- Full NPC and quest prose stays local until the content-rights and privacy review determines which fields may be sent and published. Short IDs and relationships can form the first private pilot.

## Acceptance checks

- Wrong account cannot approve another account's device or read its batches.
- Revoked or expired credentials fail; a device credential cannot call normal account, Admin or editorial APIs.
- Foreign-origin browser approval and replayed pairing codes fail.
- Two game sessions, a one-week offline queue, duplicate upload, interrupted upload and interrupted game save preserve exactly one accepted observation per source sequence.
- Account export and deletion include queued server observations and device grants. Contributor credit remains anonymous unless separately opted in.
