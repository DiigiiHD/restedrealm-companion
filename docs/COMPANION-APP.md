# RestedRealm Companion: Windows app plan

Status: phase 1 (the Rust core) is built and tested; see [app/README.md](../app/README.md). Phases 2 to 6 are not built yet. Written 25 September 2026 after the owner's decisions below. The current Python companion keeps working until this app replaces it.

## Goal

A player installs one small program from restedrealm.com, connects it to their RestedRealm account once, and never has to think about it again. The program installs and updates the addon, notices each completed game save, uploads it and keeps itself current. Its screens feel like the website.

## Decisions made

| Question | Decision |
| --- | --- |
| Name | **RestedRealm Companion** for the whole product: the Windows app and the addon's display name. The in-game command stays `/rrc`. |
| Technology | [Tauri 2](https://v2.tauri.app/): a Rust core with screens written in HTML and CSS that reuse the website's colors and fonts. |
| Platform | Windows 10 and 11 only. |
| Install location | Per user, in `%LOCALAPPDATA%\Programs\RestedRealm Companion`. No administrator prompt. Listed in Settings > Apps and in Control Panel > Programs and Features with an Uninstall button. |
| Who can connect | Everyone who signs in to restedrealm.com with Discord. Sign-in leads to the `/account` section. |
| Automatic upload | On by default, shown clearly on the setup screen with the switch visible and easy to turn off. |
| Language | English. |
| Distribution | A download button on restedrealm.com, so players come through the website. |

## What the player experiences

1. **Download** from a download page on restedrealm.com.
2. **Install.** A short wizard: welcome, install location (per-user default already filled in), "Start RestedRealm Companion when Windows starts" (checked), install, and a last page with "Launch RestedRealm Companion" (checked).
3. **Setup** on first launch, one step per screen:
   - *Find World of Warcraft.* The app locates the game and the Forever folder by itself and shows what it found. A Browse button covers unusual installs.
   - *Install the addon.* One button copies the addon into the Forever `AddOns` folder. If WoW is running, it says to fully close it or type `/reload` afterwards.
   - *Connect your account.* A **Connect** button opens restedrealm.com in the browser. The player signs in with Discord if needed and clicks **Connect this PC**. The browser hands control back to the app, which shows "Connected as <Discord name>". A "Use a code instead" link keeps the current code method as a fallback.
   - *What gets sent.* Plain language about what is uploaded, what stays on the PC, and a visible **Upload automatically** switch that is on. A link to the privacy page.
   - *Done.* "Play as usual. Your data uploads after you log out, quit or type `/reload`."
4. **Daily use.** The app starts minimized to the icon next to the Windows clock. The icon's menu shows status, "Upload now", "Open RestedRealm", "Settings" and "Quit". The main window answers three questions in plain words: is it connected, when did it last upload, and is anything waiting. Below that, a friendly summary such as "This week: 14 quests and 32 characters recorded", with a link to the player's contributions on restedrealm.com. Raw record types, sequence numbers and digests do not appear on the main screen. Notifications are rare: only a problem the player can fix, such as a lost connection or a WoW folder that moved.
   - **"View my data"** in Settings keeps the current detailed list for anyone who wants it: each record's type in readable words, its date, whether it was uploaded, and the exact content sent. It exists for trust and transparency, so a player can always check what leaves their PC, but nobody needs it to use the app.
5. **In game.** At login the addon prints one quiet line, for example "RestedRealm Companion: connected, last upload 2 hours ago". If the app has not picked up data for a few days, the line becomes a gentle warning. `/rrc status` shows the same detail.
6. **Uninstall** through Settings > Apps or Programs and Features. The uninstaller asks two questions, both unchecked by default: "Also remove the addon from World of Warcraft" and "Also delete my local RestedRealm data". It removes the start-with-Windows entry and the saved device credential, and tells the player they can remove the device from their account page too.

## How it fits together

`addon writes save -> app sees the file change -> import into the local queue -> upload -> website evidence`

The data path, trust boundary and upload contract in [PROJECT-HANDOFF.md](PROJECT-HANDOFF.md) and [UPLOAD-CONTRACT.md](../companion/UPLOAD-CONTRACT.md) stay the same. What changes is the program around them.

- **Rust core.** Port `save_parser.py`, `restedrealm_companion.py` and `uploader.py`: the data-only save parser (never runs the save as Lua), the SQLite queue with transactional import and duplicate protection, guarded rollover, and the upload with its acknowledge-only-on-success rule. The 11 Python tests become parity tests, run against the same sample saves. The Python version stays in the repository as the reference until the port matches it.
- **Screens.** HTML and CSS using the website's tokens: background `#111512`, panels `#191e19`, lines `#30362b`, text `#e8e6dc`, muted `#9b9f91`, gold `#c6a563`, green `#9eaf84`, Lora for titles and Inter for body text. The fonts ship inside the app; nothing is loaded from the internet.
- **Tauri plugins** already cover the Windows parts: `single-instance` (one copy running), `autostart` (start with Windows), a tray icon, `deep-link` (the website handing control back), `updater` (signed updates) and the NSIS installer bundler (install wizard, Programs and Features entry, uninstaller).
- **Credential** stays in Windows Credential Manager, as now.
- **Watching** uses Windows file-change notifications instead of checking every 30 seconds, then waits for the file to settle as the current code does. Idle CPU should be close to zero.
- **Automatic rollover.** When WoW is fully closed and every record is safely in the queue, the app frees the addon's 1,500-record space by itself, keeping the private backup it keeps today. The player no longer presses "Free addon space".
- **Existing pilot data.** On first launch the app imports the current Python companion's queue from `%LOCALAPPDATA%\RestedRealmCollector\Companion\queue.sqlite3` so no record or sequence is lost.
- **Logs.** A small rotating local log file, with a "Copy diagnostics" button for support that leaves out the credential and any game text.

## Connecting to an account

The browser method needs care, because a link that pairs a PC could be abused: someone could send a player a crafted link so the player's PC uploads into the attacker's account. So the app always starts the flow:

1. The app creates a random one-time value and opens `https://restedrealm.com/companion/connect?state=<value>`.
2. The signed-in player sees which PC is asking and clicks **Connect this PC**.
3. The site creates a short-lived, single-use code and opens `restedrealm-companion://connect?code=...&state=...`.
4. The app accepts it only if `state` matches the value it created in step 1, redeems the code over HTTPS, stores the credential and shows the account name it is now connected to.

Any link the app did not start is ignored. The typed-code fallback keeps working as it does today.

## The addon

- The addon's displayed title becomes "RestedRealm Companion". The folder name `RestedRealmCollector` and the save variable `RestedRealmCollectorDB` stay the same for now, so existing saves and the pilot data need no migration. Renaming the folder later is a separate, tested step that the app would have to handle.
- **Status file for the in-game check.** An addon cannot see Windows programs; it only reads its own files when the game loads it. The app therefore writes a small generated file, `CompanionStatus.lua`, into the addon folder: connected or not, last upload time, items waiting, app version. The addon lists it in its `.toc`, reads it at login or `/reload`, and prints the status line. This is the same method Raider.IO's client uses. It cannot be live: it shows what was true when the game loaded.
- The same file can later carry information back from the website, such as "RestedRealm still needs data for these quests". That is a later phase.
- The addon keeps following Blizzard's addon policy: free, readable source, no ads and no donation requests in game.

## Finding WoW, now and after Forever leaves beta

Nothing about the game's location is hard-coded.

1. Find the WoW root folders from the Battle.net install records and the Windows uninstall entry for World of Warcraft, then fall back to common locations. The player can always pick a folder by hand.
2. Inside each root, list the game version folders (`_classic_beta_`, `_retail_` and so on) and read each one's version file to see which product it is.
3. Which folder names and product codes count as **Forever** comes from a small settings file on restedrealm.com, not from the app's code. Today it lists `_classic_beta_`. When Forever moves to its final folder, the website changes that setting and every installed app picks it up at its next check, without a new app release. The real product codes must be confirmed on actual installs before this list is written.
4. When a new Forever folder appears, the app installs the addon there, keeps watching the old folder until its queue is empty, and tells the player once that it moved.

## Updates

Two things need updating, the app and the addon, and both happen without the player doing anything.

- **App updates.** Tauri's updater checks a small signed file listing the latest version at startup and every few hours. It downloads the new installer, verifies its signature and installs it quietly, since it is a per-user install, then restarts the app. The signature uses a free key pair that Tauri generates. The private key lives only in the GitHub Actions secrets of this repository, never in the code. An update that fails its signature check is refused.
- **Addon updates.** Each app release carries the matching addon. After an update the app compares the installed addon's version with its own copy and replaces it while WoW is closed. If WoW is running, it waits.
- **Game patches.** When a Forever patch changes the interface number the addon declares, that ships as a normal addon update through the app.
- **Minimum version and pause switch.** The website's settings file also holds a minimum supported app version and an "uploads paused" switch with a message. A too-old app shows "Please update" and keeps its queue until it updates. The pause switch lets the owner stop every upload at once, for example after a bad release or a request from Blizzard. Queued data stays safe on each PC.
- **Releases.** Pushing a version tag to this repository starts a GitHub Actions job. It builds the app, signs it, creates a GitHub Release with the installer and the update file, and the website's download button points at the newest release. GitHub hosts the downloads for free; players still start from restedrealm.com.

## Code signing without a budget

Windows shows a blue "Windows protected your PC" warning for a new unsigned download. The player must click "More info" and then "Run anyway". Signing needs a certificate, and certificates normally cost money. The options, best first:

1. **SignPath Foundation (free).** It signs open-source projects at no cost. This repository qualifies on paper: it is public, MIT licensed and can be built by GitHub Actions. The installer's publisher would read "SignPath Foundation" instead of RestedRealm. Acceptance is decided by them and is not guaranteed, so apply early.
2. **Unsigned with honest instructions (free), for the test group and until signing is in place.** The download page shows the warning, explains the two clicks and lists the file's SHA-256 checksum. The warning is about the first download; updates installed afterwards by the app itself normally do not show it.
3. **Microsoft's signing service (about $10 a month)** later, if the website earns support. Eligibility for individuals depends on the country and must be checked first.

Tauri's own update signature (above) is free and separate. It protects updates whether or not the installer is Windows-signed.

## Privacy and rules

- Automatic upload on by default is acceptable only because connecting is a deliberate act and the setup screen says plainly what is sent before it happens. The switch must stay one click away in Settings. This is a product decision, not legal advice.
- The website needs a privacy page covering the companion: what is collected, what is uploaded, retention (90 days for raw observations), account export and deletion, and device removal. It belongs in the website's `docs/BEFORE-LAUNCH.md`.
- The app reads only the addon's own save file and the game's version files. No game memory, network traffic, other addons, chat or credentials, as today.
- Full NPC and quest text stays local until the content-rights review decides otherwise, as today.
- Supporting the website happens on the website. The app and addon ask for nothing.

## Website work (in the `restedrealm` repository)

- Open pairing from owner-only to every signed-in Discord account; the check is in `src/app/api/collector/pair-code/route.ts`.
- Rename `/account/collector` to `/account/companion`, keeping a redirect from the old address.
- Add the `/companion/connect` approval page and the endpoint that issues the one-time code for the app's return link.
- A download page with the installer, the checksum, the Windows warning explained and what the companion does.
- The settings file for the app: Forever folder names and product codes, minimum version, upload pause and message.
- Upload limits per account and device, and abuse controls, now that anyone can connect. Follow `docs/EXTERNAL-APIS.md` where it applies.
- Privacy page and the matching `docs/BEFORE-LAUNCH.md`, `docs/CHANGELOG.md` and `docs/ROADMAP.md` entries.
- Later: a "your contributions" section in the account.

## Phases

1. **Core port. Done 25 September 2026.** Rust core in `app/core` with 20 tests of its own and a parity check that matches the Python version byte for byte on generated saves, ready to run on Windows and Linux by GitHub Actions once `docs/ci/core.yml` is moved to `.github/workflows/`. No window yet. Applying to SignPath Foundation is still to do.
2. **App shell.** Tauri window in the website look, setup screens, tray icon, start with Windows, settings, import of the existing pilot queue.
3. **Account connection.** Website changes for opening pairing, the approval page and the return link. The app side of the same flow.
4. **Addon handling.** Finding WoW, the website settings file, installing and updating the addon, `CompanionStatus.lua` and the in-game status line.
5. **Installer and updates.** NSIS installer with the launch checkbox and uninstall choices, the signed updater, the GitHub Actions release job and the download page.
6. **Test group, then everyone.** A handful of players first, unsigned if signing is not ready, with the owner's PC as the first install. Wider release when updates, uninstall and the Forever-folder switch have been proven on real PCs.

## Still open

- Final Forever folder name and product code: unknown until Blizzard ships it.
- SignPath Foundation's answer.
- Whether the addon folder is ever renamed to match the product name.
