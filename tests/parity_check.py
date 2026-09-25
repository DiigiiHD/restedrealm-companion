"""Check that the Rust core and the Python companion agree byte for byte.

Both implementations import the same generated saves; every queued digest and
payload, and the exact upload batch, must match. Run from the repository root
after `cargo build --manifest-path app/Cargo.toml`:

    python tests/parity_check.py [--rounds 300] [--seed 1]
"""

import argparse
import json
import math
import os
from pathlib import Path
import random
import sqlite3
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "companion"))
from restedrealm_companion import connect, scan_one  # noqa: E402
from uploader import prepare_batch  # noqa: E402

BINARY = ROOT / "app" / "target" / "debug" / ("rrc-core.exe" if os.name == "nt" else "rrc-core")
WORDS = ["title", "npc", "quest", "options", "name", "dialogue", "text", "id", "x", "y", "zone", "é", "日本", "a b"]
PROSE = ["dialogue", "greeting", "questText", "text", "description"]


def lua_string(text, rng):
    out = []
    for ch in text:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append(rng.choice(["\\n", "\\\n"]))
        elif ord(ch) < 32:
            out.append("\\%03d" % ord(ch))
        else:
            out.append(ch)
    return '"' + "".join(out) + '"'


def random_text(rng):
    pool = "abc XYZ 123 éüß 日本語 🐉 \"'\\\n\t\x01\x1f\x7f,;{}[]=--"
    return "".join(rng.choice(pool) for _ in range(rng.randint(0, 24)))


def random_float(rng):
    kind = rng.random()
    if kind < 0.3:
        return rng.uniform(-1000, 1000)
    if kind < 0.6:
        return rng.choice([-1, 1]) * 10 ** rng.uniform(-12, 22)
    if kind < 0.8:
        return float(rng.randint(-10**6, 10**6))
    return rng.choice([0.1, 0.5, 1e-5, 1e16, 1e-4, 1234567890123456.0, 2.5e-300, 1.7976931348623157e308, -0.0])


def lua_float(value, rng):
    text = repr(value)
    if "e" not in text and "." in text and rng.random() < 0.2:
        text = text.rstrip("0") if not text.endswith(".0") else text
    return text


def lua_value(rng, depth):
    roll = rng.random()
    if depth > 3 or roll < 0.25:
        return lua_string(random_text(rng), rng)
    if roll < 0.4:
        return str(rng.randint(-10**12, 10**12))
    if roll < 0.55:
        return lua_float(random_float(rng), rng)
    if roll < 0.6:
        return rng.choice(["true", "false"])
    if roll < 0.75:
        items = [lua_value(rng, depth + 1) for _ in range(rng.randint(0, 5))]
        return "{ " + ", ".join(items) + (", " if items and rng.random() < 0.5 else "") + "}"
    if roll < 0.85:
        keys = rng.sample(range(1, 40), rng.randint(1, 5))
        return "{ " + ", ".join("[%d] = %s" % (k, lua_value(rng, depth + 1)) for k in keys) + " }"
    return lua_table(rng, depth + 1)


def lua_table(rng, depth, extra=None):
    keys = list(dict.fromkeys(rng.choice(WORDS) + str(rng.randint(0, 3)) for _ in range(rng.randint(0, 6))))
    parts = []
    for key in keys:
        value = lua_value(rng, depth)
        if key.isidentifier() and key.isascii() and rng.random() < 0.5:
            parts.append("%s = %s" % (key, value))
        else:
            parts.append("[%s] = %s" % (lua_string(key, rng), value))
    for key, value in (extra or {}).items():
        parts.append("[%s] = %s" % (lua_string(key, rng), value))
    rng.shuffle(parts)
    sep = rng.choice([", ", ",\n  ", "; ", " -- note\n  , "])
    return "{ " + sep.join(parts) + " }"


def record(rng, seq):
    extra = {}
    if rng.random() < 0.5:
        extra[rng.choice(PROSE)] = lua_string(random_text(rng) * rng.randint(1, 40), rng)
    if rng.random() < 0.4:
        extra["options"] = "{ { id = 4, name = %s }, { id = 5 } }" % lua_string(random_text(rng), rng)
    if rng.random() < 0.2:
        extra["long"] = lua_string("x" * rng.choice([499, 500, 501, 900]), rng)
    data = lua_table(rng, 1, extra)
    context = '{ ["product"] = "wow_classic_beta", ["build"] = "70009", ["locale"] = "enUS" }'
    fields = ['["seq"] = %d' % seq, '["kind"] = %s' % lua_string(rng.choice(["gossip", "quest_objectives", "merchant", "npc"]), rng),
              '["observedAt"] = %d' % rng.randint(1_700_000_000, 1_800_000_000), '["context"] = ' + context,
              '["data"] = ' + data]
    rng.shuffle(fields)
    return "{ " + ", ".join(fields) + " }"


