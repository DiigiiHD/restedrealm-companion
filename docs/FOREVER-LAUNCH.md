# Forever launch: recognising the right game and refusing the wrong data

Written 29 September 2026. Status: plan, not built. Owner decisions are recorded at the end.

## 1. What this solves

The beta is as far as RestedRealm can test today. When World of Warcraft: Forever is released, Battle.net will install it in its own folder with its own product code, and neither is known yet. Today four places assume the beta:

| Where | Assumption |
| --- | --- |
| App, game folder | `_classic_beta_` (`app/src-tauri/src/addon.rs`, `app/core/src/bin/rrc-core.rs`) |
| App, accepted saves | product `wow_classic_beta` (`app/core/src/save.rs`) |
| Addon, record label | `product = "wow_classic_beta"` (`Collector.lua`, `context()`) |
| Website, accepted uploads | `record.context.product !== "wow_classic_beta"` (`src/server/collector.ts` in the website repository) |

The goals, in order:

1. Never read, install into or upload from any World of Warcraft version other than Forever. Retail, Classic Era, Anniversary, the Progression realms and the PTRs are left alone, even when they sit next to Forever in the same folder.
2. On launch day, switch every player to the real Forever folder without a new app install and without a scramble.
3. Treat every upload as a report from a player's PC, which can be edited, and keep fake or hostile reports off the public site.

## 2. One list of Forever versions

A single list names every accepted Forever version. Each entry:

| Field | Example (beta, today) | Meaning |
| --- | --- | --- |
| `product` | `wow_classic_beta` | Battle.net product code |
| `folder` | `_classic_beta_` | Folder inside `World of Warcraft` |
| `executables` | `["WowClassicB.exe", "WowB.exe"]` | Game program names in that folder |
| `version` | `^1\.60\.` | Pattern the client's `GetBuildInfo()` version must match |
| `minBuild` | `69913` | Oldest build accepted |
| `stage` | `beta` | `beta` or `live`; live is preferred when both are present |
| `accepting` | `true` | Whether new uploads for it are still taken |

The exact executable names and version pattern are confirmed on the owner's PC before step 1 ships; the table shows the shape, not verified values.

**Where it lives.** The website serves the list at `https://restedrealm.com/companion/forever.json`, generated from one file in the website repository, and the website's intake reads the same file. The app carries a built-in copy (the beta only) and uses it whenever the site cannot be reached. The app fetches the list at start and then every six hours, keeps it with a `fetched_at`, and only ever replaces its copy with a newer, well-formed one. A malformed or unreachable list keeps the last good copy. It is our own service, so the Blizzard and Discord rules in the website's `docs/EXTERNAL-APIS.md` do not apply, but its spirit does: no fetch blocks the app, and a failure is quiet.

**Launch day.** The owner adds one entry for the live product and deploys the website. Every running app picks it up within six hours, or at once after a restart. No app release is needed. If the live version turns out to need an addon change, that is an ordinary app release, which reaches players within an hour.

## 3. Recognising Forever: every check must pass

The app treats a folder as Forever only when all of these agree. A folder that fails any check is ignored, and the app screen says which check failed.

1. **Name on the list.** Only folder names on the list are considered. The app never walks `World of Warcraft` looking for any version folder it can find.
2. **Battle.net agrees.** The `.build.info` file that Battle.net keeps in the `World of Warcraft` folder lists the entry's `product` as installed. The column layout is read from the file's own header line, not assumed. If the file is missing or unreadable, the folder is not accepted automatically; the player can still choose it by hand, and check 4 still applies to every save.
3. **The game program is there.** One of the entry's `executables` exists in the folder.
4. **The game itself confirms it, per record.** The addon already writes the client's `version` and `build` into every record. The app accepts a record only if its version matches the entry's pattern and its build is at least `minBuild`. A save with even one mismatching record is not imported, and nothing from it is queued. The app says so on screen.
5. **The website checks again.** Intake accepts only products on the list with `accepting: true`, and only builds it knows as Forever builds from its own client data sync (`resolveProduct` in the website). An upload for an unknown build is stored as held and never feeds a public page until the build is known.

The addon is installed only into a folder that passed checks 1 to 3, so it never lands in another version. The "game running" check uses the executables from the list instead of today's fixed set.

