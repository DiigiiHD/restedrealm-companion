"""Owner pilot pairing and prepared upload transport."""

import hashlib
import json
import re
import sqlite3
import urllib.error
import urllib.request

import windows_credentials


BASE_URL = "https://restedrealm.com"
# Pairing and private upload passed the owner pilot check on 25 September 2026.
WEBSITE_READY = True
UPLOAD_READY = True
PROSE_FIELDS = {
    "dialogue", "greeting", "questText", "objectiveText", "progressText",
    "rewardText", "description", "confirmationText", "tooltipProbe", "text", "optionName",
}
_REMOVE = object()


def redact(value, path=()):
    if isinstance(value, dict):
        out = {}
        for key, item in value.items():
            if key in PROSE_FIELDS or (key == "name" and "options" in path):
                continue
            cleaned = redact(item, path + (key,))
            if cleaned is not _REMOVE:
                out[key] = cleaned
        return out
    if isinstance(value, list):
        return [cleaned for item in value if (cleaned := redact(item, path)) is not _REMOVE]
    if isinstance(value, str) and len(value) > 500:
        return _REMOVE
    return value


def prepare_batch(database: sqlite3.Connection, limit=50):
    rows = database.execute(
        """SELECT source_id,seq,digest,payload FROM observations
           WHERE uploaded_digest IS NULL OR uploaded_digest<>digest
           ORDER BY queued_at,source_id,seq LIMIT ?""", (min(max(limit, 1), 100),),
    ).fetchall()
    output = []
    acknowledged = []
    size = len('{"version":1,"records":[]}')
    for source_id, seq, original_digest, raw in rows:
        record = redact(json.loads(raw))
        canonical = json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        item = {"sourceId": source_id, "seq": seq,
                "digest": hashlib.sha256(canonical.encode("utf-8")).hexdigest(), "payload": canonical}
        encoded = json.dumps(item, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(canonical) > 100_000:
            raise ValueError(f"observation {seq} is too large for the server")
        if size + len(encoded) + 1 > 1_000_000:
            break
        output.append(item)
        acknowledged.append((source_id, seq, original_digest))
        size += len(encoded) + 1
    return {"version": 1, "records": output}, acknowledged


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        raise ValueError("RestedRealm changed the upload destination; no credential was forwarded")


def post_json(path: str, payload, credential=None):
    if not path.startswith("/api/collector/") or "//" in path:
        raise ValueError("invalid Collector endpoint")
    body = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    headers = {"Content-Type": "application/json", "Accept": "application/json"}
    if credential:
        if not re.fullmatch(r"rrc_[A-Za-z0-9_-]{43}", credential):
            raise ValueError("stored Collector credential is invalid")
        headers["Authorization"] = "Bearer " + credential
    request = urllib.request.Request(BASE_URL + path, body, headers, method="POST")
    opener = urllib.request.build_opener(NoRedirect())
    try:
        with opener.open(request, timeout=20) as response:
            if response.status != 200:
                raise ValueError("RestedRealm did not accept the request")
            return json.loads(response.read(1_048_577).decode("utf-8"))
    except urllib.error.HTTPError as exc:
        # Never print the request or Authorization header.
        try:
            detail = json.loads(exc.read(4096).decode("utf-8")).get("error")
        except (ValueError, UnicodeError):
            detail = None
        raise RuntimeError(detail or f"RestedRealm returned HTTP {exc.code}") from None


def redeem_code(code: str, device_name="Windows PC"):
    code = code.strip()
    if not re.fullmatch(r"[A-Za-z0-9_-]{16}", code):
        raise ValueError("Enter the 16-character code from your RestedRealm account page")
    response = post_json("/api/collector/redeem", {"code": code, "name": device_name})
    if not re.fullmatch(r"rrc_[A-Za-z0-9_-]{43}", response.get("token", "")):
        raise ValueError("RestedRealm returned an invalid device credential")
    if not isinstance(response.get("deviceId"), str):
        raise ValueError("RestedRealm did not identify the paired device")
    windows_credentials.save(response["deviceId"], response["token"])
    return response["deviceId"]


def upload_once(database: sqlite3.Connection):
    if not UPLOAD_READY:
        raise RuntimeError("Gameplay upload is locked until the owner pilot check is complete")
    credential = windows_credentials.load()
    if not credential:
        raise ValueError("Connect this device to RestedRealm before uploading")
    batch, acknowledged = prepare_batch(database)
    if not acknowledged:
        return 0
    response = post_json("/api/collector/batches", batch, credential["token"])
    if (type(response.get("accepted")) is not int or type(response.get("duplicate")) is not int
        or response["accepted"] + response["duplicate"] != len(acknowledged)):
        raise RuntimeError("Upload response was incomplete; all observations remain queued")
    with database:
        for source_id, seq, original_digest in acknowledged:
            database.execute(
                """UPDATE observations SET uploaded_digest=? WHERE source_id=? AND seq=? AND digest=?""",
                (original_digest, source_id, seq, original_digest),
            )
    return len(acknowledged)
