#!/usr/bin/env python3
"""Drive this checkout's own `rainworld-rs --mcp` from the shell (for agents in worktrees).

The session's `rainworld-rs` MCP tools talk to one server started from the main checkout at
session start: it runs main's code of that moment and is shared by every agent. This script
builds and runs the server of the checkout it lives in, held open in the background, so
calls see this branch's code and no other agent's state.

  scripts/mcp.py serve [server args]   build (release) and start; default --save-dir target/mcp-saves
  scripts/mcp.py call <tool> ['<json args>']
  scripts/mcp.py batch '<json list of {"tool": ..., "args": {...}}>'   (or '-' for stdin)
  scripts/mcp.py tools                 tool names and the first line of each description
  scripts/mcp.py stop | status
  --max N (before call/batch): cut each text reply to N characters (load_room and step return
  the full state; prefer get_state with "fields")

Text replies print as they are; screenshots are written to target/mcp-shots/ and their path
printed. A tool error prints `ERROR ...` and exits 1. Server log: target/mcp-server.log.
"""
import base64
import hashlib
import json
import os
import socket
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TARGET = os.path.join(ROOT, "target")
BINARY = os.path.join(TARGET, "release", "rainworld-rs")
SHOTS = os.path.join(TARGET, "mcp-shots")
LOG = os.path.join(TARGET, "mcp-server.log")
RUN_DIR = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
TAG = hashlib.sha1(ROOT.encode()).hexdigest()[:12]
SOCK = os.path.join(RUN_DIR, f"rw-mcp-{TAG}.sock")
PIDFILE = os.path.join(RUN_DIR, f"rw-mcp-{TAG}.pid")


class Server:
    """The `--mcp` child process, spoken to in newline-delimited JSON-RPC."""

    def __init__(self, args):
        log = open(LOG, "ab")
        self.proc = subprocess.Popen([BINARY, "--mcp", *args], cwd=ROOT, stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=log)
        self.next_id = 0
        self.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                    "clientInfo": {"name": "scripts/mcp.py", "version": "1"}})
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, msg):
        self.proc.stdin.write((json.dumps(msg) + "\n").encode())
        self.proc.stdin.flush()

    def request(self, method, params):
        self.next_id += 1
        self.send({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError(f"server exited (see {LOG})")
            msg = json.loads(line)
            if msg.get("id") == self.next_id:
                return msg


class Daemon:
    """Holds the server and serves one JSON request per socket connection."""

    def __init__(self, args):
        self.server = Server(args)

    def handle(self, req):
        if req.get("op") == "stop":
            return {"stopped": True}
        if req.get("op") == "tools":
            return self.server.request("tools/list", {})
        return self.server.request("tools/call", {"name": req["tool"], "arguments": req.get("args") or {}})

    def run(self):
        if os.path.exists(SOCK):
            os.unlink(SOCK)
        listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        listener.bind(SOCK)
        listener.listen(4)
        with open(PIDFILE, "w") as fh:
            fh.write(str(os.getpid()))
        try:
            while True:
                conn, _ = listener.accept()
                with conn:
                    req = json.loads(Client.read_all(conn))
                    try:
                        reply = self.handle(req)
                    except Exception as e:  # report to the caller, keep serving
                        reply = {"error": {"message": str(e)}}
                    conn.sendall(json.dumps(reply).encode())
                    if req.get("op") == "stop" or self.server.proc.poll() is not None:
                        break
        finally:
            self.server.proc.terminate()
            listener.close()
            for p in (SOCK, PIDFILE):
                if os.path.exists(p):
                    os.unlink(p)


class Client:
    max_chars = None

    @staticmethod
    def read_all(conn):
        chunks = []
        while True:
            data = conn.recv(1 << 16)
            if not data:
                return b"".join(chunks)
            chunks.append(data)

    @staticmethod
    def ask(req):
        if not os.path.exists(SOCK):
            sys.exit("no server for this checkout: run scripts/mcp.py serve")
        conn = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        conn.connect(SOCK)
        conn.sendall(json.dumps(req).encode())
        conn.shutdown(socket.SHUT_WR)
        reply = json.loads(Client.read_all(conn))
        conn.close()
        return reply

    @staticmethod
    def show(tool, reply):
        """Prints a tools/call reply; returns False on an error."""
        if "error" in reply:
            print(f"ERROR {tool}: {reply['error'].get('message')}")
            return False
        result = reply["result"]
        for block in result.get("content", []):
            if block.get("type") == "image":
                os.makedirs(SHOTS, exist_ok=True)
                path = os.path.join(SHOTS, f"{time.strftime('%H%M%S')}-{time.time_ns() % 10**6}.png")
                with open(path, "wb") as fh:
                    fh.write(base64.b64decode(block["data"]))
                print(path)
            else:
                text = block.get("text", "")
                if Client.max_chars is not None and len(text) > Client.max_chars:
                    text = text[:Client.max_chars] + f"... ({len(text)} chars)"
                print(("ERROR " + tool + ": " + text) if result.get("isError") else text)
        return not result.get("isError")


class Cli:
    @staticmethod
    def serve(args):
        if os.path.exists(SOCK):
            sys.exit(f"already serving ({SOCK}); scripts/mcp.py stop first")
        if "--save-dir" not in args:
            args = ["--save-dir", os.path.join(TARGET, "mcp-saves"), *args]
        build = subprocess.run(["cargo", "build", "--release", "-q", "-p", "rainworld-rs"], cwd=ROOT)
        if build.returncode != 0:
            sys.exit("build failed")
        subprocess.Popen([sys.executable, os.path.abspath(__file__), "_daemon", *args], cwd=ROOT,
                         start_new_session=True, stdin=subprocess.DEVNULL,
                         stdout=open(LOG, "ab"), stderr=subprocess.STDOUT)
        for _ in range(300):
            if os.path.exists(SOCK):
                print(f"serving {ROOT} on {SOCK}")
                return
            time.sleep(0.1)
        sys.exit(f"server did not start (see {LOG})")

    @staticmethod
    def batch(items):
        ok = True
        for item in items:
            print(f"== {item['tool']}")
            ok = Client.show(item["tool"], Client.ask(item)) and ok
            if not ok:
                break
        return ok

    @staticmethod
    def main(argv):
        if not argv:
            sys.exit(__doc__)
        if argv[0] == "--max":
            Client.max_chars, argv = int(argv[1]), argv[2:]
        cmd, rest = argv[0], argv[1:]
        if cmd == "_daemon":
            Daemon(rest).run()
        elif cmd == "serve":
            Cli.serve(rest)
        elif cmd == "call":
            args = json.loads(rest[1]) if len(rest) > 1 else {}
            sys.exit(0 if Client.show(rest[0], Client.ask({"tool": rest[0], "args": args})) else 1)
        elif cmd == "batch":
            text = sys.stdin.read() if rest[0] == "-" else rest[0]
            sys.exit(0 if Cli.batch(json.loads(text)) else 1)
        elif cmd == "tools":
            for t in Client.ask({"op": "tools"})["result"]["tools"]:
                print(f"{t['name']}: {t.get('description', '').splitlines()[0][:100]}")
        elif cmd == "stop":
            print("stopped" if Client.ask({"op": "stop"}).get("stopped") else "not stopped")
        elif cmd == "status":
            print(f"serving {ROOT} on {SOCK}" if os.path.exists(SOCK) else "not serving")
        else:
            sys.exit(__doc__)


if __name__ == "__main__":
    Cli.main(sys.argv[1:])
