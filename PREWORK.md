# RestedRealm community client: collection plan

Status: active prototype, 25 September 2026. The addon and local Windows companion queue exist. Account pairing and private upload intake passed isolated staging checks, but neither is on the production website and companion network use remains disabled. No public claims are implemented.

## Decisions and goal

- First supported game: World of Warcraft: Forever in `_classic_beta_` on Windows. Keep product and build in every record so later game flavors cannot be mixed in by accident.
- Uploads require a RestedRealm account. Discord remains the main website sign-in; the existing Battle.net connection is optional. The companion must never ask for a Discord or Battle.net password or reuse a provider access token.
- After a one-time upload opt-in, the future companion should upload automatically after a completed game save. Keep an always-visible pause control and a manual `Upload now` action.
- Contributor credit is anonymous by default. A person may opt into public credit, choose their Discord display name or a separate public name, preview it, and turn credit off later. Public names are profile settings, not prompts on every upload.
- The first prototype may record full visible NPC and quest text locally after collection opt-in. Publishing long prose stays on hold until rights review.
- Make normal play the source of observations. The addon records only information exposed by Blizzard's permitted addon API during actions the player actually takes. It does not automate gameplay, query hidden state, read process memory or inspect network traffic.
- The client should help answer: **What was actually seen in Forever, where, under which conditions, and on which build?** The current static extraction and Classic references answer different questions.

## What already exists

The Windows scheduled task `AzerothIndex Forever Client Sync` checks the installed build hourly. It extracts client files through WTL and sends immutable, checksummed bundles to the VPS. It is a first-party, owner-operated pipeline, not a crowdsourced addon uploader. `docs/CLIENT-SYNC.md` and `docs/CLIENT-V7-WEBSITE-GRAPH.md` on the VPS are its current handover. On this PC, the task points to `D:\azerothindex\scripts\client-sync\sync.py`. The proposed `restedrealm-client` folder is empty; the Forever AddOns folder currently has TomTom only.

The latest audited site document describes build `1.60.1.69977`, 146 available V7 raw tables and a 51,822-edge client graph. The local Forever installation showed build `1.60.1.70009` during this review. Recheck the delivered build and its coverage before implementation. A delivered bundle does not automatically update every public catalog page.

RestedRealm already separates official client rows from an imported QuestieDB Classic reference. The Classic reference contains quest, NPC, object, item-source and approximate spawn leads, but matching IDs do not prove that those facts are true in Forever. Current docs specifically identify missing trustworthy vendor offers, NPC spawns, loot tables, quest prose and per-quest reputation in the static client extraction. NPC and object pages are still pending. Community observations should be a **third, separately labelled source**, not an overwrite of either existing source.

## Proposed product shape

1. **Small in-game addon.** Subscribe to a bounded set of non-combat events. Snapshot the relevant visible UI state when the player talks, accepts a quest, opens a vendor, loots or visits a place. Keep identifiers and compact observations in SavedVariables. No persistent on-screen panel is required; offer a small status/options page and a clear pause/reset control.
2. **Optional Windows companion for reliable upload.** Install or update the addon, read only its SavedVariables after a stable game save, copy new observations into a durable local queue, and upload over HTTPS when online. A player can close the companion while playing; it can process saved data later. Keep the addon useful without the companion, but do not promise unlimited retention in a bounded SavedVariables file.
3. **Website pairing.** The companion opens `restedrealm.com` in the system browser. The player signs in with the existing site flow, then authorizes this device with a short-lived, single-use pairing code or equivalent browser handoff. Issue a revocable RestedRealm upload credential scoped to contribution, not a Discord or Battle.net token.
4. **Server intake.** Authenticate, validate and store raw submissions separately from published facts. Deduplicate retries, associate records with an account privately for abuse handling, and feed a review/aggregation job. A verified observation or consensus can create a versioned public claim with a source label, build, locale and last-seen time.

