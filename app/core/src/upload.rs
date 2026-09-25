//! Pairing and upload. Full NPC and quest prose stays in the local queue; only
//! the redacted record travels. A record counts as uploaded only after the
//! website confirms the whole batch.

use crate::canon::{json_to_canonical, sha256_hex, write_string};
use crate::credentials::{Credential, CredentialStore};
use crate::queue::{Queue, PENDING};
use crate::{Error, Result};
use rusqlite::params;
use serde_json::Value as Json;
use std::io::Read;
use std::time::Duration;

pub const BASE_URL: &str = "https://restedrealm.com";
pub const PROSE_FIELDS: &[&str] = &[
    "dialogue",
    "greeting",
    "questText",
    "objectiveText",
    "progressText",
    "rewardText",
    "description",
    "confirmationText",
    "tooltipProbe",
    "text",
    "optionName",
];
const MAX_TEXT_CHARS: usize = 500;
const MAX_RECORD_CHARS: usize = 100_000;
const MAX_BATCH_BYTES: usize = 1_000_000;
const MAX_RESPONSE_BYTES: u64 = 1_048_577;

/// Drop prose fields, option names and any string over 500 characters.
pub fn redact(value: &Json) -> Json {
    redact_inner(value, false).unwrap_or(Json::Null)
}

fn redact_inner(value: &Json, under_options: bool) -> Option<Json> {
    match value {
        Json::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map {
                if PROSE_FIELDS.contains(&key.as_str()) || (key == "name" && under_options) {
                    continue;
                }
                if let Some(cleaned) = redact_inner(item, under_options || key == "options") {
                    out.insert(key.clone(), cleaned);
                }
            }
            Some(Json::Object(out))
        }
        Json::Array(items) => Some(Json::Array(items.iter().filter_map(|i| redact_inner(i, under_options)).collect())),
        Json::String(s) if s.chars().count() > MAX_TEXT_CHARS => None,
        other => Some(other.clone()),
    }
}

pub struct BatchItem {
    pub source_id: String,
    pub seq: i64,
    pub digest: String,
    pub payload: String,
}

impl BatchItem {
    fn encode(&self) -> String {
        let mut out = String::from("{\"sourceId\":");
        write_string(&mut out, &self.source_id);
        out.push_str(&format!(",\"seq\":{},\"digest\":", self.seq));
        write_string(&mut out, &self.digest);
        out.push_str(",\"payload\":");
        write_string(&mut out, &self.payload);
        out.push('}');
        out
    }
}

pub struct Batch {
    pub items: Vec<BatchItem>,
    /// (source_id, seq, digest of the queued record) to mark after success.
    pub acknowledged: Vec<(String, i64, String)>,
}

impl Batch {
    pub fn body(&self) -> String {
        let items: Vec<String> = self.items.iter().map(BatchItem::encode).collect();
        format!("{{\"version\":1,\"records\":[{}]}}", items.join(","))
    }
}

