use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Method { GET, POST, DELETE }

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub chunked: bool,
}
impl Request {
    pub fn header(&self, k: &str) -> Option<&str> {
        self.headers.get(&k.to_ascii_lowercase()).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub code: u16,
    pub reason: &'static str,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}
impl Response {
    pub fn new(code: u16, reason: &'static str) -> Self {
        Self { code, reason, headers: HashMap::new(), body: Vec::new() }
    }
    pub fn set_header(mut self, k: &str, v: &str) -> Self { self.headers.insert(k.to_string(), v.to_string()); self }
    pub fn set_body(mut self, b: Vec<u8>) -> Self { self.body = b; self }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256 + self.body.len());
        out.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", self.code, self.reason).as_bytes());
        if !self.headers.contains_key("Content-Length") && !self.headers.contains_key("content-length") {
            out.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
        }
        for (k, v) in &self.headers { out.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes()); }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}

fn parse_start_line(s: &str) -> anyhow::Result<(Method, String, String)> {
    let mut it = s.split_whitespace();
    let method = match it.next().ok_or_else(|| anyhow::anyhow!("bad start line"))? {
        "GET" => Method::GET, "POST" => Method::POST, "DELETE" => Method::DELETE,
        _ => return Err(anyhow::anyhow!("unsupported method")),
    };
    let path = it.next().ok_or_else(|| anyhow::anyhow!("bad start line"))?.to_string();
    let ver = it.next().unwrap_or("HTTP/1.1").to_string();
    Ok((method, path, ver))
}

pub enum ParseProgress { NeedMore, Done(Request) }

pub struct Parser {
    buf: Vec<u8>,
    headers_done: bool,
    content_len: Option<usize>,
    chunked: bool,
    method: Option<Method>,
    path: String,
    version: String,
    headers: HashMap<String, String>,
}
impl Parser {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(), headers_done: false, content_len: None, chunked: false,
            method: None, path: String::new(), version: String::new(),
            headers: HashMap::new(),
        }
    }

    pub fn push(&mut self, data: &[u8]) { self.buf.extend_from_slice(data) }

    pub fn try_parse(&mut self) -> anyhow::Result<ParseProgress> {
        if !self.headers_done {
            if let Some(pos) = find_headers_end(&self.buf) {
                self.headers_done = true;
                let head = String::from_utf8(self.buf[..pos].to_vec())?;
                let mut lines = head.split("\r\n");
                let (method, path, version) = parse_start_line(lines.next().unwrap())?;
                self.method = Some(method); self.path = path; self.version = version;
                for line in lines {
                    if let Some((k,v)) = line.split_once(':') {
                        self.headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
                    }
                }
                if let Some(te) = self.headers.get("transfer-encoding") {
                    if te.to_ascii_lowercase().contains("chunked") { self.chunked = true; }
                }
                if let Some(cl) = self.headers.get("content-length") {
                    self.content_len = cl.parse::<usize>().ok();
                }
                self.buf.drain(..pos+4);
                if !self.chunked {
                    if let Some(n) = self.content_len {
                        if self.buf.len() >= n {
                            let body = self.buf[..n].to_vec(); self.buf.drain(..n);
                            return Ok(ParseProgress::Done(Request {
                                method: self.method.clone().unwrap(), path: self.path.clone(),
                                version: self.version.clone(), headers: std::mem::take(&mut self.headers),
                                body, chunked: false,
                            }));
                        } else { return Ok(ParseProgress::NeedMore); }
                    } else {
                        return Ok(ParseProgress::Done(Request {
                            method: self.method.clone().unwrap(), path: self.path.clone(),
                            version: self.version.clone(), headers: std::mem::take(&mut self.headers),
                            body: Vec::new(), chunked: false,
                        }));
                    }
                }
            } else { return Ok(ParseProgress::NeedMore); }
        }

        if self.chunked {
            let mut body = Vec::new();
            loop {
                match read_chunk(&self.buf) {
                    ChunkRead::NeedMore => return Ok(ParseProgress::NeedMore),
                    ChunkRead::Chunk { size, consumed } => {
                        let after_line = hex_line_len(&self.buf).unwrap_or(0);
                        body.extend_from_slice(&self.buf[after_line..after_line+size]);
                        self.buf.drain(..consumed);
                    }
                    ChunkRead::End { consumed } => {
                        self.buf.drain(..consumed);
                        return Ok(ParseProgress::Done(Request{
                            method: self.method.clone().unwrap(), path: self.path.clone(),
                            version: self.version.clone(), headers: std::mem::take(&mut self.headers),
                            body, chunked: true,
                        }));
                    }
                }
            }
        }
        Ok(ParseProgress::NeedMore)
    }

    // --- Expect: 100-continue helpers ---
    pub fn wants_100_continue(&self) -> bool {
        if !self.headers_done { return false; }
        if let Some(exp) = self.headers.get("expect") {
            if exp.to_ascii_lowercase().contains("100-continue") {
                if !self.chunked {
                    if let Some(n) = self.content_len { return self.buf.len() < n; }
                }
            }
        }
        false
    }
    pub fn clear_expect(&mut self) { self.headers.remove("expect"); }
}

fn hex_line_len(buf: &[u8]) -> anyhow::Result<usize> {
    if let Some(nl) = memchr::memchr(b'\n', buf) {
        if nl == 0 || buf[nl-1] != b'\r' { anyhow::bail!("bad chunk line"); }
        Ok(nl + 1)
    } else { anyhow::bail!("incomplete hex line"); }
}
fn find_headers_end(b: &[u8]) -> Option<usize> { memchr::memmem::find(b, b"\r\n\r\n") }

enum ChunkRead { NeedMore, Chunk { size: usize, consumed: usize }, End { consumed: usize } }

fn parse_hex(sz: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(sz).ok()?.trim();
    usize::from_str_radix(s, 16).ok()
}
fn read_chunk(buf: &[u8]) -> ChunkRead {
    if let Some(line_end) = memchr::memchr(b'\n', buf) {
        if line_end == 0 || buf[line_end-1] != b'\r' { return ChunkRead::NeedMore; }
        let size = match parse_hex(&buf[..line_end-1]) { Some(n) => n, None => return ChunkRead::NeedMore };
        let after_line = line_end + 1;
        if size == 0 {
            if buf.len() >= after_line + 2 && &buf[after_line..after_line+2] == b"\r\n" {
                return ChunkRead::End { consumed: after_line + 2 };
            } else { return ChunkRead::NeedMore; }
        }
        let need = after_line + size + 2;
        if buf.len() < need { return ChunkRead::NeedMore; }
        if &buf[after_line+size..after_line+size+2] != b"\r\n" { return ChunkRead::NeedMore; }
        ChunkRead::Chunk { size, consumed: need }
    } else { ChunkRead::NeedMore }
}
