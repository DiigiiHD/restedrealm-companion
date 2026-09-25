"""Send one clearly synthetic private batch through the paired upload path."""

import hashlib
import json
import secrets
import time

import uploader
import windows_credentials


def check():
    if not uploader.WEBSITE_READY or uploader.UPLOAD_READY:
        raise RuntimeError("Run the pilot check only while gameplay uploads are locked")
    credential = windows_credentials.load()
    if not credential:
        raise RuntimeError("Pair this PC from the Collector window before running the pilot check")
    source_id = secrets.token_hex(32)
    record = {
        "seq": 1,
        "kind": "entity_sighting",
        "observedAt": int(time.time()),
        "context": {"product": "wow_classic_beta", "build": "70009", "locale": "enUS"},
        "data": {"pilotCheck": True},
    }
    payload = json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    batch = {"version": 1, "records": [{
        "sourceId": source_id,
        "seq": 1,
        "digest": hashlib.sha256(payload.encode("utf-8")).hexdigest(),
        "payload": payload,
    }]}
    first = uploader.post_json("/api/collector/batches", batch, credential["token"])
    if first.get("accepted") != 1 or first.get("duplicate") != 0:
        raise RuntimeError("The synthetic batch was not accepted exactly once")
    second = uploader.post_json("/api/collector/batches", batch, credential["token"])
    if second.get("accepted") != 0 or second.get("duplicate") != 1:
        raise RuntimeError("The synthetic retry was not recognized as a duplicate")
    print("Synthetic batch accepted once and deduplicated on retry.")
    print("Synthetic source ID for private server cleanup:", source_id)


if __name__ == "__main__":
    check()
