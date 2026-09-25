# RestedRealm Collector, Forever probe

Start with [the project handoff](docs/PROJECT-HANDOFF.md) for current architecture, deployment state, Blizzard policy and next work. Coding agents should also read [AGENTS.md](AGENTS.md).

Source repository: [DiigiiHD/restedrealm-companion](https://github.com/DiigiiHD/restedrealm-companion). The website has a separate private [RestedRealm repository](https://github.com/DiigiiHD/restedrealm).

The Forever addon records observations from normal play (on by default since 0.1.14; `/rrc off` pauses it). The Windows companion keeps them in a local queue, and the owner can now upload structured observations to RestedRealm after account pairing. Full NPC and quest prose remains local. The site keeps raw account-linked uploads for 90 days and shows only scoped, corroborated quest reward reports automatically.

## Installed location

`C:\Program Files (x86)\World of Warcraft\_classic_beta_\Interface\AddOns\RestedRealmCollector`

Source files are in `addon/RestedRealmCollector/`. The addon uses Forever interface `16001`, following the installed beta-compatible addon. The beta may change this value; check after each update.

## In-game trial

1. Start or restart Forever. Check that **RestedRealm Collector** appears in the AddOns list.
2. Type `/rrc status`. It should say collection is off.
3. Collection is on from the first login (since 0.1.14). Type `/rrc off` to pause it and `/rrc on` to resume; the choice is remembered. Visible NPC and quest text is saved locally by default and never uploaded. Use `/rrc text off` if you want only IDs and short labels.
4. Talk to one NPC with gossip, open one vendor, inspect one quest and open one loot window. Normal interaction only; the addon never clicks or plays for you.
5. Type `/rrc status` to check the saved count. Log out normally or type `/reload` to make the game save the data. Check `WTF\Account\<account>\SavedVariables\RestedRealmCollector.lua` under `_classic_beta_` after WoW has completed the save. Do not share the file unreviewed.
6. Restart Forever and check `/rrc status` again. The count should survive. This is especially important because beta SavedVariables behavior has changed recently.

Commands: `/rrc on`, `/rrc off`, `/rrc status`, `/rrc scan`, `/rrc text on`, `/rrc text off`, `/rrc clear`.

Collection is off on first load. Once enabled, full visible NPC and quest text stays local until a separate publication review. The addon holds at most 1,500 records and stops recording when full, preserving earlier observations. It does not intentionally collect chat, player identities, account IDs, full inventory, credentials or continuous movement history. Game dialogue could include a character name, so the future uploader must review and redact text before sending it. Locations are the **player's point at interaction**, not a proven exact NPC or object spawn. `loot_window` means an item was visible in a loot window, not that it was acquired. Some IDs or fields may be absent until verified in this Forever build.

## First live result, 25 September 2026

The first session saved four records and the count survived `/reload`. Version 0.1.1 fixed vendor details using `C_MerchantFrame.GetItemInfo`; a live visit then saved item prices and stock fields. A later session reached 19 records, including a quest detail screen and a loot window with Duskbat Pelt item 2876. No observation of The Chill of Death quest objective was captured, and the addon has not linked the loot to that quest.

Version 0.1.2 fixes a GUID parser error: earlier records used the zone field as the NPC/loot-source ID. On `/reload`, it removed those invalid IDs from old records and marked why; dialogue, vendor items, quest details, map positions and the loot item remain. A private copy of the pre-migration SavedVariables file is under `%LOCALAPPDATA%\RestedRealmCollector\Backups`. New records have `identitySchema = 2`. Lua syntax and a local migration/merchant/loot smoke test passed. Live observations reached 25 records, including six new loot windows. One new loot window contains Duskbat Pelt item 2876 with creature source ID 1553, confirming that source/item pair was captured on this Forever build. The addon has still not linked it to The Chill of Death quest. No upload has occurred.

## Version 0.1.3 beta probes

This update records changed active quest objectives after opt-in and quest-log updates, including quest ID, title, objective type, visible text and progress counts. It records quest accepted/removed/turned-in events, quest-giver lists, required and reward items from quest dialogs, trainer offerings, flight-map nodes and non-copper vendor costs when the beta exposes them. A one-second delay coalesces quest-log events; unchanged objectives are not saved again. `/rrc scan` queues a manual objective/map-pin snapshot.

For quest maps, it attempts to save an available quest **pin** with map ID and normalized coordinates. This is not the blue shaded area's shape. The public Forever addon API has not yet been shown to expose the polygon points, so no area geometry is claimed. A quest-item relation remains a candidate until the observed objective, item and progress change can be reconciled. Trainer and taxi calls are guarded because their availability and field coverage still need a live Forever test.

After installing an update while WoW is open, use `/reload`, open a quest area on the map, then type `/rrc scan`. After another `/reload`, inspect the saved metadata for quest ID, objective progress and any map pin. No website upload exists yet.

## Version 0.1.4 follow-up

The first 0.1.3 live session captured a completed quest (`Forsaken Duties`, ID 359) and its turn-in, plus the offered `Hides for the Forsaken` quest (ID 97558). The latter quest detail directly recorded NPC Shelene Rhobart (ID 3549) at map 1420, approximately 65.44, 60.07. Earlier `Forsaken Duties` and `Return to the Magistrate` details directly recorded Deathguard Linnea (ID 1495) nearby. The NPC subtitle `<Journeyman Leatherworker>` was not captured. The session did not capture the displayed quest level or any active quest-objective snapshots, and the accept event lacked an ID. Version 0.1.4 added the `C_QuestLog.GetInfo` path and resolved these quest-log gaps in the next live save.

## Version 0.1.5 reward follow-up

The 0.1.4 live save confirmed `Hides for the Forsaken` (ID 97558) at level 11 with acceptance and three item objectives. The quest detail saved 511 characters of quest narrative and 93 characters of objective text. It did not save item reward choices, XP or money, because the previous probe asked the wrong reward-count API and only recorded XP/money on the completion screen. Version 0.1.5 uses the separate required-item, fixed-reward and choice-reward counts, plus both dialog and quest-log XP/money APIs. The 25 September live save verified 650 XP, 300 copper, and three choice items for this quest. It records reward type so the site can render the appropriate localized heading. The display phrase `You will be able to choose one of these rewards` is interface copy, not a field asserted to be in the quest narrative. No upload has occurred.

## Version 0.1.6 coverage audit

The first probe omitted ordinary quest reward fields and quest-greeting IDs. [The coverage audit](COVERAGE-AUDIT.md) lists each expected field, its live evidence, and remaining gaps. Version 0.1.6 added guarded quest IDs, levels and availability flags to legacy quest greetings and reports whether the related APIs exist in Forever. It also tries `C_QuestLog.GetQuestsOnMap` for a quest pin when the icon API is unavailable. These paths are locally tested but have not yet been seen live. No repeated quest acceptance is requested for this audit.

## Version 0.1.7 broader probe

This pass adds guarded NPC subtitle/level/type capture, zone and subzone context, quest-start item IDs, description and objective prose from existing quest-log entries, currency/spell/title/honor reward probes, selected quest-log reputation evidence, item variant strings and loot-slot-clear events. It raises the local text limit to 8,192 characters and records when text was truncated. Reputation amounts are stored raw with an unverified scale, and a cleared loot slot is not described as confirmed acquisition. Local syntax and smoke checks passed; these additions need the single live sample in [the coverage audit](COVERAGE-AUDIT.md) before they can be called Forever-verified. No upload has occurred.

After the next game save, `tools/audit_save.lua` can summarize record kinds, collector versions, available APIs and the fields for one quest ID without printing names or prose. It is a local, read-only checker for this trusted SavedVariables file; it does not upload data. The existing 69-record save still reports version 0.1.5 until WoW loads and writes the new addon version.

## Version 0.1.8 collection paths

This pass adds bounded sightings when a player explicitly targets a creature or object, guarded records of gossip selection API calls and confirmation prompts, more offer/option flags, profession openings and recipe learning/viewing, and quest map hints such as map ID and blob count. Loot records now preserve exposed quest IDs, quest-item flags and multiple source IDs, and read from loot-ready, open and changed events without duplicating an unchanged window. The addon checks only that looted item's bag count shortly after a slot clears; it records a positive delta without storing full inventory. A sighting is the player's location while targeting, not the entity's exact spawn. A gossip selection record proves an API call, not necessarily a deliberate click. The recipe path records observed output and reagent IDs only when the interface exposes them; it does not scan the full recipe book. See [the coverage audit](COVERAGE-AUDIT.md) for the next single live verification run and remaining limits.

## Version 0.1.9 live-gap follow-up

The 0.1.8 save has 153 records, zero drops and zero handler errors. It confirms the Hides for the Forsaken reward choices, XP, money, quest text and map pin. It also exposed a missing NPC subtitle and no bag-count increase after two cleared loot slots. Version 0.1.9 adds a private tooltip fallback for bracketed NPC subtitles, checks the specific item on bag updates and for up to five seconds after a loot slot clears, and can record player position when the client reports crossing a quest blob edge. A slot-clear sequence links any later positive bag delta to the same observation. Since the live build reports no selected-recipe API, a guarded classic profession reader also captures visible recipe rows, outputs and required reagents on profession open/update. The map evidence is a pin, sparse inside/outside samples and optional edge points, never a claimed exact shaded boundary. The next live save is needed to verify these paths; [the coverage audit](COVERAGE-AUDIT.md) lists the precise checks.

## Version 0.1.10 subtitle diagnostic

The latest 0.1.9 save confirms three quest-area edge events and per-quest inside/outside values, with no handler errors. It contains no new loot or profession record. Twenty NPC interactions yielded no subtitle, but none is confirmed to have shown a bracketed title. Version 0.1.10 stores a small set of NPC-only tooltip lines locally when both subtitle parsers miss, so a future live example can show whether the client omitted the title or the parser needs a different format. These lines remain private SavedVariables data.

## Version 0.1.11 NPC role and gossip selection correction

The 0.1.10 save proves that the client returns Shari Stilwell's visible `<Paladin Trainer>` title as plain `Paladin Trainer` on the second tooltip line, before `Level 16`. Version 0.1.11 accepts that pattern when the following line matches the NPC's level and migrates the existing diagnostic records. The same save shows Burgess's `The inn` option has real ID 95466 and `orderIndex=2`, while the earlier selection hook mislabeled the raw index `2` as an option ID. Version 0.1.11 stores both values with their meanings, and migrates older indexed selections only when a recent matching gossip menu resolves them. A read-only run against the actual 229-record save recovered all 12 indexed selections, including the inn and Shari's training choice. The addon also probes the client's gossip POI APIs for a destination marker after direction conversations. This new path is locally tested but not live-verified. Sitting on a chair generated no collector record; this probe does not track character posture or object use without a client event or object ID.

The next saved run reached 243 records with zero drops or handler errors. New 0.1.11 records contain Shari's `Paladin Trainer` role and merchant roles for Eliza Callen and Abigail Shiel. Deathguard Cyrus's `A class trainer` and `Paladin` selections now carry the real option IDs. A gossip destination POI was not returned, and no trainer-service or profession window record was produced in that run. See [the coverage audit](COVERAGE-AUDIT.md) for the live evidence and remaining limits.

## Version 0.1.12 trainer diagnostic

The 0.1.11 save contains Shari's training choice but no trainer-service record. The old trainer handler silently returned when the client exposed zero services, so the saved data cannot show whether a trainer event fired with an empty list. Version 0.1.12 records a bounded `trainer_window` observation on empty `TRAINER_SHOW` or `TRAINER_UPDATE` events, while preserving the existing service reader when entries are available. This path passed the local smoke test and needs a live trainer window to verify it.

## Version 0.1.13 trainer cap and loot-quest audit

The 0.1.12 save confirms that Shari's trainer UI first returned zero services, then 142; the old 100-row limit truncated 42. Faruza's window similarly returned zero, then five, and all five were saved. Version 0.1.13 raises the trainer cap to 250 and records whether the list was truncated. The same save captured Duskbat Pelt 2876 and Duskbat Wing Membrane 278242 from Greater Duskbat 1553, both slot clears, and one-second-later objective increases for The Chill of Death and Hides for the Forsaken respectively. `tools/audit_loot_quest_links.lua` derives conservative candidate links from those facts without uploading data or asserting a direct quest ID that the loot API did not return. The Red Skeletal Horse was recorded as creature 267357 with dialogue. Spellbook and personal character tabs are not collected by this world-data probe; the privacy and data-scope decision is described in [PREWORK.md](PREWORK.md).

The Windows companion queue is in [`companion/`](companion/README.md). It safely parses completed SavedVariables as data and preserves observations in private local SQLite; it never executes the save as Lua. The closed game's save yielded 283 observations with zero addon drops; a guarded rollover freed its 1,500 addon slots while keeping a private backup. The owner manually uploaded all 283 on 25 September 2026. They remain in the local queue with zero awaiting upload.

The owner-only website intake for account pairing, private uploads, revocation, export and 90-day retention is live. The owner PC is paired. A synthetic batch was accepted once, deduplicated on retry and removed. Migration 055 adds private per-field reward evidence. The first live session after rollover added 48 records; the server now has 331 observations, and the local queue has zero pending. See [`site-patch/`](site-patch/README.md) for the deployed path and publication limits. The companion's automatic upload checkbox remains off until explicitly enabled.

The API choices follow Blizzard-generated `classic_beta` interface documentation for [gossip](https://github.com/Gethe/wow-ui-source/blob/classic_beta/Interface/AddOns/Blizzard_APIDocumentationGenerated/GossipInfoDocumentation.lua) and [merchant events](https://github.com/Gethe/wow-ui-source/blob/classic_beta/Interface/AddOns/Blizzard_APIDocumentationGenerated/MerchantFrameDocumentation.lua), with runtime guards because the installed Forever beta is the final authority. The addon is free, visible source, has no ads or donation prompt, and uses only player-initiated observations.

## License

The addon, companion, tools and tests in this repository are released under the [MIT License](LICENSE). The license covers this project's own source code only. World of Warcraft game content and data, including any quest or NPC text a local save captures, remain Blizzard's and are not licensed here. Local queues, game saves, credentials and backups stay private and are never part of the repository.