The addon cannot itself send HTTP to the VPS through normal WoW addon APIs. The companion is the bridge, following the same broad addon plus uploader pattern documented by [Wowhead Client](https://www.wowhead.com/client). Unlike the existing hourly WTL job, this client processes observations made during play.

## Collection matrix

`Probe` means the field or event must be tested against the installed Forever beta before promising it. Even a familiar Retail or Classic API can differ here. `Observe` means capture only when the player naturally opens or completes the interaction. All text is locale-specific. All coordinates need a map ID, instance context and uncertainty; a point sampled after the interaction is not automatically an exact spawn.

| Priority | Observation to collect | How it helps RestedRealm | Limits and verification |
| --- | --- | --- | --- |
| P0 | Product, build, locale, region, realm, event time, addon schema version | Prevent cross-build and cross-realm mixing; show freshness | Use client-provided values where available; server receipt time is separate. Do not publish account or character identity. |
| P0 | NPC encountered: numeric ID if exposed, name, type, reaction, map/zone and position at interaction | First Forever-observed NPC evidence and map overlays | Probe GUID/ID availability. Names alone never establish identity. Capture a point, not a movement trail. |
| P0 | Gossip/dialogue: NPC ID, visible text, option text/order/type, selected branch | Searchable conversation paths and conditions | Probe text and option APIs. Record only branches the player sees; never infer unseen branches. Long prose needs publication-rights review. |
| P0 | Vendor interaction: NPC ID, offered item IDs, quantities, currency/cost, limited stock if exposed | Real Forever vendor inventory instead of Classic vendor leads | Probe merchant APIs. Record level, faction and reputation context where relevant; price can vary by discounts and conditions. A missing offer in one visit does not prove absence. |
| P0 | Quest offer, acceptance, objectives, progress, completion and turn-in: quest ID, text, giver/finisher, required targets, rewards, choice rewards, XP, money and reputation where visible | Fill the largest static-client gaps and verify quest-to-NPC/object/item links | Probe each stage separately. Preserve exact reward choices and eligibility context; do not infer quest availability from a static ID. Text publication requires rights review. |
| P0 | Item obtained: item ID/link and variant, source interaction, quantity and map context | Evidence for acquisition paths and item variants | Capture source before loot UI closes. Do not call an observed drop a drop rate. Do not upload a player's inventory contents. |
| P0 | Game object interaction: numeric ID if available, name, location, action and resulting loot/quest change | Forever-observed objects and gathering points | Probe object ID and position APIs. Distinguish object location from player location and note uncertainty. |
| P1 | Trainer services: NPC, taught spell/skill/rank, price, level and class requirements | Real trainer lists and learn costs | Probe trainer API in Forever; static spell/talent tables remain the canonical definitions. |
| P1 | Profession actions: recipe learned, ingredients displayed, crafted output and skill requirement | Connect profession, recipe, reagent and result pages | Observe opened profession UI and completed crafts only. Do not infer a recipe source from possession. |
| P1 | Reputation change and gated access: faction ID, visible amount, related quest/action, before/after standing | Verify Forever reputation rewards and vendor conditions | Attribute only when the causal action is unambiguous. Avoid publishing an amount from a noisy combined event. |
| P1 | Taxi, transport and entrances: discovered node, route, price, destination and instance entrance | Travel planning and map connections | Probe available APIs. A route price may depend on standing; no continuous location logging. |
| P1 | Achievement/collection acquisition: achievement, mount, pet or toy ID and source action | Provenance for collection pages | Static client tables already define many entities; record only the observed acquisition path and conditions. |
| P1 | World/event state: visible temporary NPC, vendor, quest or object availability and dates | Build-aware and time-aware availability | Require repeated observations and event context. Do not label absence from one visit as removal. |
| P2 | Encounter or boss loot and mechanics seen through permitted, non-secret APIs | Later instance guides and source evidence | Keep out of the first release. Forever inherits modern addon restrictions; do not build combat decision tools or work around secret values. |
| P2 | Auction or market samples, if an explicit, low-impact API path exists | Historical pricing context | Separate opt-in and feasibility/policy review. Avoid bulk scanning or player identifiers; market prices are realm/time-specific. |

### Talents, spells and character data

The owner-operated client export already carries class, talent and spell structure. The community addon should **not** ask players to upload full talent builds, gear, inventory, gold or play history by default. Those are personal data and add little to the content gap. A later opt-in module could verify a displayed trainer rank, learned spell cost or an apparent client-data mismatch. It must never process protected combat state to recommend actions.

### Deliberately excluded from collection

Chat, whispers, guild/officer messages, friends lists, party rosters, other players' names or GUIDs, raw account identifiers, screenshots, passwords, OAuth tokens, full character inventories, continuous movement traces, memory reads, packet captures and automated interactions. The companion should read only the RestedRealm addon's SavedVariables, not other addons' files.

## Entity matching and trust

An observation is an event, not a canonical fact. Store at least: observation type, product, build, locale, realm/region, optional phase or instance context, observed time, server receipt time, source account held privately, addon/companion version, game IDs, map ID/coordinates and uncertainty, normalized fields, and a content digest for idempotency. Keep a raw, versioned record and a separate public projection.

- Join item, quest, spell and faction IDs to the existing official client catalog only within the correct product/build. Do not join an NPC to the client `Creature` presentation table or to a QuestieDB NPC solely because IDs or names match. Record a validated mapping and its evidence first.
- Preserve edges as `observed_in_game` with build, locale and event context. Continue to label QuestieDB edges as Classic reference and official client edges as client-derived. Display the difference on pages.
- Treat text, vendor stock, prices and rewards as conditional. Capture faction, class, level, reputation tier, difficulty, phase and relevant quest state only when available and necessary. Otherwise label the condition unknown.
- Make a single sighting publishable as a dated observation only after validation. Stronger claims such as a stable vendor inventory, spawn region or drop frequency need independent confirmations, denominator data where applicable, conflict handling and moderator review.
- A missing event is not negative evidence. For drop rates, the denominator must be eligible kills/openings, not only successful loot. Do not publish percentages from sparse or biased samples.
- Let official build changes invalidate or hold older projections for review rather than silently carrying Forever observations forward. Show `last seen on build ...`, not `currently available`, until reconfirmed.

This can differentiate RestedRealm through transparent, build-specific evidence, a visible history of changes, contextual dialogue/quest paths, and honest uncertainty. It is an opportunity, not a claim that Wowhead lacks every feature. Wowhead's own client already documents NPC, vendor, quest, object, loot and NPC-message tracking.

## Retention and the one-week upload case

WoW writes addon SavedVariables on normal save points such as logout or UI reload, not continuously to an external uploader. The first prototype must verify exact Forever beta persistence across logout, `/reload`, a cold restart and a client update. A recent [Forever beta bug report](https://us.forums.blizzard.com/en/wow/t/wowf-beta-addon-savedvariables-appear-to-write-correctly-to-disk-but-are-not-restored-at-startup/2356559) reported SavedVariables restore trouble and later reported recovery. That is a test lead, not a stable API guarantee.

The addon should retain compact, deduplicated observations in bounded segments and show a simple storage status. The companion should wait for a stable completed save, copy new segments into its own local durable queue, and acknowledge uploads only after the server returns batch IDs and hashes. The queue survives restarts and offline periods, retries with backoff, and can upload a week later. Do not delete local queued data on a timeout or ambiguous response. The companion must never edit the live SavedVariables file while WoW is using it. If only the addon is installed and its bounded storage fills before the player uploads, disclose that older observations may be dropped.

Give users `Pause collection`, `Preview pending data`, `Upload now`, `Forget local data`, `Disconnect device` and clear last-upload/error states. A browser sign-in or pairing should be needed once per device, with a revocable credential. Default to collection on only after an explicit onboarding choice; default upload behavior needs the owner's final preference below. The desktop app should stay out of the game process and use little CPU while idle.

## Intake and publication safeguards

- New `/api/client-uploads` style endpoint, name to be chosen in design: account-bound upload scope, TLS, revocation, batch size and decompression limits, schema/version validation, quotas and rate limits. No public write directly to `gd_rows`, `source_records` or website pages.
- Treat every submitted ID, text and coordinate as untrusted. Bound lengths, normalize locale and product, reject path or archive traversal, do not execute Lua from submissions, and escape text on output. Record idempotency keys so retries do not double-count evidence.
- Quarantine malformed, impossible, spammy or conflicting observations. Rank corroboration by independent accounts and builds, not by raw event count from one installation. Keep an audit trail for moderator decisions and source corrections.
- Store contributor account identity separately from the public observation. Public pages can show aggregate evidence without naming the player. Define account-deletion behavior, raw retention and export before launch.
- Do not let crowdsourced data overwrite the owner's curated writing, official client extraction or Classic references. The website's existing worker and versioned candidate/review pattern should own the public projection.
- Monitor upload success, queue age, beta build coverage, API probe failures, rejected batches, publication lag, addon Lua errors, SavedVariables size and event-handler cost. Set budgets through measurement on the beta client rather than guessing a safe size.

## Blizzard and content rules to verify before public release

Blizzard's [UI Add-On Development Policy](https://eu.forums.blizzard.com/en/wow/t/wow-user-interface-add-on-development-policy/1642) requires free distribution, visible unobfuscated addon code, no in-addon ads or donation requests, and no negative performance impact. It reserves the right to disable addon functions. The addon should use only documented/permitted UI APIs and should not automate gameplay. Blizzard describes modern [secret combat values](https://news.blizzard.com/en-us/article/24246290/combat-philosophy-and-addon-disarmament-in-midnight) that addons may display but not process; test the exact Forever beta behavior, and never work around those limits.

Blizzard's [EULA](https://www.blizzard.com/en-us/legal/08b946df-660a-40e4-a072-1fbde65173b1/blizzard-end-user-license-agreement) restricts unauthorized data mining and automated game control. The official addon policy and Wowhead's existing client show a common addon-plus-uploader pattern, but they do not by themselves grant permission to republish unlimited game text or assets. Get a focused terms/content-rights review before publishing a large dialogue or quest-prose corpus, and keep the design able to suppress those fields while still collecting IDs and relationships. Do not copy Wowhead's database or prose.

## First implementation gates

1. **Beta API spike:** install a tiny, visible-source test addon in `_classic_beta_`. Confirm its TOC/interface value, SavedVariables round trip and performance. For each P0 row, record the exact event/API, returned fields, whether values are secret or unavailable, and a sanitized sample. Do not build the uploader on assumed API coverage.
2. **Data contract:** write versioned observation schemas and fixtures for NPC, gossip, vendor, quest, loot and object events. Define ID extraction, coordinate uncertainty, locale, conditions and source labels. Compare against the latest delivered official build and existing QuestieDB namespaces.
3. **Local durability:** implement a bounded addon log and a companion queue. Prove two game sessions, game restart, one-week offline delay, retry, duplicate upload and interrupted save without loss or double count.
4. **Private pilot:** pair the owner's RestedRealm account, upload to a staging intake, inspect a preview and reconcile records against existing item/quest pages. Measure addon CPU/memory and SavedVariables growth while playing normally.
5. **Review and public pilot:** moderation, contributor controls, privacy/terms review, revocation and data deletion, signed distribution/update plan, and build-aware public projections. Only then invite contributors.

Acceptance for the first useful release: the player can install once, play without interruption, close the game, sign in to RestedRealm, review and upload a week's queued observations, and see accepted contributions as dated Forever observations without confusing them with Classic references or static client facts.

## Questions still open

1. Which first five interactions should be tested in the beta? Quest giver, vendor, gossip NPC, world object and loot source are my recommendation. Are there specific Forever-exclusive zones or quests you want in the pilot?
2. Is Windows-only companion support acceptable for the first release? The addon data contract can remain portable for a later macOS/Linux companion.
3. Resolved for the private pilot: the owner chose 90 days for raw, account-linked observations after upload. Account deletion removes them sooner. The companion's local queue is separate and stays until the player clears it.
4. Who reviews conflicting observations and build transitions? The current admin update workflow is the likely place, but it needs a dedicated community-data queue and thresholds.

## Source notes

Local: Windows scheduled task action, the installed `_classic_beta_` addon folder and `.build.info`, and the empty `restedrealm-client` folder, checked 25 September 2026.

VPS: `/opt/restedrealm/docs/CLIENT-SYNC.md`, `CLIENT-V7-WEBSITE-GRAPH.md`, `QUESTIEDB-INTEGRATION.md`, `GAME-DATA-PAGES.md`, `EXTERNAL-APIS.md`, and `HOW-WE-BUILD.md`, read 25 September 2026. These docs distinguish deployed features from plans and contain the build-specific coverage quoted above.

External: [Blizzard Add-On Development Policy](https://eu.forums.blizzard.com/en/wow/t/wow-user-interface-add-on-development-policy/1642), [Blizzard EULA](https://www.blizzard.com/en-us/legal/08b946df-660a-40e4-a072-1fbde65173b1/blizzard-end-user-license-agreement), [Blizzard addon capability explanation](https://news.blizzard.com/en-us/article/24246290/combat-philosophy-and-addon-disarmament-in-midnight), [Wowhead Client](https://www.wowhead.com/client), and the linked Forever beta SavedVariables report, checked 25 September 2026. The exact Forever API surface remains to be measured locally.
