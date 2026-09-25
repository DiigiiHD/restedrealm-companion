import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "companion"))
from pilot_check import check


class PilotCheckTests(unittest.TestCase):
    def test_only_synthetic_private_batch_is_sent_and_retried(self):
        sent = []

        def post(path, batch, token):
            self.assertEqual(path, "/api/collector/batches")
            self.assertEqual(token, "test-token")
            sent.append(batch)
            return {"accepted": 1, "duplicate": 0} if len(sent) == 1 else {"accepted": 0, "duplicate": 1}

        with patch("pilot_check.windows_credentials.load", return_value={"token": "test-token"}), \
             patch("pilot_check.uploader.UPLOAD_READY", False), \
             patch("pilot_check.uploader.post_json", side_effect=post), \
             patch("pilot_check.secrets.token_hex", return_value="a" * 64), \
             patch("pilot_check.time.time", return_value=1_000):
            check()
        self.assertEqual(sent[0], sent[1])
        record = json.loads(sent[0]["records"][0]["payload"])
        self.assertEqual(record["data"], {"pilotCheck": True})
        self.assertNotIn("npc", record["data"])


if __name__ == "__main__":
    unittest.main()
