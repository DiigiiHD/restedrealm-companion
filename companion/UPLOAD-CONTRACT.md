# Account-paired upload contract

Status, 25 September 2026: the pairing described here is open to every signed-in account in the website source, which is not yet deployed. Production still runs the owner-only pilot.

## Pairing

1. **Through the browser (the Windows app).** The app creates a random `state` (48 hex characters) and opens `https://restedrealm.com/companion/connect?state=…&device=…`. Opening that page changes nothing. When the signed-in player presses Connect this PC, the page calls `POST /api/collector/connect` with the state (same-origin, signed in) and receives a one-time code and `restedrealm-companion://connect?code=…&state=…`. The browser opens that link; the app accepts it only if the state is the one it created in the last 15 minutes, then redeems the code as below. Any other link is ignored.
2. **With a typed code (fallback).** The player creates a 16-character, single-use code on `/account/companion`. It expires after five minutes.
3. Redeeming: `POST /api/collector/redeem` with the code and the PC's name returns a random upload-only credential. The app stores it in Windows Credential Manager, not in SavedVariables, the queue or a report. The server stores only its hash and the account link. The account page can disconnect a PC.

## Batch and response

- `POST /api/collector/batches` with `Authorization: Bearer <device credential>`, HTTPS only. No Discord or Battle.net token is sent.
- Version 1 accepts at most 100 observations and 1 MiB of JSON per request. Every record carries Forever product, build and locale. The server limits the request body before parsing.
- Each observation has a stable local sequence and SHA-256 digest of its transmitted JSON. The server recomputes that digest, checks kind, product, build, locale, size, structure and prohibited prose fields, and enforces field-specific limits for projected quest reward claims. It does not prove that the game produced the record.
- A malformed batch (bad metadata, a repeated record, a checksum mismatch) is refused whole. A well-formed record the server will not store (unknown kind, private text field, too large) is refused on its own and listed in the response as `rejected: [{ sourceId, seq, reason }]`; the rest of the batch is stored. The app acknowledges local records only when `accepted + duplicate + rejected` covers the entire batch and every rejected entry names a record it sent. It keeps refused records locally, shows them as "Not accepted" and does not send that version again; a corrected version (new digest) is sent. The Python prototype does not know `rejected` and leaves such a batch pending. A timeout or ambiguous response leaves them pending. Replaying an accepted batch creates no second observation.
- Keep account and device identifiers private. Raw intake is separate from `gd_rows`, `source_records` and QuestieDB reference. Only structured quest choice item IDs, XP and coin enter the first per-field evidence table. A public quest page may show a scoped corroboration panel after two distinct accounts agree on a build and character context without a conflict. This remains a report, not verified game truth.
- Full NPC and quest prose stays local until the content-rights and privacy review determines which fields may be sent and published. Short IDs and relationships can form the first private pilot.

## Acceptance checks

- Wrong account cannot approve another account's device or read its batches.
- Revoked or expired credentials fail; a device credential cannot call normal account, Admin or editorial APIs.
- Foreign-origin browser approval and replayed pairing codes fail.
- Two game sessions, a one-week offline queue, duplicate upload, interrupted upload and interrupted game save preserve exactly one accepted observation per source sequence.
- Account export and deletion include queued server observations and device grants. Contributor credit remains anonymous unless separately opted in.