## 4. The label on each record

The addon runs inside the client and cannot know which Battle.net product it belongs to, so it stops naming one. From the addon release in step 3:

- The addon writes `product` as it does today during a transition release, plus `projectID` (the client's `WOW_PROJECT_ID`) as an extra client signal.
- The app sets `context.product` from the list entry of the folder the save came from, before the record's digest is computed. A record's label therefore always matches a folder that passed section 3.
- The digest stays deterministic, so duplicate detection and retries behave as today.

Beta records keep the label `wow_classic_beta` forever. Live records get the live code. Agreement rules on the website already compare the same build only, so beta and live evidence never count towards each other.

## 5. Switching automatically

- The app looks for every list entry under the chosen `World of Warcraft` folder. A player who picked the beta folder by hand is looked after too: the app looks one level up.
- When a `live` entry passes section 3 and its `WTF` folder exists (the game has been started at least once), the app installs the addon there and makes it the watched folder. The app screen shows "Watching: World of Warcraft: Forever" or "Watching: Forever beta".
- The beta folder is still read until every beta record is imported and the beta save is emptied as usual. After that it is left alone.
- If both exist and the player wants the beta back, a setting chooses; the default is live.
- A player with only the beta installed notices no change.

## 6. Hostile or fake reports

Nothing on a player's PC proves a report is true. Saves can be edited and the upload API can be called by hand with a real device credential. The website is where trust is decided.

**Already in place.** Device credentials tied to a Discord-signed-in account and revocable; size, structure, digest, product, build and locale checks; per-record refusal; per-device daily limits; sanitized text only; tooltip probes refused; control characters rejected; text rendered as plain text, never HTML; public data only when two different accounts agree on the same build with no conflicting report.

**To add, in the website repository:**

1. **Catalog checks.** Quest, NPC, item and spell IDs must exist in the Forever client data for the record's build. Unknown IDs are held, not published.
2. **A review list.** Conflicting reports, accounts reporting far above normal volume, and values outside what the client data allows are listed for the owner. Nothing listed is published until reviewed.
3. **Account weight.** An account's reports count towards agreement only after it has a history, for example a minimum age and a minimum number of accepted records across several days, so a handful of new Discord accounts cannot outvote real players. The exact thresholds are an owner decision.
4. **Per-account limits** next to the per-device ones.
5. **One-step block.** Blocking an account revokes its devices and removes its evidence from every agreement it took part in; affected public values are recomputed.

These do not depend on launch and can be built before it.

## 7. Order of work

Each step is its own pull request with its own tests, and each ships before the next starts.

1. **Website: the list.** Add the list file with the beta entry, serve `/companion/forever.json`, and make intake read it instead of the fixed string. Tests: unknown product refused, `accepting: false` refused, known product accepted.
2. **App: recognition.** Built-in list, fetching and caching the list, the five checks in section 3, per-record version and build checks, game-running check from the list, and the app screen line. Tests: a fake `World of Warcraft` tree with retail, Classic Era and beta folders, where only the beta is read; a beta folder holding a save with a retail version, which is refused; a missing `.build.info`; a malformed list, which keeps the last good copy.
3. **Addon and app: the label.** `projectID` in the addon; the app sets `product` from the folder. Tests: parity of digests for existing beta saves; a record claiming another product is relabelled by folder, not trusted.
4. **App: the switch.** Live preferred over beta, beta drained first, the setting. Tests: beta only; beta and live; live only; live present but never started.
5. **Website: the protections** in section 6, in the order listed.

Steps 1 to 4 are needed before launch. Step 5 can start any time.

## 8. Checked on the owner's PC before step 2

- The exact executable name(s) in `_classic_beta_`.
- The beta's `GetBuildInfo()` version string (for example `1.60.1`) and build.
- The `.build.info` header line and the beta's row, with any keys or tokens removed.

## 9. Decisions

- 29 September 2026, owner: recognise the Forever folder with certainty, never read another World of Warcraft version, and guard against malicious uploads. This plan follows from that.
- Open: the account-weight thresholds in section 6.3.
- Open: whether the app should ever keep reading the beta after live launches, beyond draining what is left.
