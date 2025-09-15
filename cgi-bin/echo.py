#!/usr/bin/env python3
import os, sys

body = sys.stdin.buffer.read()
print("Status: 200 OK\r")
print("Content-Type: text/plain\r")
print("\r")
print("CGI OK")
print("PATH_INFO=", os.environ.get("PATH_INFO"))
print("METHOD=", os.environ.get("REQUEST_METHOD"))
print("LEN=", os.environ.get("CONTENT_LENGTH"))
print("BODY=", body.decode(errors='ignore'))
