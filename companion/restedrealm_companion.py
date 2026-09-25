"""Local, offline-first collector companion. Python 3.10+, standard library only."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import time

from save_parser import SaveFormatError, parse_save, parse_save_with_span


VERSION = "0.1.0-local"
MAX_SAVE_BYTES = 32 * 1024 * 1024
DEFAULT_GAME = Path(os.environ.get("PROGRAMFILES(X86)", r"C:\Program Files (x86)")) / "World of Warcraft" / "_classic_beta_"
DEFAULT_STATE = Path(os.environ.get("LOCALAPPDATA", str(Path.home()))) / "RestedRealmCollector" / "Companion"


def saved_files(game: Path):
    base = game / "WTF" / "Account"
    return sorted(base.glob("*/SavedVariables/RestedRealmCollector.lua"))


def stable_bytes(path: Path, delay=1.0):
    first = path.stat()
    if first.st_size > MAX_SAVE_BYTES:
        raise ValueError("save exceeds 32 MiB limit")
    time.sleep(delay)
    second = path.stat()
    if (first.st_size, first.st_mtime_ns) != (second.st_size, second.st_mtime_ns):
        raise ValueError("save is still being written")
    payload = path.read_bytes()
    third = path.stat()
    if (second.st_size, second.st_mtime_ns) != (third.st_size, third.st_mtime_ns) or len(payload) != second.st_size:
        raise ValueError("save changed while reading")
    return payload


def connect(state: Path):
    state.mkdir(parents=True, exist_ok=True)
    database = sqlite3.connect(state / "queue.sqlite3")
    database.execute("PRAGMA journal_mode=WAL")
    database.execute("PRAGMA synchronous=FULL")
    database.execute("PRAGMA secure_delete=ON")
    database.execute("""CREATE TABLE IF NOT EXISTS observations (
        source_id TEXT NOT NULL,
        seq INTEGER NOT NULL,
        digest TEXT NOT NULL,
        payload TEXT NOT NULL,
        queued_at INTEGER NOT NULL,
        uploaded_digest TEXT,
        PRIMARY KEY (source_id, seq)
    )""")
    columns = {row[1] for row in database.execute("PRAGMA table_info(observations)")}
    if "uploaded_digest" not in columns:
        database.execute("ALTER TABLE observations ADD COLUMN uploaded_digest TEXT")
    database.execute("""CREATE TABLE IF NOT EXISTS imports (
        source_id TEXT PRIMARY KEY,
        save_digest TEXT NOT NULL,
        record_count INTEGER NOT NULL,
        dropped INTEGER NOT NULL,
        imported_at INTEGER NOT NULL
    )""")
    database.execute("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
    return database


def validate_record(record):
    if not isinstance(record, dict):
        raise SaveFormatError("observation must be a table")
    seq = record.get("seq")
    if type(seq) is not int or seq < 1:
        raise SaveFormatError("observation has no valid sequence")
    if not isinstance(record.get("kind"), str) or len(record["kind"]) > 80:
        raise SaveFormatError("observation has no valid kind")
    context = record.get("context")
    if not isinstance(context, dict) or context.get("product") != "wow_classic_beta":
        raise SaveFormatError("observation product is not Forever")
    if not isinstance(record.get("data"), dict):
        raise SaveFormatError("observation data must be a table")
    return seq


def scan_one(database, path: Path, delay=1.0):
    payload = stable_bytes(path, delay)
    save_digest = hashlib.sha256(payload).hexdigest()
    source_id = hashlib.sha256(str(path.resolve()).casefold().encode("utf-8")).hexdigest()
    prior = database.execute("SELECT save_digest FROM imports WHERE source_id=?", (source_id,)).fetchone()
    if prior and prior[0] == save_digest:
        return {"new": 0, "updated": 0, "records": None, "dropped": None, "unchanged": True}
    db = parse_save(payload.decode("utf-8-sig"))
    records = db.get("records", [])
    if records == {}:
        records = []
    if not isinstance(records, list) or len(records) > 50_000:
        raise SaveFormatError("invalid observations list")
    prepared = []
    seen = set()
    for record in records:
        seq = validate_record(record)
        if seq in seen:
            raise SaveFormatError("duplicate sequence in save")
        seen.add(seq)
        canonical = json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        prepared.append((seq, hashlib.sha256(canonical.encode("utf-8")).hexdigest(), canonical))
    now = int(time.time())
    new = updated = 0
    # One transaction: a partial save cannot leave a partially imported queue.
    with database:
        for seq, digest, canonical in prepared:
            old = database.execute("SELECT digest FROM observations WHERE source_id=? AND seq=?", (source_id, seq)).fetchone()
            if old is None:
                database.execute("INSERT INTO observations(source_id,seq,digest,payload,queued_at) VALUES (?, ?, ?, ?, ?)",
                                 (source_id, seq, digest, canonical, now))
                new += 1
            elif old[0] != digest:
                # The addon can correct old observations during a schema migration.
                database.execute("UPDATE observations SET digest=?, payload=?, uploaded_digest=NULL WHERE source_id=? AND seq=?",
                                 (digest, canonical, source_id, seq))
                updated += 1
        dropped = db.get("dropped", 0)
        if type(dropped) is not int or dropped < 0:
            dropped = 0
        database.execute("""INSERT INTO imports VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(source_id) DO UPDATE SET save_digest=excluded.save_digest,
            record_count=excluded.record_count, dropped=excluded.dropped,
            imported_at=excluded.imported_at""",
            (source_id, save_digest, len(records), dropped, now))
    return {"new": new, "updated": updated, "records": len(records), "dropped": dropped, "unchanged": False}


def wow_running():
    if os.name != "nt":
        return False
    for name in ("WowB.exe", "Wow.exe", "WowClassic.exe"):
        result = subprocess.run(["tasklist", "/FI", f"IMAGENAME eq {name}", "/FO", "CSV", "/NH"],
                                capture_output=True, text=True, check=True)
        if any(line.startswith('"' + name + '"') for line in result.stdout.splitlines()):
            return True
    return False


def compact_one(database, path: Path, state: Path, delay=1.0, game_running=wow_running):
    """Remove archived records from a closed game's save, preserving other fields verbatim."""
    if game_running():
        raise RuntimeError("Close WoW completely before rolling over its save")
    original = stable_bytes(path, delay)
    text = original.decode("utf-8-sig")
    db, span = parse_save_with_span(text)
    records = db.get("records", [])
    if records == {} or records == []:
        return 0
    if not isinstance(records, list) or not span:
        raise SaveFormatError("no compactable observation list")
    source_id = hashlib.sha256(str(path.resolve()).casefold().encode("utf-8")).hexdigest()
    for record in records:
        seq = validate_record(record)
        canonical = json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
        row = database.execute("SELECT digest FROM observations WHERE source_id=? AND seq=?", (source_id, seq)).fetchone()
        if not row or row[0] != digest:
            raise RuntimeError("Some game observations are not yet safely queued; scan again first")
    replacement = text[:span[0]] + "{}" + text[span[1]:]
    check, _ = parse_save_with_span(replacement)
    if check.get("records") != {}:
        raise RuntimeError("rollover verification failed")
    if game_running() or stable_bytes(path, 0) != original:
        raise RuntimeError("WoW started or the save changed; rollover cancelled")
    backup_dir = state / "Backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    backup = backup_dir / (hashlib.sha256(original).hexdigest() + ".lua")
    if not backup.exists():
        with open(backup, "xb") as stream:
            stream.write(original)
            stream.flush()
            os.fsync(stream.fileno())
    elif hashlib.sha256(backup.read_bytes()).digest() != hashlib.sha256(original).digest():
        raise RuntimeError("existing backup does not match the game save")
    temp_name = None
    try:
        with tempfile.NamedTemporaryFile(mode="wb", prefix=".rrc-", suffix=".tmp", dir=path.parent,
                                         delete=False) as stream:
            temp_name = stream.name
            stream.write((b"\xef\xbb\xbf" if original.startswith(b"\xef\xbb\xbf") else b"") + replacement.encode("utf-8"))
            stream.flush()
            os.fsync(stream.fileno())
        if game_running() or stable_bytes(path, 0) != original:
            raise RuntimeError("WoW started or the save changed; rollover cancelled")
        os.replace(temp_name, path)
    finally:
        if temp_name and os.path.exists(temp_name):
            os.unlink(temp_name)
    return len(records)


