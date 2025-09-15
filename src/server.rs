use crate::{
    cgi,
    config::{Config, ServerCfg},
    ep,
    http::{Method, ParseProgress, Parser, Request, Response},
    router,
    session::SessionStore,
    upload,
};
use anyhow::Context;
use libc::*;
use std::{
    collections::HashMap,
    io,
    net::Ipv4Addr,
    os::fd::RawFd,
    path::Path,
};

struct Conn {
    parser: Parser,
    outbuf: Vec<u8>,
    last_active: u128,
}

pub struct HttpServer {
    cfg: Config,
    ep: ep::Epoll,
    listeners: Vec<RawFd>,
    conns: HashMap<RawFd, Conn>,
    servers_by_port: HashMap<u16, ServerCfg>,
    sessions: SessionStore,
    max_events: i32,
    timeout_ms: u64,
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        for &fd in &self.listeners { unsafe { close(fd) }; }
    }
}

impl HttpServer {
    pub fn new(cfg: Config) -> anyhow::Result<Self> {
        Ok(Self {
            max_events: cfg.epoll_max_events,
            timeout_ms: cfg.request_timeout_ms,
            sessions: SessionStore::new_ttl(3600),
            servers_by_port: HashMap::new(),
            listeners: Vec::new(),
            conns: HashMap::new(),
            ep: ep::Epoll::new()?,
            cfg,
        })
    }

    fn bind_listeners(&mut self) -> anyhow::Result<()> {
        for s in &self.cfg.servers {
            for &port in &s.ports {
                let fd = unsafe { socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0) };
                if fd < 0 { return Err(io::Error::last_os_error()).context("socket"); }

                let yes: i32 = 1;
                unsafe {
                    let optlen: libc::socklen_t = std::mem::size_of::<i32>() as libc::socklen_t;
                    let rc = setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes as *const _ as *const _, optlen);
                    if rc < 0 { return Err(io::Error::last_os_error()).context("setsockopt(SO_REUSEADDR)"); }
                }

                let addr = sockaddr_in {
                    sin_family: AF_INET as u16,
                    sin_port: port.to_be(),
                    sin_addr: in_addr { s_addr: u32::from(Ipv4Addr::UNSPECIFIED).to_be() },
                    sin_zero: [0; 8],
                };

                let namelen: libc::socklen_t = std::mem::size_of::<sockaddr_in>() as libc::socklen_t;
                let r = unsafe { bind(fd, &addr as *const _ as *const sockaddr, namelen) };
                if r < 0 { return Err(io::Error::last_os_error()).context("bind"); }

                let r = unsafe { listen(fd, 512) };
                if r < 0 { return Err(io::Error::last_os_error()).context("listen"); }

                self.ep.add(fd, ep::READ_FLAGS, fd as u64)?;
                self.listeners.push(fd);
                self.servers_by_port.insert(port, s.clone());
            }
        }
        Ok(())
    }

    fn accept_loop(&mut self, lfd: RawFd) -> anyhow::Result<()> {
        loop {
            let mut addr: sockaddr_in = unsafe { std::mem::zeroed() };
            let mut len: libc::socklen_t = std::mem::size_of::<sockaddr_in>() as libc::socklen_t;

            let cfd = unsafe {
                accept4(lfd, &mut addr as *mut _ as *mut sockaddr, &mut len as *mut libc::socklen_t, SOCK_NONBLOCK | SOCK_CLOEXEC)
            };

            if cfd < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::WouldBlock { break; }
                return Err(e).context("accept4");
            }

            self.ep.add(cfd, ep::READ_FLAGS, cfd as u64)?;
            self.conns.insert(cfd, Conn { parser: Parser::new(), outbuf: Vec::new(), last_active: crate::util::now_ms() });
        }
        Ok(())
    }

    fn close_conn(&mut self, fd: RawFd) {
        let _ = self.ep.del(fd);
        self.conns.remove(&fd);
        unsafe { close(fd) };
    }

// src/server.rs

