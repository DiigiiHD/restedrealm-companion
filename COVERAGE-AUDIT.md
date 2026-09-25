# Forever collector coverage audit

25 September 2026. Source version: `0.1.13-probe`. This is a local collection probe, with no uploader or website publication. The latest save has 266 records, including 23 written by 0.1.12, with zero dropped records and zero handler errors. The trainer diagnostic and quest-item loot paths were exercised live. Version 0.1.13 raises the trainer-service cap after a live truncation; it has not yet been exercised live.

The first probe captured events before it had an explicit field contract. That is why ordinary reward data was missed. A saved count only proves that records were written; it does not prove that a quest record is complete. Future field additions should start from this matrix and a representative saved record, with missing values marked explicitly.

Quest objective, text and reputation deduplication is now scoped to client build and locale. A new Forever build can therefore create fresh evidence even when a quest's visible values are unchanged.

## Hides for the Forsaken, quest 97558

| Field | Status | Evidence and limit |
| --- | --- | --- |
| Quest ID and title | Verified live | Quest detail, acceptance and quest log agree on ID 97558. |
| Level | Verified live | Quest-log entry reports level 11. |
| Giver | Verified live | Quest detail identifies Shelene Rhobart, NPC 3549. |
| Giver position | Verified live as player position | Map 1420, approximately 65.44, 60.07 at interaction. This is not an exact spawn coordinate. |
| NPC subtitle | Verified in 0.1.11 saved records | Shari Stilwell, NPC 246152, has `Paladin Trainer` with `subtitleSource=tooltip_role_before_level` in a new gossip record; Eliza Callen has `Leather Armor Merchant`, and Abigail Shiel has `Trade Supplies`. Older diagnostic records also migrated, including Shari and the Zeppelin Masters. The client returns titles as plain text before the level line. Shelene Rhobart has not been revisited with this parser. |
| Quest and objective prose | Verified live, local only | Detail record has 511 and 93 characters respectively. Full text awaits publication rights review. |
| Accept event | Verified live | Quest ID was recorded. |
| Active objectives | Verified live | 0/8 Duskbat Wing Membrane, 0/6 Darkhound Hide, 0/3 Vile Fin Murloc Skin. These are visible labels and counts, not proven item-source links. |
| Reward choices | Verified live | Patchwing Pants 281283, Howlhide Vest 281284, Finscale Soles 281285; each quantity 1, choice type. |
| Fixed item rewards | Verified live as empty | Zero fixed items in the latest quest-log reward snapshot. |
| XP and money | Verified live | 650 XP and 300 copper, or 3 silver. |
| Reward section wording | Representable | The client stores choice versus fixed reward type. `You will be able to choose one of these rewards` is game UI wording and can be rendered in the site's chosen locale; it is not stored as quest narrative. |
| Reputation, spell, title or currency rewards | Partly checked, still unverified for this quest | The new detail and log snapshot show no currencies or spell rewards and zero honor. The selected-quest reputation probe produced no record. Nil must not be presented as absent. |
| Quest area on map | Pin, membership and edge samples verified; full shape missing | The 0.1.9 scan saved per-quest inside/outside values. It also recorded three edge events for quest 86784 on map 1420: inside, outside, inside. These points are player positions at membership changes, not the blue area's full boundary. `QuestPOIGetIconInfo` remains unavailable. |

## Collection contract and live status

`Verified` means present in the owner's saved observations. `Implemented` means local code exists but the specific Forever interaction has not yet proved it. `Missing` means the collector does not capture the field reliably. A nil field means unknown, never zero or absent.