def scan(database, game: Path, delay=1.0):
    paths = saved_files(game)
    if not paths:
        print("No RestedRealmCollector SavedVariables file found in the Forever installation.")
        return False
    good = False
    for path in paths:
        try:
            result = scan_one(database, path, delay)
        except (OSError, UnicodeError, ValueError, sqlite3.Error) as exc:
            # Do not expose an account folder name or raw SavedVariables text.
            print(f"Save skipped: {type(exc).__name__}: {exc}", file=sys.stderr)
            continue
        good = True
        if result["unchanged"]:
            print("Save unchanged.")
        else:
            print(f"Queued {result['new']} new, updated {result['updated']} corrected; "
                  f"{result['records']} in save, {result['dropped']} addon drops.")
    return good


def status(database):
    count = database.execute("SELECT COUNT(*) FROM observations").fetchone()[0]
    pending = database.execute("SELECT COUNT(*) FROM observations WHERE uploaded_digest IS NULL OR uploaded_digest<>digest").fetchone()[0]
    imports = database.execute("SELECT COUNT(*), COALESCE(MAX(imported_at), 0), COALESCE(SUM(dropped), 0) FROM imports").fetchone()
    print(f"Companion {VERSION}: {count} observations safely queued on this PC; {pending} pending upload; "
          f"{imports[0]} save source(s); {imports[2]} addon drops reported.")
    if imports[1]:
        print("Last local import:", time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(imports[1])))
    print("Website upload: disabled in this companion until owner pilot pairing is tested.")