fn read_once(&mut self, fd: RawFd) -> anyhow::Result<bool> {
    use std::convert::TryInto;
    let mut any = false;
    let mut buf = [0u8; 8192];
    loop {
        let n = unsafe { read(fd, buf.as_mut_ptr() as *mut _, buf.len().try_into().unwrap()) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::WouldBlock {
                break; // drained this readiness
            }
            return Err(e).context("read");
        }
        if n == 0 {
            // peer closed
            return Ok(false);
        }
        any = true;
        if let Some(c) = self.conns.get_mut(&fd) {
            c.parser.push(&buf[..n as usize]);
            c.last_active = crate::util::now_ms();
        }
    }
    Ok(any)
}



    // one non-blocking write attempt per readiness
    fn write_once(&mut self, fd: RawFd) -> anyhow::Result<bool> {
        if let Some(conn) = self.conns.get_mut(&fd) {
            if conn.outbuf.is_empty() { return Ok(true); }
            let n = unsafe { write(fd, conn.outbuf.as_ptr() as *const _, conn.outbuf.len()) };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::WouldBlock { return Ok(false); }
                return Err(e).context("write");
            }
            if n == 0 { return Ok(false); }
            conn.outbuf.drain(..n as usize);
        }
        Ok(true)
    }

    fn handle_request(&mut self, _fd: RawFd, req: Request, port: u16) -> Response {
        let scfg = match self.servers_by_port.get(&port) { Some(s) => s, None => return Response::new(500, "Internal Server Error") };
        if scfg.client_max_body_size > 0 && req.body.len() > scfg.client_max_body_size {
            return self.error_from_cfg(scfg, 413);
        }

        let mr = match router::match_route(scfg, &req.path) { Some(m) => m, None => return self.error_from_cfg(scfg, 404) };
        if !router::allow_method(mr.route, &req.method) { return self.error_from_cfg(scfg, 405); }
        if let Some(redir) = &mr.route.redirect { return Response::new(redir.status, "").set_header("Location", &redir.to); }

        // Sessions
        let cookie_header = req.header("cookie").unwrap_or("");
        let mut sid_in: Option<&str> = None;
        for kv in cookie_header.split(';') {
            let kv = kv.trim();
            if let Some(v) = kv.strip_prefix("SESSIONID=") { sid_in = Some(v); break; }
        }
        let (sid, _session) = self.sessions.get_or_create(sid_in);
        self.sessions.touch(&sid);

        // CGI?
        if let Some(ref cgi_cfg) = mr.route.cgi {
            if let Some(ext) = Path::new(&mr.local_path).extension().and_then(|s| s.to_str()) {
                if format!(".{}", ext) == cgi_cfg.ext {
                    let rel = mr.local_path.strip_prefix(&mr.route.root).unwrap_or(&mr.local_path);
                    let rel = rel.to_string_lossy().trim_start_matches('/').to_string();
                    let mut resp = cgi::run_cgi(cgi_cfg, &mr.route.root, &req, &rel);
                    resp.headers.insert("Set-Cookie".into(), format!("SESSIONID={}; HttpOnly; Path=/", sid));
                    resp.headers.entry("Date".into()).or_insert(crate::util::http_date(std::time::SystemTime::now()));
                    return resp;
                }
            }
        }

        match req.method {
            Method::GET => {
                if mr.is_dir {
                    if let Some(idx) = router::resolve_index(mr.route, &mr.local_path) { return ok_file_with_cookie(idx, sid); }
                    if mr.route.dir_listing { return dir_with_cookie(&mr.local_path, &req.path, sid); }
                    return self.error_from_cfg(scfg, 403);
                }
                ok_file_with_cookie(&mr.local_path, sid)
            }
            Method::DELETE => {
                if mr.local_path.is_file() {
                    let _ = std::fs::remove_file(&mr.local_path);
                    Response::new(200, "OK").set_header("Set-Cookie", &format!("SESSIONID={}; Path=/; HttpOnly", sid))
                } else { self.error_from_cfg(scfg, 404) }
            }
   Method::POST => {
    if mr.route.upload_enabled {
        let ct = req.header("content-type").unwrap_or("");
        if let Some(bpos) = ct.find("boundary=") {
            // --- PATCHED BOUNDARY EXTRACTION ---
            let mut boundary = &ct[bpos + 9..];
            if let Some(semi) = boundary.find(';') {
                boundary = &boundary[..semi];
            }
            boundary = boundary.trim().trim_matches('"');
            // -----------------------------------

            match upload::parse_multipart(&req.body, boundary) {
                Ok(files) if !files.is_empty() => {
                    std::fs::create_dir_all(&mr.local_path).ok();
                    for f in files {
                        let p = mr.local_path.join(sanitize(&f.filename));
                        let _ = std::fs::write(p, f.content);
                    }
                    return Response::new(201, "Created")
                        .set_header(
                            "Set-Cookie",
                            &format!("SESSIONID={}; Path=/; HttpOnly", sid),
                        )
                        .set_body(b"uploaded".to_vec());
                }
                _ => return Response::new(400, "Bad Request"),
            }
        } else {
            return Response::new(400, "Bad Request");
        }
    }
    Response::new(200, "OK")
        .set_header("Content-Type", "application/json")
        .set_header("Set-Cookie", &format!("SESSIONID={}; Path=/; HttpOnly", sid))
        .set_body(req.body)
}

        }
    }

