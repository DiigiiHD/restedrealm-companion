# RestedRealm Forever Collector handoff

Last verified: 25 September 2026. This document is the starting point for another coding agent working from this repository. [README.md](../README.md), [companion/README.md](../companion/README.md), [COVERAGE-AUDIT.md](../COVERAGE-AUDIT.md) and [PREWORK.md](../PREWORK.md) provide the detailed history and known API gaps.

## Purpose and scope

RestedRealm is a World of Warcraft: Forever only fan site. The addon records game events exposed by the Forever UI API during ordinary, opted-in play. The Windows companion imports completed addon SavedVariables saves into an offline SQLite queue and sends selected structured observations to the RestedRealm website after account pairing. It never reads game memory or network traffic. It does not need Discord or Battle.net credentials: the user signs into the website and redeems a short connection code, while the companion stores an upload-only device credential in Windows Credential Manager.

The addon and companion source live in the public [restedrealm-companion repository](https://github.com/DiigiiHD/restedrealm-companion). The website source and migrations live in the separate, private [restedrealm repository](https://github.com/DiigiiHD/restedrealm) deployed at `/opt/restedrealm` on the VPS. The local `site-patch/` directory is a historical partial staging copy and is ignored by this repo. Edit the website in its own repository and test it in an isolated checkout before deployment.

## Current verified state

- The addon source is `addon/RestedRealmCollector/`. It targets the Forever `_classic_beta_` installation. The installed copy is under `C:\Program Files (x86)\World of Warcraft\_classic_beta_\Interface\AddOns\RestedRealmCollector`. A source change must be copied to that installed folder before an in-game reload can test it.
- The companion source is `companion/`. Launch `companion/Start RestedRealm Collector.cmd`; the GUI shows the saved pairing on restart. Manual upload works. Automatic upload is **off until the user checks its opt-in box**. The window watches completed saves every 30 seconds while open.
- Local queue: `%LOCALAPPDATA%\RestedRealmCollector\Companion\queue.sqlite3`. On 25 September, it held 331 observations and zero pending. The first 283 were safely imported and the closed game save was rolled over with a private backup. The next WoW load saved 48 new records, sequence 284 through 331, with collection enabled and full text capture on. All 48 uploaded. The addon's save had 48 records after that reload; the companion queue retained all 331.
- Production intake: migrations `054_collector_intake.sql` and `055_collector_claim_evidence.sql` are applied. Three production web replicas and the worker were healthy after deployment. The server held 331 observations from one account and one device, with 90-day retention for raw account-linked observations. Account deletion and raw expiry cascade to claim evidence. The owner-only `/account/collector` page can issue one-use pairing codes and revoke devices.
- The first 331 records produced private reward evidence for choice item IDs, XP and coin. Quest 426 has `[3447, 3834]`, 875 XP and 450 copper for a Horde Scourge Paladin at level 8 in enUS on build 70009. Repeated observations from one account still count as one contributor. The public quest page shows a separately labeled owner observation and keeps its Classic coordinates labeled as approximate. An automatic corroboration panel requires two independent accounts reporting the same field, build and character context with no conflict. The owner-only pilot cannot meet that threshold yet.
- The production browser check for quest 426 passed on desktop and phone. The isolated site build passed TypeScript, its optimized build and 226 integration tests. The companion has 11 passing Python tests. The previous web image is tagged `azerothindex-web:before-claims-20260925` for rollback.

## Data path and trust boundary

`addon event -> SavedVariables on logout or /reload -> companion parser -> local SQLite queue -> HTTPS private batch intake -> raw observation -> per-field evidence -> scoped public report`

The addon keeps at most 1,500 records in its save and stops collection at the cap rather than discarding older records. The companion imports complete saves transactionally, deduplicates by source and sequence, and retains observations across offline sessions. Rollover requires WoW to be fully closed, verifies every record was copied and keeps a backup. The uploader strips full NPC and quest prose from the transport while preserving it in the local queue. It sends only to `https://restedrealm.com/api/collector/`, refuses redirects, and acknowledges an observation only after the server reports accepted or duplicate. A failed or ambiguous upload leaves it pending.

The server checks upload credentials, size, structure, digest, product, build, locale and prohibited prose fields. A checksum establishes transport consistency and retry identity. Neither it nor account pairing proves a player really saw the claimed game event; SavedVariables and companion code can be edited. Public reports therefore need per-field context, independent account support, conflict handling and source labels. There is no blanket Forever approval badge. Map pins, player positions and Classic quest-giver coordinates are different evidence types; do not conflate them.

The website's current claim extractor covers only `quest_objectives` reward choice IDs, XP and money. NPCs, vendors, dialogue, map geometry, loot relationships, quest giver positions and other observation kinds remain private research data, not automatic public facts. Full prose publishing still needs a separate rights and privacy review. Contributor names should remain private by default; any public credit requires a separate opt-in and an alias choice.

## Key files and checks

| Area | Files | Check |
| --- | --- | --- |
| Addon capture | `addon/RestedRealmCollector/Collector.lua`, `.toc` | `tests/collector_smoke.lua` with a Lua runtime; then an actual Forever session and `/rrc status` |
| Companion import and queue | `companion/restedrealm_companion.py`, `save_parser.py`, `gui.py` | `python -m unittest discover -s tests -p 'test_*.py' -v` |
| Pairing and upload | `companion/uploader.py`, `windows_credentials.py`, `UPLOAD-CONTRACT.md` | Mock transport tests, then a clearly labeled synthetic batch and duplicate retry before a new deployment is trusted |
| Windows app core (Rust) | `app/core/src/*.rs`, `app/README.md` | `cargo fmt --check`, `cargo clippy`, `cargo test` in `app/`, then `python tests/parity_check.py` |
| Website intake | `/opt/restedrealm/src/server/collector.ts`, `src/app/api/collector/`, migrations 054 and 055 | TypeScript, production build, isolated database integration tests, account isolation and browser checks |
| Public evidence | `/opt/restedrealm/src/lib/collector-claim-rules.ts`, `src/server/collector-claims.ts`, `src/app/quests/[id]/page.tsx`, `docs/COLLECTOR-EVIDENCE-POLICY.md` | Conflict, distinct account, build rollover, deletion cascade and desktop/phone rendering checks |

Never run an active scanner or experimental migration against production simply to test Collector changes. Use an isolated checkout and staging database, then migrate and roll the production web replicas one by one after validation. The production Compose command needs explicit `-f compose.yaml --profile production`; the default override may start development services. Keep prior images available for rollback and check all three replica health states.

## Blizzard rules and rights review

The [official WoW UI Add-On Development Policy](https://eu.forums.blizzard.com/en/wow/t/wow-user-interface-add-on-development-policy/1642) requires addons to be free of charge, fully visible and unobfuscated, free of advertising and in-game donation requests, and free of behavior that harms realm or player performance. It also requires compliance with the WoW terms and EULA and allows Blizzard to disable addon functionality. This is why the in-game Lua source belongs in a publicly viewable repository before broad distribution. A private GitHub repository alone does not meet the visibility rule. The companion is a separate desktop program, but keep its behavior transparent and limited to local save import and opt-in upload.

Blizzard's [EULA](https://www.blizzard.com/en-us/legal/08b946df-660a-40e4-a072-1fbde65173b1/blizzard-end-user-license-agreement) governs game access and reserves rights in game content and data. Do not assume capturing text in a player's private local save grants permission to republish complete quest or NPC dialogue, artwork, audio or other game content. Keep full text local while rights are reviewed. Avoid memory reading, packet inspection, unattended gameplay automation, protected API workarounds and any collection that burdens the client. Test against the actual Forever build because UI APIs and allowed behavior can change. These are implementation guardrails, not a claim that Blizzard approved the addon or that this document is legal advice.

## Next work

1. The source is MIT licensed (see [LICENSE](../LICENSE), added 25 September 2026). The license covers this project's code only, not Blizzard game content or data. The local queue, game saves, credentials, backups and staging screenshots must remain private.
2. Finish the Windows app in `app/` (see [COMPANION-APP.md](COMPANION-APP.md) for status): publish the first release ([RELEASING.md](RELEASING.md)), then Battle.net discovery, the website settings file and the in-game status line. The Python prototype in `companion/` is kept as the reference and should not run next to the app.
3. Verify new Forever builds and addon API changes with a real session. Keep build and character context attached to every observation.
4. Add reviewer decisions, a conflict queue, catalog validation and explicit publication rules for more claim types before expanding automatic website updates.
5. Quest text: decided on 25 September 2026 (collect and show as Blizzard content, sanitized in the addon, published when two players agree). NPC dialogue is stored privately and has no public display yet.

## Git and naming

The Collector source is in the public `DiigiiHD/restedrealm-companion` repository, so the addon can be inspected without exposing the private website repository. The website GitHub repository is named `DiigiiHD/restedrealm`. That rename does not rename the VPS directory, Docker Compose project, database, service names or internal import identifiers. Change those only as a separate, tested infrastructure task; they are not required for the public brand to be RestedRealm.
