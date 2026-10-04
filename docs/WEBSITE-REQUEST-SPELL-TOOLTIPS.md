# Website request: record spell tooltips as the game shows them

From the RestedRealm website side, 4 October 2026, at the owner's request. This file is self-contained: the website repository is not needed to do the work. Read `AGENTS.md` and `docs/PROJECT-HANDOFF.md` first; their rules on privacy, game text and addon performance apply.

## Why

restedrealm.com builds its spell pages from the Forever game files. It works out the numbers itself, for example "Cannibalize 680 of your own Health over 15 sec to gain (680 + Spirit) Mana" for Dark Sacrifice rank 2 (spell 1277325). The game shows the real numbers for the player's level and stats, and it includes Blizzard's hotfixes, which the website's data does not have yet.

If the addon records the description the game itself shows for each spell the character knows, the website can compare it with its own and list every spell where they differ. Mistakes and hotfixes then surface automatically instead of by a person checking pages by hand. The owner asked for exactly that: "I can't have it that things are incorrect, missing, not updated manually."

## What to build in the addon

A new record kind, `spell_tooltip`, in `addon/RestedRealmCollector/Collector.lua`:

1. **When:** on the spellbook events (`SPELLS_CHANGED` and `LEARNED_SPELL_IN_TAB`; register them in the `handlers` table the same way the other events are registered, with `pcall`, so a missing event on this client does nothing), and once shortly after login. Throttle it: one scan per event burst, never on every frame.
2. **Which spells:** the spells in the player's spellbook (all tabs, including passive ones). Leave out flyouts and spells the character does not know.
3. **What to record**, through the existing `record(kind, data)`:
   ```lua
   record("spell_tooltip", {
     spellId = 1277325,
     rank = "Rank 2",          -- GetSpellSubtext(spellId), when it returns one
     text = "<the description exactly as the game shows it>",
     level = UnitLevel("player"),
     class = select(2, UnitClass("player")),  -- "PRIEST"
     race = select(2, UnitRace("player")),    -- "Scourge"
     source = "GetSpellDescription",
   })
   ```
   - `text` comes from `C_Spell.GetSpellDescription(spellId)` where that exists, otherwise `GetSpellDescription(spellId)`. Store which one in `source`. Some descriptions load lazily: if the result is empty, try again on the next scan rather than recording an empty string.
   - `text` is game text, like quest text. Put it through the same path the addon already uses for quest text (`longText`, the `captureText` setting and the `<name>` / `<race>` / `<class>` replacement), so the record carries `textSchema` exactly like `quest_log_text` does. The website only stores text from records marked that way.
4. **Do not repeat:** keep a fingerprint per spell, as `db.questTextFingerprints` does for quest text, keyed by spell ID with the text, level and build. Record again only when that changes, for example after a level up, a talent or a new build.
5. **Size:** a spellbook holds a few hundred spells, so the first scan writes a few hundred records. After that, only changes. The website accepts text up to 8,192 characters for sanitized records.

The Windows companion needs no change: it uploads every kind the addon writes.

## Website side

- **Already live:** the website accepts and stores `spell_tooltip` records. They are listed in Admin as "kept, not shown yet". So nothing is refused if the addon release goes out first.
- **Built when the first records arrive:** a comparison that works out the website's own description for the same spell at the reported level and lists every spell where the numbers differ, in Admin and in the website's data watch. Public pages will later be able to say "matches the game at level 30". The text itself is not shown publicly until that wording is decided.

## Tests

- Extend `tests/collector_smoke.lua`: with a stubbed `GetSpellDescription`, a spellbook of two spells produces two `spell_tooltip` records with `textSchema`. A second scan with the same text produces none. A level change produces new ones. An empty description produces none.
- Check that a client without `C_Spell` falls back to `GetSpellDescription` without errors.

## Check after release

Play a level 30 Undead priest with the Companion connected and upload. The website's Admin evidence view lists `spell_tooltip` records for that account, including Dark Sacrifice (spell 1277325) with the game's own mana number.