fn error_from_cfg(&self, scfg: &ServerCfg, code: u16) -> Response {
    use std::fs;
    if let Some(p) = scfg.error_pages.get(&code.to_string()) {
        if let Ok(bytes) = fs::read(p) {
            return Response::new(code, "")
                .set_header("Content-Type", "text/html; charset=utf-8")
                .set_body(bytes);
        }
    }
    Response::new(code, "")
}

    pub fn run(mut self) -> anyhow::Result<()> {
        self.bind_listeners()?;
        let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; self.max_events as usize];
        let timeout = self.timeout_ms as isize;

        loop {
            self.sessions_gc_and_timeouts();

            let n = self.ep.wait(&mut events, timeout)?;
            for i in 0..n {
                let ev = events[i];
                let fd = ev.u64 as RawFd;
                let ev_in  = (ev.events & EPOLLIN  as u32) != 0;
                let ev_out = (ev.events & EPOLLOUT as u32) != 0;
                let ev_err = (ev.events & (EPOLLERR | EPOLLHUP) as u32) != 0;

                if self.listeners.contains(&fd) {
                    let _ = self.accept_loop(fd);
                    let _ = self.ep.modf(fd, ep::READ_FLAGS, fd as u64);
                    continue;
                }

                if ev_err { self.close_conn(fd); continue; }

                if ev_in {
                    match self.read_once(fd) {
                        Ok(true) => {},
                        Ok(false) => { self.close_conn(fd); continue; }
                        Err(_) => { self.close_conn(fd); continue; }
                    }

                   

                    // parse at most one request per wake-up
                    let parsed = if let Some(conn) = self.conns.get_mut(&fd) {
                        match conn.parser.try_parse() {
                            Ok(ParseProgress::NeedMore) => None,
                            Ok(ParseProgress::Done(req)) => Some(Ok(req)),
                            Err(e) => Some(Err(e)),
                        }
                    } else { None };

                    if let Some(result) = parsed {
                        match result {
                            Ok(req) => {
                                let port = local_port(fd).unwrap_or(0);
                                let resp = self.handle_request(fd, req, port);
                                if let Some(conn) = self.conns.get_mut(&fd) {
                                    conn.outbuf.extend_from_slice(&resp.to_bytes());
                                }
                                // --- immediate flush to avoid "Empty reply from server"
                                let _ = self.write_once(fd);
                            }
                            Err(_) => {
                                if let Some(conn) = self.conns.get_mut(&fd) {
                                    conn.outbuf.extend_from_slice(
                                        Response::new(400, "Bad Request").to_bytes().as_slice(),
                                    );
                                }
                                // flush error immediately too
                                let _ = self.write_once(fd);
                            }
                        }
                    }
                }

                if ev_out {
                    let _ = self.write_once(fd);
                }

                if let Some(conn) = self.conns.get(&fd) {
                    let want_write = !conn.outbuf.is_empty();
                    let new_flags = if want_write { ep::READ_FLAGS | ep::WRITE_FLAGS } else { ep::READ_FLAGS };
                    let _ = self.ep.modf(fd, new_flags, fd as u64);
                }
            }
        }
    }

    fn sessions_gc_and_timeouts(&mut self) {
        self.sessions.gc();
        let now = crate::util::now_ms();
        let to_close: Vec<RawFd> = self.conns.iter()
            .filter(|(_, c)| now - c.last_active > self.timeout_ms as u128)
            .map(|(fd, _)| *fd).collect();
        for fd in to_close { self.close_conn(fd); }
    }
}

fn sanitize(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.').collect()
}

fn local_port(fd: RawFd) -> Option<u16> {
    unsafe {
        let mut addr: sockaddr_in = std::mem::zeroed();
        let mut len: libc::socklen_t = std::mem::size_of::<sockaddr_in>() as libc::socklen_t;
        if getsockname(fd, &mut addr as *mut _ as *mut sockaddr, &mut len as *mut libc::socklen_t) < 0 { return None; }
        Some(u16::from_be(addr.sin_port))
    }
}

// ---- helpers to attach cookie + date
use std::time::SystemTime;
fn ok_file_with_cookie<P: AsRef<std::path::Path>>(path: P, sid: String) -> Response {
    let mut r = router::static_file(path.as_ref());
    r.headers.insert("Set-Cookie".into(), format!("SESSIONID={}; Path=/; HttpOnly", sid));
    r.headers.entry("Date".into()).or_insert(crate::util::http_date(SystemTime::now()));
    r
}
fn dir_with_cookie(dir: &std::path::Path, url_path: &str, sid: String) -> Response {
    let mut r = router::dir_listing_html(dir, url_path);
    r.headers.insert("Set-Cookie".into(), format!("SESSIONID={}; Path=/; HttpOnly", sid));
    r.headers.entry("Date".into()).or_insert(crate::util::http_date(SystemTime::now()));
    r
}
