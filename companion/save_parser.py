"""Read WoW SavedVariables as data. This module never executes Lua."""

import math
import re


class SaveFormatError(ValueError):
    pass


_NUMBER = re.compile(r"-?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?")
_IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z_0-9]*")


class Parser:
    def __init__(self, source: str):
        self.source = source.lstrip("\ufeff")
        self.at = 0
        self.nodes = 0
        self.records_span = None

    def fail(self, reason: str):
        raise SaveFormatError(f"{reason} at offset {self.at}")

    def space(self):
        s = self.source
        while self.at < len(s):
            if s[self.at].isspace():
                self.at += 1
            elif s.startswith("--", self.at):
                end = s.find("\n", self.at)
                self.at = len(s) if end < 0 else end + 1
            else:
                break

    def take(self, expected: str):
        self.space()
        if not self.source.startswith(expected, self.at):
            self.fail(f"expected {expected!r}")
        self.at += len(expected)

    def identifier(self):
        self.space()
        match = _IDENTIFIER.match(self.source, self.at)
        if not match:
            self.fail("expected identifier")
        self.at = match.end()
        return match.group()

    def quoted(self):
        s = self.source
        quote = s[self.at]
        self.at += 1
        out = []
        escaped = {"a": "\a", "b": "\b", "f": "\f", "n": "\n", "r": "\r",
                   "t": "\t", "v": "\v", "\\": "\\", '"': '"', "'": "'"}
        while self.at < len(s):
            ch = s[self.at]
            self.at += 1
            if ch == quote:
                return "".join(out)
            if ch == "\\":
                if self.at >= len(s):
                    self.fail("unfinished escape")
                ch = s[self.at]
                self.at += 1
                if ch.isascii() and ch.isdigit():
                    start = self.at - 1
                    while self.at < min(len(s), start + 3) and s[self.at].isascii() and s[self.at].isdigit():
                        self.at += 1
                    value = int(s[start:self.at])
                    if value > 255:
                        self.fail("invalid byte escape")
                    out.append(chr(value))
                elif ch in escaped:
                    out.append(escaped[ch])
                elif ch == "\n":
                    out.append("\n")
                else:
                    self.fail("unsupported escape")
            else:
                out.append(ch)
        self.fail("unfinished string")

    def value(self, depth=0):
        self.space()
        self.nodes += 1
        if self.nodes > 500_000 or depth > 64:
            self.fail("save complexity limit exceeded")
        s = self.source
        if self.at >= len(s):
            self.fail("unexpected end")
        ch = s[self.at]
        if ch == "{":
            return self.table(depth + 1)
        if ch in "\"'":
            return self.quoted()
        if ch == "-" or ch.isdigit() or ch == ".":
            match = _NUMBER.match(s, self.at)
            if not match:
                self.fail("bad number")
            self.at = match.end()
            value = float(match.group()) if any(c in match.group() for c in ".eE") else int(match.group())
            if isinstance(value, float) and not math.isfinite(value):
                self.fail("nonfinite number")
            return value
        word = self.identifier()
        if word == "true":
            return True
        if word == "false":
            return False
        if word == "nil":
            return None
        self.fail("unexpected name")

    def table(self, depth):
        self.take("{")
        entries = {}
        next_index = 1
        while True:
            self.space()
            if self.at >= len(self.source):
                self.fail("unfinished table")
            if self.source[self.at] == "}":
                self.at += 1
                return self.normalize(entries)
            if self.source[self.at] == "[":
                self.at += 1
                key = self.value(depth)
                self.take("]")
                self.take("=")
                if depth == 1 and key == "records":
                    self.space()
                    start = self.at
                value = self.value(depth)
            else:
                mark = self.at
                match = _IDENTIFIER.match(self.source, self.at)
                if match:
                    self.at = match.end()
                    self.space()
                if match and self.at < len(self.source) and self.source[self.at] == "=":
                    key = match.group()
                    self.at += 1
                    if depth == 1 and key == "records":
                        self.space()
                        start = self.at
                    value = self.value(depth)
                else:
                    self.at = mark
                    key = next_index
                    next_index += 1
                    value = self.value(depth)
            if depth == 1 and key == "records":
                self.records_span = (start, self.at)
            if key is None or isinstance(key, (dict, list)):
                self.fail("invalid table key")
            if key in entries:
                self.fail("duplicate table key")
            entries[key] = value
            self.space()
            if self.at < len(self.source) and self.source[self.at] in ",;":
                self.at += 1
            elif self.at >= len(self.source) or self.source[self.at] != "}":
                self.fail("expected table separator")

    @staticmethod
    def normalize(entries):
        if entries and all(type(k) is int and k > 0 for k in entries):
            if len(entries) == max(entries):
                return [entries[i] for i in range(1, len(entries) + 1)]
        return entries


def parse_save(source: str):
    return parse_save_with_span(source)[0]


def parse_save_with_span(source: str):
    parser = Parser(source)
    name = parser.identifier()
    if name != "RestedRealmCollectorDB":
        raise SaveFormatError("not a RestedRealmCollector SavedVariables file")
    parser.take("=")
    db = parser.value()
    parser.space()
    if parser.at != len(parser.source):
        parser.fail("trailing code")
    if not isinstance(db, dict):
        raise SaveFormatError("collector root must be a table")
    return db, parser.records_span