def save(rng, count):
    records = ",\n ".join(record(rng, seq) for seq in range(1, count + 1))
    bom = "﻿" if rng.random() < 0.2 else ""
    return bom + "RestedRealmCollectorDB = {\n [\"schema\"] = 1, [\"dropped\"] = %d,\n [\"records\"] = {\n %s\n },\n}\n" % (
        rng.randint(0, 3), records)


def damage(text, rng):
    choice = rng.randrange(8)
    if choice == 0:
        return text[:rng.randrange(len(text))]
    if choice == 1:
        return text + 'os.execute("whoami")'
    if choice == 2:
        return text.replace('["seq"] = 2', '["seq"] = 1')
    if choice == 3:
        return text.replace("wow_classic_beta", "wow", 1)
    if choice == 4:
        return text.replace('["schema"] = 1', '["schema"] = "\\x41"')
    if choice == 5:
        return text.replace('["schema"] = 1', '["schema"] = { [1] = 1, a = 2 }')
    if choice == 6:
        return text.replace('["seq"] = 1', '["seq"] = 0')
    return text.replace('["schema"] = 1', '["schema"] = "\\300"')


def rust(game, state, *command):
    result = subprocess.run([str(BINARY), "--game", str(game), "--state", str(state), "--delay", "0", *command],
                            capture_output=True, text=True, encoding="utf-8")
    return result


def rows(state):
    with sqlite3.connect(state / "queue.sqlite3") as db:
        return db.execute("SELECT source_id, seq, digest, payload FROM observations ORDER BY source_id, seq").fetchall()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rounds", type=int, default=300)
    parser.add_argument("--seed", type=int, default=1)
    args = parser.parse_args()
    if not BINARY.exists():
        raise SystemExit(f"Build the Rust core first: {BINARY} is missing")
    rng = random.Random(args.seed)
    compared = rejected = 0
    for round_number in range(args.rounds):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            saved = root / "game" / "WTF" / "Account" / "ACCOUNT" / "SavedVariables"
            saved.mkdir(parents=True)
            path = saved / "RestedRealmCollector.lua"
            text = save(rng, rng.randint(1, 6))
            if rng.random() < 0.2:
                text = damage(text, rng)
            path.write_text(text, encoding="utf-8")
            python_db = connect(root / "python")
            try:
                python_error = None
                scan_one(python_db, path, delay=0)
            except Exception as exc:  # noqa: BLE001 - any refusal must be matched
                python_error = exc
            result = rust(root / "game", root / "rust", "scan")
            rust_ok = result.returncode == 0
            if (python_error is None) != rust_ok:
                raise SystemExit(f"round {round_number}: Python {'failed: ' + repr(python_error) if python_error else 'passed'},"
                                 f" Rust {'passed' if rust_ok else 'failed: ' + result.stderr.strip()}\n{path.read_text(encoding='utf-8')[:2000]}")
            if python_error is not None:
                rejected += 1
                python_db.close()
                continue
            left, right = rows(root / "python"), rows(root / "rust")
            if left != right:
                for a, b in zip(left, right):
                    if a != b:
                        raise SystemExit(f"round {round_number}: queued record differs\nPython: {a}\nRust:   {b}")
                raise SystemExit(f"round {round_number}: record counts differ, {len(left)} and {len(right)}")
            batch, _ = prepare_batch(python_db)
            python_body = json.dumps(batch, ensure_ascii=False, separators=(",", ":"))
            python_db.close()
            # The batch uses the Python queue's source IDs, so run Rust against a copy of it.
            rust_batch = rust(root / "game", root / "python", "batch")
            if rust_batch.returncode != 0 or rust_batch.stdout.rstrip("\n") != python_body:
                raise SystemExit(f"round {round_number}: upload batch differs\nPython: {python_body[:3000]}\nRust:   {rust_batch.stdout[:3000]}{rust_batch.stderr}")
            compared += len(left)
    print(f"Parity OK: {compared} records matched across {args.rounds} saves; {rejected} saves rejected by both.")


if __name__ == "__main__":
    main()
