use crate::http::{Request, Response};
use crate::config::CgiCfg;
use std::{io::Write, path::Path, process::{Command, Stdio}};

pub fn run_cgi(cfg: &CgiCfg, route_root: &str, req: &Request, rel_path: &str) -> Response {
    let script_path = Path::new(route_root).join(rel_path);
    if !script_path.exists() { return Response::new(404, "Not Found"); }

    let mut cmd = Command::new(&cfg.runner);
    cmd.arg(&script_path);

    // env
    cmd.env("REQUEST_METHOD", match req.method { crate::http::Method::GET=>"GET", crate::http::Method::POST=>"POST", crate::http::Method::DELETE=>"DELETE"});
    cmd.env("PATH_INFO", script_path.to_string_lossy().to_string());
    if let Some(ct) = req.header("content-type") { cmd.env("CONTENT_TYPE", ct); }
    if let Some(cl) = req.header("content-length") { cmd.env("CONTENT_LENGTH", cl); }

    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() { Ok(c) => c, Err(_) => return Response::new(500, "Internal Server Error") };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&req.body);
    }
    let out = child.wait_with_output();
    if let Ok(o) = out {
        let stdout = o.stdout;
        let txt = String::from_utf8_lossy(&stdout);
        let (headers, body) = if let Some(pos) = txt.find("\r\n\r\n") {
            (&txt[..pos], &txt[pos+4..])
        } else { ("Content-Type: text/plain", &*txt) };
        let mut resp = Response::new(200, "OK");
        for line in headers.split("\r\n") {
            if let Some((k,v)) = line.split_once(':') { resp.headers.insert(k.trim().to_string(), v.trim().to_string()); }
        }
        resp.body = body.as_bytes().to_vec();
        return resp;
    }
    Response::new(500, "Internal Server Error")
}