| Interaction | Expected fields | Current status |
| --- | --- | --- |
| Shared context | Forever product, build, locale, realm, faction, class, race, level, time, map, zone, subzone and player position | Core context populated in live records; zone and subzone added in 0.1.7. Region and phase are missing. |
| NPC and gossip | Creature ID/name/level/type/classification, interaction point, subtitle, dialogue, options, visible quest offers and active quests | The 0.1.10 save contains Zapetta, Hin Denburg, Deathguard Burgess, Jamie Nore, Executor Zygand and Shari Stilwell with IDs and interaction points. Their visible dialogue or greeting was saved. Version 0.1.11 now persists plain-text roles and true selected option IDs. The latest run captured Deathguard Cyrus: `A class trainer` (ID 95464, raw index 4), then `Paladin` (ID 143045, raw index 1), followed by a new gossip page. The previous Burgess `The inn` choice was migrated to option ID 95466. Complete branch semantics and travel are still not proven. |
| Quest greeting | Offer and active quest title, ID, level, status, NPC and greeting text | Title/greeting implemented. Version 0.1.6 adds guarded ID, level and offer flags; live check pending. |
| Quest detail and existing quest log | Quest ID, title, level, giver, start item, prose, objectives, required items, reward choices, fixed rewards, XP, money, currencies, spell/title/honor rewards | Hides example verifies the principal fields, though reward items/XP/money came from a quest-log snapshot. Version 0.1.7 adds guarded probes for the remaining reward categories, the event's start-item ID, and description/objective prose from already accepted quests. Quest-specific reputation remains unverified. Long text can reach 8,192 characters and marks truncation. |
| Quest lifecycle | Accepted, objective progress, removed, turned in | Accepted and objective snapshots verified. Turn-in seen for a different quest. `removed` must not be labelled abandonment without a separate reason. |
| Vendor | NPC, items, prices, stock, stack size, purchase conditions, alternative costs, item variant | Ordinary item prices and stock fields verified. Alternative currency costs and numeric item variants added in 0.1.7; live coverage remains unverified. |
| Loot and sightings | Item ID and variant, quantity, source creature or object ID, direct loot-slot quest ID when exposed, interaction point, slot clear, targeted entity sightings | The latest run saw Duskbat Pelt 2876 and Duskbat Wing Membrane 278242 together in a Greater Duskbat 1553 loot window, each quantity 1 and `isQuestItem=true`; both slots cleared. One second later, matching quest objectives advanced by one. This is strong candidate evidence for source and quest links, although the loot slots supplied no quest IDs and the bag-count probe emitted no increase. `tools/audit_loot_quest_links.lua` conservatively reproduces these links from visible loot, slot clear, exact objective name and positive progress within ten seconds. It also finds two earlier candidate links from creature 1547. |
| Trainer and taxi | NPC and visible services/routes/costs | The latest run proves the trainer event path. Shari's window initially returned zero services, then 142; version 0.1.12 saved the empty diagnostic and the first 100 named services with prices. Faruza, Apprentice Herbalist, similarly returned zero then five, and all five were saved. Version 0.1.13 raises the trainer cap to 250 with an explicit truncation flag; the 142-service case needs a new window to prove full coverage. Zeppelin Masters were recorded with dialogue, but no route/action event. |
| World object and profession actions | Object identity, point, opened state, recipe/craft result | Game objects can be identified from target, gossip or loot GUIDs when exposed. The live build reports no selected-recipe API, and the 0.1.9 runtime check reports classic trade-skill APIs unavailable during the scan. No profession window was observed in this save. The guarded legacy reader remains locally tested only; exact craft completion and all object actions remain unverified. |
| Quest map | Pin or area with map ID and geometry type | The 0.1.8 scan returned pins for 13 quests on map 1420 and, in an earlier pass, pins on map 1415. The 0.1.9 scan verified per-quest inside-blob values, and the optional edge event fired three times. It cannot extract the game's shaded polygon; edge samples must not be rendered as an exact boundary. |

## Before claiming a field is collected

1. Identify the exact event and API return for the Forever build, including a missing or secret-value outcome.
2. Add a bounded record field with source and uncertainty. Do not infer a canonical quest, NPC, item or map relation from matching text alone.
3. Check one saved example for ID, value, count, type and persistence after `/reload` or logout. A unit test can catch code errors, but cannot establish that the beta returned the field.
4. Update this matrix and the record schema together. Ask the player for a new action only when a live interaction is genuinely needed.

