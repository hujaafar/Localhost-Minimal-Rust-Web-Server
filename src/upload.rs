use anyhow::Context;

pub struct FilePart { pub filename: String, pub content: Vec<u8> }

pub fn parse_multipart(body: &[u8], boundary: &str) -> anyhow::Result<Vec<FilePart>> {
    let b = format!("--{}", boundary);
    let end = format!("--{}--", boundary);
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        if let Some(pos) = memchr::memmem::find(&body[i..], b.as_bytes()) {
            i += pos + b.len();
            if body.get(i..i+2) == Some(b"\r\n") { i += 2; }
            // headers
            let start_headers = i;
            let Some(h_end_rel) = memchr::memmem::find(&body[i..], b"\r\n\r\n") else { break };
            let h_end = i + h_end_rel;
            let headers_raw = &body[start_headers..h_end];
            i = h_end + 4;
            let headers = String::from_utf8(headers_raw.to_vec()).context("header utf8")?;
            let mut filename = String::new();
            for line in headers.split("\r\n") {
                if let Some(v) = line.strip_prefix("Content-Disposition:") {
                    if let Some(fnpos) = v.find("filename=") {
                        let val = &v[fnpos+9..].trim();
                        let val = val.trim_matches(' ').trim_matches('"');
                        filename = val.to_string();
                    }
                }
            }
            // content until next boundary or end
            if let Some(next) = memchr::memmem::find(&body[i..], b.as_bytes()) {
                let content = &body[i..i+next-2]; // strip trailing CRLF
                out.push(FilePart{ filename, content: content.to_vec() });
                i = i + next;
            } else if let Some(endpos) = memchr::memmem::find(&body[i..], end.as_bytes()) {
                let content = &body[i..i+endpos-2];
                out.push(FilePart{ filename, content: content.to_vec() });
                break;
            } else { break; }
        } else { break; }
    }
    Ok(out)
}