def preview(database, limit: int):
    rows = database.execute("SELECT seq, digest, payload FROM observations ORDER BY queued_at DESC, seq DESC LIMIT ?",
                            (limit,)).fetchall()
    for seq, digest, payload in rows:
        record = json.loads(payload)
        observed = record.get("observedAt")
        when = time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime(observed)) if type(observed) is int else "unknown time"
        print(f"{digest[:12]}  seq {seq}  {record['kind']}  {when}")
    if not rows:
        print("Queue is empty.")


def forget_queue(database):
    with database:
        database.execute("DELETE FROM observations")
        database.execute("DELETE FROM imports")
        database.execute("DELETE FROM settings")
    database.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    database.execute("VACUUM")


def main():
    parser = argparse.ArgumentParser(description="RestedRealm local collection companion")
    parser.add_argument("--game", type=Path, default=DEFAULT_GAME, help="Forever installation directory")
    parser.add_argument("--state", type=Path, default=DEFAULT_STATE, help="private local queue directory")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("scan", help="copy observations from a completed game save")
    commands.add_parser("status", help="show local queue status")
    listing = commands.add_parser("preview", help="show IDs, kinds and dates without private text")
    listing.add_argument("--limit", type=int, default=20)
    watch = commands.add_parser("watch", help="check for completed saves until Ctrl+C")
    watch.add_argument("--interval", type=int, default=30)
    forget = commands.add_parser("forget", help="delete the companion queue; the game save is untouched")
    forget.add_argument("--yes", action="store_true")
    rollover = commands.add_parser("rollover", help="archive and clear already queued records after WoW exits")
    rollover.add_argument("--yes", action="store_true")
    args = parser.parse_args()
    database = connect(args.state)
    try:
        if args.command == "scan":
            worked = scan(database, args.game)
            status(database)
            if not worked:
                raise SystemExit(1)
        elif args.command == "status":
            status(database)
        elif args.command == "preview":
            preview(database, max(1, min(args.limit, 100)))
        elif args.command == "watch":
            print("Watching completed Forever saves. Press Ctrl+C to stop.")
            try:
                while True:
                    scan(database, args.game)
                    time.sleep(max(10, args.interval))
            except KeyboardInterrupt:
                print("Watcher stopped. Queued observations remain on this PC.")
        elif args.command == "forget":
            if not args.yes:
                parser.error("forget requires --yes")
            forget_queue(database)
            print("Companion queue forgotten. The WoW SavedVariables file was not changed.")
        elif args.command == "rollover":
            if not args.yes:
                parser.error("rollover requires --yes")
            worked = False
            for path in saved_files(args.game):
                try:
                    count = compact_one(database, path, args.state)
                    print(f"Rolled over {count} safely queued observations; a private backup was kept.")
                    worked = True
                except (OSError, UnicodeError, ValueError, RuntimeError) as exc:
                    print(f"Rollover skipped: {type(exc).__name__}: {exc}", file=sys.stderr)
            if not worked:
                raise SystemExit(1)
    finally:
        database.close()


if __name__ == "__main__":
    main()
