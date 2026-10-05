# Website request: check every spell description in the background, and the empty trainer list

From the RestedRealm website (restedrealm.com), 5 October 2026. Self-contained:
everything needed is in this file.

## What the first real records showed

The owner played a level 9 Night Elf priest with addon 0.1.19 and visited the
priest trainer in Dolanaar. Fifteen `spell_tooltip` records arrived and were
compared with our spell pages. Seven of the fifteen were wrong on the website:
damage and healing ranges ("47 to 58" shown as "51"), a talent condition
shown as "?", and language and armor lists shown as "????". All three causes are
fixed on the website, and that check now runs automatically for every record
(Admin, Data, Spell text).

Two things for the Companion follow from that run.

## 1. A background check of every spell the website lists (new kind `spell_scan`)

`C_Spell.GetSpellDescription` (or `GetSpellDescription`) answers for any spell
ID, not only spells the character knows. The addon already records a vendor's
whole item list when the window opens; this is the same idea for spells. It
turns a few trainer visits into a check of every spell page on the site.

**The list.** `GET https://restedrealm.com/api/collector/spell-scan` (public, no
token, cached for an hour) returns:

```json
{ "build": "1.60.1.70205", "listVersion": "1.60.1.70205-7", "packSize": 200, "spells": [17, 116, 120, "... about 13,000 IDs"] }
```

These are the spells whose descriptions contain numbers the website works out
from the client files. The app fetches it when `listVersion` changes (a new build
or a new published catalog) and hands it to the addon in whatever way suits the
app; a generated Lua data file in the addon folder, loaded at login, is one
option, since SavedVariables are overwritten at logout.

**The scan.** Once per `listVersion` and character level (levels change
numbers), while the player is idle and out of combat:

- Read descriptions in small steps (for example 20 per frame or a few hundred
  per second at most), so the game never stutters. Spells whose data is not
  loaded yet can be requested with `Spell:CreateFromSpellID(id):ContinueOnSpellLoad`
  or simply retried later in the same scan.
- Leave out empty descriptions.
- Sanitize text like quest text (`<name>`, `<race>`, `<class>`), `textSchema: 1`.
- Respect `/rrc text off`: nothing is scanned with text capture off.
- Pause on combat or when the player is busy, and continue where it stopped.

**The record.** One record per pack of up to 200 descriptions:

```json
{ "kind": "spell_scan", "textSchema": 1,
  "data": { "listVersion": "1.60.1.70205-7", "level": 60, "class": "PRIEST", "race": "Scourge",
            "talentPoints": [0, 5, 11],
            "entries": [ { "spellId": 2050, "text": "Heal your target for 47 to 58." } ] } }
```

- `level`, `class`, `race`, `talentPoints`: as `spell_tooltip` sends them.
- `entries`: at most 200 per record (the website accepts up to 300 per array and
  1 MB per batch). About 13,000 spells is about 66 records, well under the
  website's limit of 5,000 records per device per day.
- Already accepted by the website since 5 October 2026 (test:
  `tests/collector-intake.test.mjs`, "a spell scan pack of 200 descriptions").

**Privacy.** These are game texts, not about the player; the website's Companion
page already says a later version will check them in the background and that
they are kept privately like other spell descriptions. Please mention the scan
in the addon's own notes.

**Check after release:** play any character with the new version, let it idle for a
few minutes, and Admin, Data, Spell text on the website lists thousands of
checked spells for that level.

## 2. The trainer list arrived empty

At the priest trainer the addon recorded `trainer_window` with
`meaning: "trainer_event_without_visible_services"` and `shownCount: 0`, and no
`trainer` record followed. The addon already switches the trainer filter to show
every status (`showAllTrainerServices`), so the filter is not the cause.
`GetNumTrainerServices()` returned 0 at the moment it was read.

Likely causes to check in game: the services are not filled yet at
`TRAINER_SHOW` and arrive with a later `TRAINER_UPDATE` that is not read (or is
read before the filter change takes effect); or this client fills the list
through another API. Please read the list again on `TRAINER_UPDATE` and once
shortly after the window opens (for example 0.5 seconds later), record what the
API returned each time, and keep the existing empty record only when the list
stays empty.

**Check after release:** visit any class trainer; a `trainer` record arrives with
its services (names, status, levels, cost).
