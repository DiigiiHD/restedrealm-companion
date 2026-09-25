# RestedRealm Collector companion: local queue

Status: private owner pilot, 25 September 2026. The private website intake is live. Owner pairing and a synthetic upload with duplicate retry passed; that test record was removed. The owner uploaded 283 saved observations, then a fresh post-rollover session added 48 more. The server has 331 observations and the local queue has zero pending. The companion offers manual upload and automatic upload after explicit opt-in.

## Use on this PC

1. Double-click `Start RestedRealm Collector.cmd` in this folder. It imports the last completed Forever save and checks for new completed saves every 30 seconds while open.
2. **Scan saved data** checks immediately. **Pause watching** stops periodic checks; **Watch saves** resumes them.
3. Double-click a row to inspect the full data held locally. The list shows only observation types, sequence numbers and dates.
4. Pair this PC from your signed-in RestedRealm Collector account page. The window then shows **Connected to RestedRealm**, including after a restart. Choose **Upload now** to send the queued observations, or enable **Upload automatically after a completed save**. The checkbox is off by default. No account credential or full NPC and quest prose is included in the upload payload.

The private queue is `%LOCALAPPDATA%\RestedRealmCollector\Companion\queue.sqlite3`. It survives app restarts and offline time. A second scan of the same game save does not duplicate records. A corrected record from an addon migration replaces its queued version. **Forget local queue** deletes only the companion copy; it does not change the WoW save. The addon still has a 1,500-record cap. Once WoW is fully closed, **Free addon space** (or `rollover --yes`) can remove records already copied into the queue from the game save. It first verifies every record's digest and keeps a private byte-for-byte backup under the companion state directory. Rollover is manual in this prototype.

The companion reads only `_classic_beta_\WTF\Account\*\SavedVariables\RestedRealmCollector.lua`. It waits for the file size and timestamp to settle, reads it once more with a consistency check, parses a limited data-only subset of Lua, validates Forever records, and commits a whole import in one SQLite transaction. An incomplete save leaves the existing queue untouched. It does not read other addons, game memory, traffic, chat or credentials.

For command-line use from this folder:

```powershell
python .\restedrealm_companion.py scan
python .\restedrealm_companion.py status
python .\restedrealm_companion.py preview --limit 20
python .\restedrealm_companion.py watch
python .\restedrealm_companion.py rollover --yes  # only after WoW is closed
```

The `watch` command runs until Ctrl+C. The desktop window watches only while open. Automatic website upload requires the owner's explicit checkbox opt-in; manual upload uses the button. A failed upload leaves the unacknowledged records queued for retry. Uploaded records remain in the local queue until you clear them.

## Verification

```powershell
python -m unittest discover -s ..\tests -p test_companion.py -v
```

The tests cover rejecting executable Lua, importing a save, repeat scans after restart, record correction, append across sessions, an incomplete save, guarded rollover, forgetting the queue, prose redaction, and leaving uploads pending after a timeout. On 25 September, the closed game's real save was imported with 283 observations and zero drops, then rolled over. A private backup was kept. The next in-game session restored collection and saved 48 new records, with `nextSeq = 331`, collection enabled and full text on. All 48 reached the server as sequences 284 through 331.
