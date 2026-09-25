import json
import sqlite3
import sys
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "companion"))
from restedrealm_companion import compact_one, connect, forget_queue, scan_one
from save_parser import SaveFormatError, parse_save
from uploader import prepare_batch, upload_once


def sample(seq=1, title="First"):
    return ("RestedRealmCollectorDB = {\n"
            " [\"schema\"] = 1, [\"dropped\"] = 0, [\"records\"] = {\n"
            "  { [\"seq\"] = %d, [\"kind\"] = \"gossip\",\n"
            "    [\"context\"] = { [\"product\"] = \"wow_classic_beta\", [\"build\"] = \"70009\", [\"locale\"] = \"enUS\" },\n"
            "    [\"data\"] = { [\"title\"] = \"%s\" } },\n"
            " },\n}\n") % (seq, title)


class ParserTests(unittest.TestCase):
    def test_safe_data_types_and_escapes(self):
        db = parse_save('RestedRealmCollectorDB = { ["a"] = {1, 2}, '
                        '["b"] = "line\\ntext", ["c"] = false, ["d"] = -2.5 } -- end')
        self.assertEqual(db["a"], [1, 2])
        self.assertEqual(db["b"], "line\ntext")
        self.assertEqual(db["c"], False)
        self.assertEqual(db["d"], -2.5)

    def test_code_is_rejected(self):
        with self.assertRaises(SaveFormatError):
            parse_save(sample() + 'os.execute("whoami")')
        with self.assertRaises(SaveFormatError):
            parse_save('RestedRealmCollectorDB = { ["x"] = os.execute("whoami") }')


class QueueTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.path = self.root / "RestedRealmCollector.lua"
        self.database = connect(self.root / "state")

    def tearDown(self):
        self.database.close()
        self.tmp.cleanup()

    def count(self):
        return self.database.execute("SELECT COUNT(*) FROM observations").fetchone()[0]

    def test_repeat_restart_and_corrected_save(self):
        self.path.write_text(sample(), encoding="utf-8")
        self.assertEqual(scan_one(self.database, self.path, delay=0)["new"], 1)
        self.assertTrue(scan_one(self.database, self.path, delay=0)["unchanged"])
        self.database.close()
        self.database = connect(self.root / "state")
        self.assertEqual(self.count(), 1)
        self.path.write_text(sample(title="Corrected"), encoding="utf-8")
        self.assertEqual(scan_one(self.database, self.path, delay=0)["updated"], 1)
        self.assertEqual(self.count(), 1)
        self.assertIn("Corrected", self.database.execute("SELECT payload FROM observations").fetchone()[0])

    def test_incomplete_save_does_not_change_queue(self):
        self.path.write_text(sample(), encoding="utf-8")
        scan_one(self.database, self.path, delay=0)
        self.path.write_text('RestedRealmCollectorDB = { ["records"] = {', encoding="utf-8")
        with self.assertRaises(SaveFormatError):
            scan_one(self.database, self.path, delay=0)
        self.assertEqual(self.count(), 1)

    def test_new_record_preserves_old_one(self):
        self.path.write_text(sample(), encoding="utf-8")
        scan_one(self.database, self.path, delay=0)
        self.path.write_text(sample(seq=2), encoding="utf-8")
        self.assertEqual(scan_one(self.database, self.path, delay=0)["new"], 1)
        self.assertEqual(self.count(), 2)

    def test_rollover_only_after_archiving_and_game_exit(self):
        self.path.write_text(sample(), encoding="utf-8")
        with self.assertRaises(RuntimeError):
            compact_one(self.database, self.path, self.root / "state", delay=0, game_running=lambda: True)
        with self.assertRaises(RuntimeError):
            compact_one(self.database, self.path, self.root / "state", delay=0, game_running=lambda: False)
        scan_one(self.database, self.path, delay=0)
        self.assertEqual(compact_one(self.database, self.path, self.root / "state", delay=0,
                                     game_running=lambda: False), 1)
        self.assertEqual(parse_save(self.path.read_text(encoding="utf-8"))["records"], {})
        self.assertEqual(self.count(), 1)
        self.assertEqual(len(list((self.root / "state" / "Backups").glob("*.lua"))), 1)

    def test_upload_acks_only_after_complete_response(self):
        self.path.write_text(sample(), encoding="utf-8")
        scan_one(self.database, self.path, delay=0)
        batch, rows = prepare_batch(self.database)
        self.assertEqual(len(rows), 1)
        self.assertEqual(batch["version"], 1)
        with patch("uploader.UPLOAD_READY", True), patch("uploader.windows_credentials.load", return_value={"token": "rrc_" + "a" * 43}):
            with patch("uploader.post_json", side_effect=TimeoutError):
                with self.assertRaises(TimeoutError):
                    upload_once(self.database)
            self.assertIsNone(self.database.execute("SELECT uploaded_digest FROM observations").fetchone()[0])
            with patch("uploader.post_json", return_value={"accepted": 1, "duplicate": 0}):
                self.assertEqual(upload_once(self.database), 1)
            self.assertIsNotNone(self.database.execute("SELECT uploaded_digest FROM observations").fetchone()[0])

    def test_gameplay_upload_stays_locked_during_pairing(self):
        with patch("uploader.UPLOAD_READY", False), patch("uploader.windows_credentials.load", side_effect=AssertionError("credential should not be read")):
            with self.assertRaisesRegex(RuntimeError, "locked"):
                upload_once(self.database)

    def test_prose_is_removed_from_transport_only(self):
        self.path.write_text(sample(title="Local text"), encoding="utf-8")
        scan_one(self.database, self.path, delay=0)
        with self.database:
            row = self.database.execute("SELECT source_id,seq,payload FROM observations").fetchone()
            record = json.loads(row[2])
            record["data"]["dialogue"] = "Full NPC dialogue stays local"
            record["data"]["options"] = [{"id": 4, "name": "A dialogue choice"}]
            raw = json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
            import hashlib
            digest = hashlib.sha256(raw.encode("utf-8")).hexdigest()
            self.database.execute("UPDATE observations SET digest=?,payload=? WHERE source_id=? AND seq=?",
                                  (digest, raw, row[0], row[1]))
        batch, _ = prepare_batch(self.database)
        sent = json.loads(batch["records"][0]["payload"])
        self.assertNotIn("dialogue", sent["data"])
        self.assertNotIn("name", sent["data"]["options"][0])
        self.assertIn("dialogue", json.loads(self.database.execute("SELECT payload FROM observations").fetchone()[0])["data"])

    def test_forget_queue_disables_upload_and_preserves_game_save(self):
        self.path.write_text(sample(), encoding="utf-8")
        scan_one(self.database, self.path, delay=0)
        self.database.execute("INSERT INTO settings(key,value) VALUES('upload_opt_in','1')")
        self.database.commit()
        forget_queue(self.database)
        self.assertEqual(self.count(), 0)
        self.assertIsNone(self.database.execute("SELECT value FROM settings WHERE key='upload_opt_in'").fetchone())
        self.assertEqual(len(parse_save(self.path.read_text(encoding="utf-8"))["records"]), 1)


if __name__ == "__main__":
    unittest.main()
