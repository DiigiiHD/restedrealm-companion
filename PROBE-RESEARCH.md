# Forever probe research and implementation boundary

25 September 2026. This document records public product/API research for the local addon probe. It does not authorize copying Wowhead data or publishing game prose.

## Wowhead comparison

[Wowhead's Client page](https://www.wowhead.com/classic/client) describes a two-part workflow: its in-game Looter addon collects observations during play, and a separate desktop application uploads them. The desktop program can be closed during gameplay and can upload after WoW closes. [Wowhead's FAQ](https://www.wowhead.com/faq) says its client collects NPC and item encounter data and that submitted data goes through an internal process before appearing on the site. These public pages establish the broad architecture; they do not disclose a complete field schema or prove what Wowhead Looter currently captures on Forever.

RestedRealm already has a static Forever client extraction. The addon therefore concentrates on observed, build-specific facts: which NPC or object was seen, where the player was, what was offered, how a quest progressed, what appeared in loot, and what the UI actually showed. The future companion remains necessary because the addon cannot send HTTPS requests directly through ordinary WoW addon APIs.

## API paths reviewed

- Blizzard-generated [Forever gossip documentation](https://raw.githubusercontent.com/Gethe/wow-ui-source/classic_beta/Interface/AddOns/Blizzard_APIDocumentationGenerated/GossipInfoDocumentation.lua) lists option and quest fields, plus selection methods and confirmation events. The probe records the fields returned and hooks selection calls without changing gameplay. A selection call can be automatic, so it is not labelled as a guaranteed human click.
- Blizzard-generated [Forever quest-log documentation](https://raw.githubusercontent.com/Gethe/wow-ui-source/classic_beta/Interface/AddOns/Blizzard_APIDocumentationGenerated/QuestLogDocumentation.lua) exposes objective snapshots and quests on a map. The probe also checks legacy quest reward and text APIs because the live build returned them. A map pin or blob count is not the shaded area's polygon.
- Blizzard-generated [Forever trade-skill documentation](https://raw.githubusercontent.com/Gethe/wow-ui-source/classic_beta/Interface/AddOns/Blizzard_APIDocumentationGenerated/TradeSkillUIDocumentation.lua) includes `NEW_RECIPE_LEARNED` and trade-skill window events. The probe uses those events and guarded recipe-information calls. It does not poll every known recipe or inventory slot.
- Blizzard-generated [Forever map documentation](https://raw.githubusercontent.com/Gethe/wow-ui-source/classic_beta/Interface/AddOns/Blizzard_APIDocumentationGenerated/MapDocumentation.lua) supports normalized player map positions. Every sighting retains the map ID and explicitly labels the coordinate as the player's position.
- Blizzard's [loot-frame source](https://github.com/Gethe/wow-ui-source/blob/live/Interface/AddOns/Blizzard_UIPanels_Game/Mainline/LootFrame.lua) reads quest-item flags and a quest ID from `GetLootSlotInfo`, when supplied by the game. The probe now preserves these fields and every exposed loot-source pair. A quest ID in the slot would be stronger evidence than matching the item name to objective text, but it still needs a live Forever sample.

The installed beta is the final check for every API. A function existing in public documentation does not mean the active Forever build returns useful values at every event. `apiStatus` and the sanitized saved-file audit report availability and actual field presence separately.

## Implemented observation families in 0.1.8

| Family | Record kinds | Evidence limit |
| --- | --- | --- |
| NPC and object encounters | `entity_sighting`, `gossip`, `gossip_selection`, `gossip_confirmation`, `quest_greeting` | Target/interact position is a player point, not exact spawn. Gossip selection proves a call, not a click. |
| Vendors and services | `merchant`, `trainer`, `taxi_map` | Offers, prices and routes may depend on faction, reputation, level, stock and build. A missing offer is not negative proof. |
| Quests | `quest`, `quest_state`, `quest_objectives`, `quest_log_text`, `quest_reputation` | Detail, active state, completion and turn-in are separate observations. Nil rewards mean unknown. Reputation scale is unverified. |
| Items and loot | `loot_window`, `loot_slot_cleared`, `item_count_increase` | A short delayed count of that item in the player's bags can show an increase. It does not prove which source caused it or establish a drop rate. Full inventory contents are not stored. |
| Professions | `profession_opened`, `recipe` | Learned/viewed recipes and available schematic fields are captured when the API returns them; complete craft outcome is not proven. |

All records retain build, locale, time and player location when available. The addon has no network access, collects no chat or player GUIDs, does not automate actions, and caps observations. The local helper audits a trusted SavedVariables file without printing names or prose.

## Remaining boundaries

- The reviewed API paths have not yielded polygon points for the blue quest area. The probe records a pin, map ID, on-map flags and blob count when available, with no inferred polygon.
- A direct quest ID on a loot slot can identify a quest-item relation when the beta supplies it. Otherwise, objective text, loot sources and changes in counts can support only a candidate relation. A content pipeline must reconcile IDs, locale, timing and multiple observations.
- Definitive item-source attribution, kill denominators, full object interaction outcomes and craft completions need further verified event paths. This probe does not manufacture those claims from a loot window or recipe display.
- The probe is local. A durable desktop queue, RestedRealm account pairing, authenticated upload endpoint, validation, review and publication controls remain separate product work described in [PREWORK.md](PREWORK.md).

The next game run is a field verification of version 0.1.8, not a claim that the collector is finished. See [COVERAGE-AUDIT.md](COVERAGE-AUDIT.md) for the one-run checklist.
