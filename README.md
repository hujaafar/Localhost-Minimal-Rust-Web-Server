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


---

🧠 Notes
Multiple [server] blocks = virtual hosts
Duplicate ports and invalid configs are automatically rejected
🏃 Getting Started
1. Build
cargo build --release
2. Run
./target/release/localhost server.conf
3. Test
curl -i http://127.0.0.1:8080/
4. Test virtual host
curl -i --resolve demo.com:8080:127.0.0.1 http://demo.com:8080/
🧩 Running CGI Scripts
HTML Form
<form method="POST" action="/py/echo.py">
  <input type="text" name="name" />
  <input type="submit" />
</form>
curl (chunked request)
printf 'hello from chunks\n' | curl -v --http1.1 -X POST --data-binary @- http://127.0.0.1:8080/py/echo.py

✅ Works with both Content-Length and Transfer-Encoding: chunked

🧠 Test Summary
Scenario	Status	Description
Static / Dynamic Routes	✅	Working
CGI Execution	✅	Python OK
Multi-vhost Setup	✅	Stable
Duplicate Port Check	✅	Safe
Faulty Config Handling	✅	Isolated
Load Testing	✅	>99.5%
Memory Stability	✅	Stable
Socket Clean-up	✅	No leaks
📈 Siege Benchmark
Command
siege -b http://127.0.0.1:8080/
Output
{
  "transactions": 3502,
  "availability": 100.0,
  "elapsed_time": 8.78,
  "response_time": 0.06,
  "transaction_rate": 398.86,
  "successful_transactions": 3502,
  "failed_transactions": 0
}
🧪 Testing Tools
curl → HTTP testing
siege → Load testing
top / ps / ss → Monitoring

All results documented in audit.md.