The latest saved file has 266 records, 23 from 0.1.12, with zero dropped records and zero handler errors. Red Skeletal Horse, creature 267357, was saved as a sighting and gossip interaction with dialogue. Shari and Faruza produced trainer windows after an initial empty event. The loot/progress observations are detailed below. Opening the spellbook and the character, reputation, skills, PvP, currency and statistics tabs created no distinct collector records. The probe does not currently snapshot personal spell lists or character profile tabs; [PREWORK.md](PREWORK.md) explicitly keeps full builds, gear, inventory, gold and play history out of default community collection. Hash-checked private copies of older saves are in `%LOCALAPPDATA%\RestedRealmCollector\Backups`. Collection remains bounded at 1,500 records; the current save is about 18% of that cap. A future companion must copy completed saves into a durable queue before relying on a week of play.

## New quest-item link evidence

| Loot observation | Quest objective change | Assessment |
| --- | --- | --- |
| Greater Duskbat 1553 showed Duskbat Pelt 2876; slot cleared at sequence 262 | The Chill of Death 375, Duskbat Pelt 2/5 to 3/5 at sequence 265, one second later | Strong candidate source and quest link; no direct loot-slot quest ID or bag delta. |
| Greater Duskbat 1553 showed Duskbat Wing Membrane 278242; slot cleared at sequence 263 | Hides for the Forsaken 97558, Duskbat Wing Membrane 0/8 to 1/8 at sequence 266, one second later | Strong candidate source and quest link; no direct loot-slot quest ID or bag delta. |

The read-only audit tool also finds older exact-name, one-second progress matches for Darkhound Blood 2858 to A New Plague 367 and Darkhound Hide 278243 to Hides for the Forsaken 97558, both from creature 1547. These are observational candidates, not a complete drop table or an eligible-kill denominator.

## Completed 0.1.8 verification run

The owner completed this ordinary-play sample. The live results are recorded above. No quest needs to be reaccepted just to exercise the collector.

1. Check `/rrc status` shows `0.1.8-probe`, collection on and zero handler errors. Type `/rrc scan` once while the quest log is available.
2. Target a creature, speak to a gossip NPC and select one option, then visit a quest giver and vendor. If convenient, use an NPC with a subtitle such as Shelene Rhobart. Open a quest detail or completion panel if normal play leads to one.
3. During normal play, open a loot window and loot an item. The audit will check quest-item flags, any direct quest ID, all exposed source IDs and whether the specific item's bag count increased after the slot cleared. A slot-clear record alone is only a UI event.
4. If naturally nearby, open a profession window, trainer and flight master. Learned recipe events will record only if one happens normally; there is no need to learn a recipe solely for this probe.
5. At the end, `/reload` or log out once so SavedVariables is written. The local audit should check the complete fields in each resulting record, not just the record count.

## Next live check for 0.1.13

1. After `/reload`, opening Shari's trainer window again should save all 142 visible services, with `truncated=false`. No purchase is needed.
2. A profession window, if convenient during normal play, should produce a `profession_catalog` or a guarded modern profession record. The client reported neither profession API path at the last quest scan, so a real window is still needed to establish what it exposes.
3. If a guard direction conversation naturally occurs, check whether `gossip_poi` is returned for the current map. The functions exist in this build, but the previous guard conversations produced no marker.

The spellbook and personal character tabs remain outside default community collection. The owner-operated static export already supplies spell/talent definitions. A separate explicit opt-in would be appropriate for personal spell lists, reputation, skills, PvP, currencies or statistics if those are later needed for a character-profile feature.

Remaining limits before a claim of complete coverage: no verified blue quest-area polygon, complete gossip branch trace, all world-object actions, definitive item-source attribution when the loot slot omits a quest ID, exact craft completion, or eligible-kill denominator. These require further API discovery, live samples and data modeling.
