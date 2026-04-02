# 🦀 Localhost — Minimal Rust Web Server

A high-performance HTTP/1.1 web server written entirely in Rust.  
It leverages epoll for concurrency, supports CGI scripts, and provides multi-virtual-host configuration through simple `.conf` files.  
Built for learning, testing, and performance benchmarking — not just theory.

---

## 🌟 Highlights

- Fully HTTP/1.1 compliant (keep-alive & close)
- Epoll-driven event loop for scalable I/O
- Serves static content (HTML, CSS, JS, images)
- Executes CGI programs (Python, Bash, etc.)
- Supports both chunked and content-length POST bodies
- Virtual host support via configuration sections
- Built-in error handling (404, 405, 500...)
- Simple session cookies (`Set-Cookie: SID=...`)
- Fully configurable host, port, and root
- Proven stability under load (>99% uptime)

---

## ⚙️ Example Configuration

`server.conf`

```ini
[server "demo"]
host = 127.0.0.1
port = 8080
root = ./sites/demo
index = index.html

[server "broken"]
host = 127.0.0.1
port = 8080
root = ./sites/broken
index = index.html
