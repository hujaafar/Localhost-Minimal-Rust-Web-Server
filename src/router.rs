use crate::config::{RouteCfg, ServerCfg};
use crate::http::{Method, Response};
use crate::util::guess_mime;
use std::{fs, path::{Path, PathBuf}};

pub struct Route<'a> { pub cfg: &'a RouteCfg }
pub struct MatchedRoute<'a> { pub route: &'a RouteCfg, pub local_path: PathBuf, pub is_dir: bool }

pub fn match_route<'a>(servers: &'a ServerCfg, path: &str) -> Option<MatchedRoute<'a>> {
    // Longest-prefix match
    let mut best: Option<&RouteCfg> = None;
    for r in &servers.routes {
        if path == r.path || path.starts_with(&(r.path.to_string() + "/")) {
            best = match best { None => Some(r), Some(prev) if r.path.len() > prev.path.len() => Some(r), _ => best };
        }
    }
    let route = best?;
    let rest = path.strip_prefix(&route.path).unwrap_or("");
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    let root = Path::new(&route.root);
    let local = root.join(rest);
    let is_dir = local.is_dir();
    Some(MatchedRoute { route, local_path: local, is_dir })
}

pub fn dir_listing_html(dir: &Path, url_path: &str) -> Response {
    let entries = match fs::read_dir(dir) { Ok(r) => r, Err(_) => return Response::new(403, "Forbidden") };
    let mut body = String::from("<html><body><h1>Index of ");
    body.push_str(url_path);
    body.push_str("</h1><ul>");
    for e in entries {
        if let Ok(e) = e {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let slash = if is_dir { "/" } else { "" };
            body.push_str(&format!("<li><a href=\"{0}{1}\">{0}{1}</a></li>", name, slash));
        }
    }
    body.push_str("</ul></body></html>");
    Response::new(200, "OK").set_header("Content-Type", "text/html; charset=utf-8").set_body(body.into_bytes())
}

pub fn static_file(path: &Path) -> Response {
    match fs::read(path) {
        Ok(bytes) => {
            let mime = guess_mime(&path.to_string_lossy());
            Response::new(200, "OK").set_header("Content-Type", mime).set_body(bytes)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Response::new(404, "Not Found"),
        Err(_) => Response::new(403, "Forbidden"),
    }
}

pub fn allow_method(route: &RouteCfg, m: &Method) -> bool {
    if route.methods.is_empty() { return true; }
    let s = match m { Method::GET => "GET", Method::POST => "POST", Method::DELETE => "DELETE" };
    route.methods.iter().any(|x| x == s)
}

pub fn resolve_index(route: &RouteCfg, local: &Path) -> Option<PathBuf> {
    for idx in &route.index { let p = local.join(idx); if p.is_file() { return Some(p); } }
    None
}