/// The oldest pending records that fit in one request.
pub fn prepare_batch(queue: &Queue, limit: i64) -> Result<Batch> {
    let mut statement = queue.db.prepare(&format!(
        "SELECT o.source_id,o.seq,o.digest,o.payload FROM observations o
         WHERE {PENDING} ORDER BY o.queued_at,o.source_id,o.seq LIMIT ?"
    ))?;
    let rows = statement
        .query_map([limit.clamp(1, 100)], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut batch = Batch { items: Vec::new(), acknowledged: Vec::new() };
    let mut size = "{\"version\":1,\"records\":[]}".len();
    for (source_id, seq, original_digest, raw) in rows {
        let record: Json =
            serde_json::from_str(&raw).map_err(|_| Error::Upload(format!("queued observation {seq} is damaged")))?;
        let payload = json_to_canonical(&redact(&record));
        let item = BatchItem { digest: sha256_hex(payload.as_bytes()), source_id: source_id.clone(), seq, payload };
        let encoded = item.encode().len();
        if item.payload.chars().count() > MAX_RECORD_CHARS {
            return Err(Error::Upload(format!("observation {seq} is too large for the server")));
        }
        if size + encoded + 1 > MAX_BATCH_BYTES {
            break;
        }
        size += encoded + 1;
        batch.items.push(item);
        batch.acknowledged.push((source_id, seq, original_digest));
    }
    Ok(batch)
}

/// How requests reach the website. Tests use a fake.
pub trait Transport {
    fn post(&self, path: &str, body: &str, credential: Option<&str>) -> Result<Json>;
}

pub struct Https {
    pub base_url: String,
    agent: ureq::Agent,
}

impl Https {
    pub fn new(base_url: &str) -> Https {
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(Duration::from_secs(20))
            .user_agent(&format!("RestedRealmCompanion/{}", crate::VERSION))
            .build();
        Https { base_url: base_url.trim_end_matches('/').to_string(), agent }
    }
}

impl Default for Https {
    /// restedrealm.com. Development builds can point at a local site with
    /// `RRC_BASE_URL`; release builds ignore it.
    fn default() -> Https {
        #[cfg(debug_assertions)]
        if let Ok(url) = std::env::var("RRC_BASE_URL") {
            return Https::new(&url);
        }
        Https::new(BASE_URL)
    }
}

fn valid_credential(token: &str) -> bool {
    token.len() == 47
        && token.starts_with("rrc_")
        && token[4..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn read_json(response: ureq::Response) -> Option<Json> {
    let mut text = String::new();
    response.into_reader().take(MAX_RESPONSE_BYTES).read_to_string(&mut text).ok()?;
    serde_json::from_str(&text).ok()
}

impl Transport for Https {
    fn post(&self, path: &str, body: &str, credential: Option<&str>) -> Result<Json> {
        if !path.starts_with("/api/collector/") || path.contains("//") {
            return Err(Error::Upload("invalid Collector endpoint".into()));
        }
        let mut request = self
            .agent
            .post(&format!("{}{}", self.base_url, path))
            .set("Content-Type", "application/json")
            .set("Accept", "application/json");
        if let Some(token) = credential {
            if !valid_credential(token) {
                return Err(Error::Upload("stored RestedRealm credential is invalid".into()));
            }
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        // Never include the request or the Authorization header in an error.
        match request.send_string(body) {
            Ok(response) if response.status() == 200 => {
                read_json(response).ok_or_else(|| Error::Upload("RestedRealm sent an unreadable answer".into()))
            }
            Ok(response) if (300..400).contains(&response.status()) => {
                Err(Error::Upload("RestedRealm changed the upload destination; no credential was forwarded".into()))
            }
            Ok(_) => Err(Error::Upload("RestedRealm did not accept the request".into())),
            Err(ureq::Error::Status(code, response)) => {
                let detail =
                    read_json(response).and_then(|j| j.get("error").and_then(Json::as_str).map(str::to_string));
                Err(Error::Upload(detail.unwrap_or_else(|| format!("RestedRealm returned HTTP {code}"))))
            }
            Err(ureq::Error::Transport(_)) => Err(Error::Upload("Could not reach RestedRealm".into())),
        }
    }
}

/// Redeem a one-time code from the account page and keep the device credential.
pub fn redeem_code(
    transport: &dyn Transport,
    store: &dyn CredentialStore,
    code: &str,
    device_name: &str,
) -> Result<String> {
    let code = code.trim();
    if code.len() != 16 || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return Err(Error::Upload("Enter the 16-character code from your RestedRealm account page".into()));
    }
    let mut body = String::from("{\"code\":");
    write_string(&mut body, code);
    body.push_str(",\"name\":");
    write_string(&mut body, device_name);
    body.push('}');
    let response = transport.post("/api/collector/redeem", &body, None)?;
    let token = response.get("token").and_then(Json::as_str).unwrap_or_default();
    if !valid_credential(token) {
        return Err(Error::Upload("RestedRealm returned an invalid device credential".into()));
    }
    let Some(device_id) = response.get("deviceId").and_then(Json::as_str) else {
        return Err(Error::Upload("RestedRealm did not identify the paired device".into()));
    };
    store.save(&Credential { device_id: device_id.to_string(), token: token.to_string() })?;
    Ok(device_id.to_string())
}

/// Send one batch. Returns how many records the website confirmed.
pub fn upload_once(queue: &mut Queue, transport: &dyn Transport, store: &dyn CredentialStore) -> Result<usize> {
    let Some(credential) = store.load()? else {
        return Err(Error::Upload("Connect this PC to RestedRealm before uploading".into()));
    };
    let batch = prepare_batch(queue, 50)?;
    if batch.acknowledged.is_empty() {
        return Ok(0);
    }
    let response = transport.post("/api/collector/batches", &batch.body(), Some(&credential.token))?;
    let count = |key: &str| response.get(key).and_then(Json::as_i64);
    // The website names records it will not store; they are kept here and not sent again.
    let mut refused: Vec<(String, i64, String)> = Vec::new();
    if let Some(list) = response.get("rejected").and_then(Json::as_array) {
        for entry in list {
            let source = entry.get("sourceId").and_then(Json::as_str).unwrap_or_default();
            let seq = entry.get("seq").and_then(Json::as_i64).unwrap_or(0);
            let reason = entry.get("reason").and_then(Json::as_str).unwrap_or("Refused by RestedRealm");
            if !batch.acknowledged.iter().any(|(s, q, _)| s == source && *q == seq) {
                return Err(Error::Upload(
                    "Upload response named a record that was not sent; all observations remain queued".into(),
                ));
            }
            refused.push((source.to_string(), seq, reason.chars().take(200).collect()));
        }
    }
    match (count("accepted"), count("duplicate")) {
        (Some(a), Some(d)) if a >= 0 && d >= 0 && (a + d) as usize + refused.len() == batch.acknowledged.len() => {}
        _ => return Err(Error::Upload("Upload response was incomplete; all observations remain queued".into())),
    }
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let tx = queue.db.transaction()?;
    for (source_id, seq, digest) in &batch.acknowledged {
        if let Some((_, _, reason)) = refused.iter().find(|(s, q, _)| s == source_id && q == seq) {
            tx.execute(
                "INSERT INTO rejections(source_id,seq,digest,reason,rejected_at) VALUES(?,?,?,?,?)
                 ON CONFLICT(source_id,seq) DO UPDATE SET digest=excluded.digest, reason=excluded.reason,
                 rejected_at=excluded.rejected_at",
                params![source_id, seq, digest, reason, stamp],
            )?;
        } else {
            tx.execute(
                "UPDATE observations SET uploaded_digest=? WHERE source_id=? AND seq=? AND digest=?",
                params![digest, source_id, seq, digest],
            )?;
        }
    }
    tx.commit()?;
    Ok(batch.acknowledged.len())
}

/// Upload until nothing is pending or a batch fails.
pub fn upload_all(queue: &mut Queue, transport: &dyn Transport, store: &dyn CredentialStore) -> Result<usize> {
    let mut total = 0;
    loop {
        let sent = upload_once(queue, transport, store)?;
        if sent == 0 {
            return Ok(total);
        }
        total += sent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::MemoryStore;
    use crate::save::tests::{sample, Fixture};
    use std::cell::RefCell;

    type Answer = Box<dyn FnMut(&str, &str) -> Result<Json>>;

    struct Fake {
        answer: RefCell<Answer>,
    }

    impl Fake {
        fn new(f: impl FnMut(&str, &str) -> Result<Json> + 'static) -> Fake {
            Fake { answer: RefCell::new(Box::new(f)) }
        }
    }

    impl Transport for Fake {
        fn post(&self, path: &str, body: &str, _credential: Option<&str>) -> Result<Json> {
            (self.answer.borrow_mut())(path, body)
        }
    }

    fn paired() -> MemoryStore {
        let store = MemoryStore::default();
        store.save(&Credential { device_id: "d".into(), token: format!("rrc_{}", "a".repeat(43)) }).unwrap();
        store
    }

    #[test]
    fn upload_acks_only_after_complete_response() {
        let mut f = Fixture::new();
        f.write(&sample(1, "First"));
        f.scan().unwrap();
        let batch = prepare_batch(&f.queue, 50).unwrap();
        assert_eq!(batch.acknowledged.len(), 1);
        assert!(batch.body().starts_with("{\"version\":1,"));
        let store = paired();
        let timeout = Fake::new(|_, _| Err(Error::Upload("timed out".into())));
        assert!(upload_once(&mut f.queue, &timeout, &store).is_err());
        assert_eq!(f.queue.status().unwrap().pending, 1);
        let partial = Fake::new(|_, _| Ok(serde_json::json!({"accepted": 0, "duplicate": 0})));
        assert!(upload_once(&mut f.queue, &partial, &store).is_err());
        assert_eq!(f.queue.status().unwrap().pending, 1);
        let ok = Fake::new(|_, _| Ok(serde_json::json!({"accepted": 1, "duplicate": 0})));
        assert_eq!(upload_once(&mut f.queue, &ok, &store).unwrap(), 1);
        assert_eq!(f.queue.status().unwrap().pending, 0);
    }

    #[test]
    fn refused_records_are_kept_but_not_resent() {
        let mut f = Fixture::new();
        let second = "\n  { [\"seq\"] = 2, [\"kind\"] = \"gossip\", [\"context\"] = { [\"product\"] = \"wow_classic_beta\" }, [\"data\"] = {} },\n },\n}\n";
        f.write(&sample(1, "First").replace("\n },\n}\n", second));
        f.scan().unwrap();
        assert_eq!(f.queue.status().unwrap().pending, 2);
        let store = paired();
        let sources: Vec<String> =
            prepare_batch(&f.queue, 50).unwrap().acknowledged.iter().map(|a| a.0.clone()).collect();
        let source = sources[0].clone();
        let answer = serde_json::json!({"accepted": 1, "duplicate": 0,
            "rejected": [{"sourceId": source, "seq": 2, "reason": "Observation is not a valid Forever record."}]});
        let ok = Fake::new(move |_, _| Ok(answer.clone()));
        assert_eq!(upload_once(&mut f.queue, &ok, &store).unwrap(), 2);
        let status = f.queue.status().unwrap();
        assert_eq!((status.pending, status.rejected), (0, 1));
        assert!(prepare_batch(&f.queue, 50).unwrap().acknowledged.is_empty());
        let rows = f.queue.recent(10).unwrap();
        assert_eq!(
            rows.iter().find(|r| r.seq == 2).unwrap().rejected.as_deref(),
            Some("Observation is not a valid Forever record.")
        );

        // A response naming a record we did not send is not trusted.
        f.queue.forget().unwrap();
        f.scan().unwrap();
        let wrong = Fake::new(|_, _| {
            Ok(
                serde_json::json!({"accepted": 1, "duplicate": 0, "rejected": [{"sourceId": "x", "seq": 9, "reason": "?"}]}),
            )
        });
        assert!(upload_once(&mut f.queue, &wrong, &store).is_err());
        assert_eq!(f.queue.status().unwrap().pending, 2);
    }

    #[test]
    fn upload_needs_a_paired_pc() {
        let mut f = Fixture::new();
        let never = Fake::new(|_, _| panic!("nothing should be sent"));
        assert!(upload_once(&mut f.queue, &never, &MemoryStore::default()).is_err());
    }

    #[test]
    fn prose_is_removed_from_transport_only() {
        let record = serde_json::json!({
            "seq": 1, "kind": "gossip",
            "data": {"title": "t", "dialogue": "Full NPC dialogue stays local",
                     "options": [{"id": 4, "name": "A dialogue choice"}], "long": "x".repeat(501)},
        });
        let sent = redact(&record);
        assert!(sent["data"].get("dialogue").is_none());
        assert!(sent["data"]["options"][0].get("name").is_none());
        assert_eq!(sent["data"]["options"][0]["id"], 4);
        assert!(sent["data"].get("long").is_none());
        assert_eq!(sent["data"]["title"], "t");
    }

    #[test]
    fn redeem_stores_credential_and_rejects_bad_codes() {
        let store = MemoryStore::default();
        let never = Fake::new(|_, _| panic!("a bad code must not be sent"));
        assert!(redeem_code(&never, &store, "short", "PC").is_err());
        let token = format!("rrc_{}", "b".repeat(43));
        let answer = serde_json::json!({"token": token, "deviceId": "dev-1"});
        let ok = Fake::new(move |path, body| {
            assert_eq!(path, "/api/collector/redeem");
            assert!(body.contains("\"code\":\"ABCDEFGHIJKLMNOP\""));
            Ok(answer.clone())
        });
        assert_eq!(redeem_code(&ok, &store, " ABCDEFGHIJKLMNOP ", "PC").unwrap(), "dev-1");
        assert_eq!(store.load().unwrap().unwrap().token, token);
    }

    /// Serve one canned HTTP answer on a local port and return what was received.
    fn serve_once(head: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let answer = format!("HTTP/1.1 {head}\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            std::io::Read::read_exact(&mut reader, &mut body).unwrap();
            (&stream).write_all(answer.as_bytes()).unwrap();
            head + &String::from_utf8(body).unwrap()
        });
        (base, handle)
    }

    #[test]
    fn https_sends_credential_and_reads_answer() {
        let (base, server) = serve_once("200 OK\r\nContent-Type: application/json", "{\"accepted\":1,\"duplicate\":0}");
        let token = format!("rrc_{}", "c".repeat(43));
        let answer = Https::new(&base).post("/api/collector/batches", "{}", Some(&token)).unwrap();
        assert_eq!(answer["accepted"], 1);
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /api/collector/batches "));
        assert!(request.contains(&format!("Bearer {token}")));
    }

    #[test]
    fn https_refuses_redirects_and_reports_server_errors() {
        let (base, server) = serve_once("302 Found\r\nLocation: http://example.invalid/steal", "");
        let token = format!("rrc_{}", "c".repeat(43));
        let error = Https::new(&base).post("/api/collector/batches", "{}", Some(&token)).unwrap_err();
        assert!(error.to_string().contains("changed the upload destination"), "{error}");
        server.join().unwrap();

        let (base, server) =
            serve_once("403 Forbidden\r\nContent-Type: application/json", "{\"error\":\"Device was revoked\"}");
        let error = Https::new(&base).post("/api/collector/batches", "{}", Some(&token)).unwrap_err();
        assert_eq!(error.to_string(), "Device was revoked");
        server.join().unwrap();

        assert!(Https::new(&base).post("/api/account", "{}", None).is_err());
        assert!(Https::new(&base).post("/api/collector/batches", "{}", Some("stolen")).is_err());
    }
}
