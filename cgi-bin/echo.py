import os, sys

data = sys.stdin.buffer.read()
print("Content-Type: text/plain; charset=utf-8")
print()
print("CGI OK")
print("PATH_INFO:", os.environ.get("PATH_INFO", ""))
print("BYTES:", len(data))
try:
    print("BODY:", data.decode("utf-8", "ignore"))
except Exception:
    print("BODY: <non-utf8>")
