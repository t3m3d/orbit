"""Exercise Orbit's real private bridge and BitTorrent protocol using only localhost.

Run after `cargo build --manifest-path native/Cargo.toml --bin orbit-engine`:
    python3 -m unittest discover -s tests/macos -v
"""
import hashlib
import http.server
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
ENGINE = Path(os.environ.get("ORBIT_TEST_ENGINE", ROOT / "native/target/debug/orbit-engine"))


def bencode(value):
    if isinstance(value, int):
        return b"i" + str(value).encode() + b"e"
    if isinstance(value, bytes):
        return str(len(value)).encode() + b":" + value
    if isinstance(value, list):
        return b"l" + b"".join(map(bencode, value)) + b"e"
    return b"d" + b"".join(bencode(k) + bencode(v) for k, v in sorted(value.items())) + b"e"


def exact(sock, count):
    result = b""
    while len(result) < count:
        chunk = sock.recv(count - len(result))
        if not chunk:
            raise EOFError
        result += chunk
    return result


class BridgeTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="orbit-bridge-test-")
        self.root = Path(self.temporary.name)
        self.env = dict(os.environ, ORBIT_DATA_DIR=str(self.root / "data"))
        self.process = None
        self.start()

    def tearDown(self):
        self.stop()
        self.temporary.cleanup()

    def start(self):
        self.process = subprocess.Popen([str(ENGINE), str(self.root / "state.json")], env=self.env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.wait(lambda s: s["status"].startswith("Offline"))

    def stop(self):
        if self.process is not None:
            if self.process.poll() is None:
                self.send("shutdown")
            out, err = self.process.communicate(timeout=10)
            self.assertEqual(self.process.returncode, 0, out + err)
            self.process = None

    def send(self, action, **values):
        self.process.stdin.write(json.dumps(dict(action=action, **values)) + "\n")
        self.process.stdin.flush()

    def wait(self, predicate, timeout=15):
        deadline = time.monotonic() + timeout
        state = None
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                self.fail("Core exited: " + self.process.stderr.read())
            try:
                state = json.loads((self.root / "state.json").read_text())
                if predicate(state):
                    return state
            except (FileNotFoundError, json.JSONDecodeError):
                pass
            time.sleep(0.04)
        self.fail(f"Timed out; last state: {state}")

    def configure(self, mode="offline", remember=False):
        self.send("configure", mode=mode, destination=str(self.root / "downloads"), dht=False,
                  remember=remember, down_kib=0, up_kib=0)
        return self.wait(lambda s: s["settings"]["network_mode"] == mode
                         and s["settings"]["network_setup_completed"]
                         and s["settings"]["startup_network_policy"] == ("remember_choice" if remember else "ask_every_start")
                         and (s["status"].startswith("Offline") if mode == "offline" else s["status"].startswith("Download Only")))

    def test_offline_validation_persistence_and_eof_shutdown(self):
        state = self.wait(lambda s: s["status"].startswith("Offline"))
        self.assertEqual(state["protocol"], 1)
        self.assertEqual(state["torrents"], [])
        self.assertFalse((self.root / "data/session").exists())
        self.send("preview", source="magnet:?xt=urn:btih:abc")
        self.wait(lambda s: s["error"] and "Choose Download Only" in s["error"])
        self.send("configure", mode="offline", destination="relative", dht=False, remember=False, down_kib=0, up_kib=0)
        self.wait(lambda s: s["error"] and "absolute path" in s["error"])
        self.configure(remember=True)
        saved = json.loads((self.root / "data/settings.json").read_text())
        self.assertEqual(saved["network_mode"], "offline")
        self.assertFalse(saved["dht_enabled"])
        self.assertFalse(saved["close_to_tray"])
        self.process.stdin.close()
        self.process.stdin = None
        self.process.wait(timeout=8)
        self.assertEqual(self.process.returncode, 0)

    def test_second_core_cannot_share_resume_database(self):
        result = subprocess.run([str(ENGINE), str(self.root / "other.json")], env=self.env,
                                input="", capture_output=True, text=True, timeout=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already running", result.stderr)

    def test_local_transfer_pause_recovery_and_keep_data(self):
        payload = b"Orbit localhost integration fixture.\n" * 900
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen()
        listener.settimeout(1)
        peer_port = listener.getsockname()[1]
        finished = threading.Event()

        def serve_peer():
            while not finished.is_set():
                try:
                    connection, _ = listener.accept()
                except socket.timeout:
                    continue
                except OSError:
                    return
                with connection:
                    connection.settimeout(5)
                    try:
                        handshake = exact(connection, 68)
                        connection.sendall(handshake[:48] + b"-ORTEST-123456789012")
                        connection.sendall(struct.pack(">I", 2) + b"\x05\x80")
                        connection.sendall(struct.pack(">I", 1) + b"\x01")
                        while True:
                            length = struct.unpack(">I", exact(connection, 4))[0]
                            if not length:
                                continue
                            message = exact(connection, length)
                            if message[0] == 6:
                                piece, offset, count = struct.unpack(">III", message[1:])
                                block = b"\x07" + struct.pack(">II", piece, offset) + payload[offset:offset + count]
                                connection.sendall(struct.pack(">I", len(block)) + block)
                    except (EOFError, OSError):
                        pass

        peer = threading.Thread(target=serve_peer, daemon=True)
        peer.start()

        class Tracker(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                body = bencode({b"complete": 1, b"incomplete": 0, b"interval": 1, b"peers": socket.inet_aton("127.0.0.1") + struct.pack(">H", peer_port)})
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        tracker = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Tracker)
        threading.Thread(target=tracker.serve_forever, daemon=True).start()
        try:
            source = self.root / "fixture.torrent"
            info = {b"length": len(payload), b"name": b"fixture.txt", b"piece length": 65536,
                    b"pieces": hashlib.sha1(payload).digest(), b"private": 1}
            source.write_bytes(bencode({b"announce": f"http://127.0.0.1:{tracker.server_port}/announce".encode(), b"info": info}))
            self.configure("download_only")
            self.send("preview", source="ftp://example.invalid/a.torrent")
            self.wait(lambda s: s["error"] and "HTTP(S)" in s["error"])
            self.send("preview", source=str(source))
            state = self.wait(lambda s: s["preview"] is not None)
            request = dict(state["preview"])
            self.assertEqual(request.pop("total"), len(payload))
            files = request.pop("files")
            request.update(selected_files=[f["index"] for f in files], start_paused=True)
            self.send("add", request=request)
            state = self.wait(lambda s: bool(s["torrents"]) and s["torrents"][0]["state"].lower() == "paused")
            row = state["torrents"][0]
            self.assertEqual(row["state"].lower(), "paused")
            self.send("resume", id=row["id"])
            state = self.wait(lambda s: bool(s["torrents"]) and s["torrents"][0]["finished"], timeout=30)
            row = state["torrents"][0]
            self.assertEqual(row["uploaded"], 0)
            output = Path(row["output_folder"]) / "fixture.txt"
            self.assertEqual(output.read_bytes(), payload)
            self.stop()
            self.start()  # Ask-every-launch returns Offline without restoring transfers.
            self.assertEqual(self.wait(lambda s: s["status"].startswith("Offline"))["torrents"], [])
            self.configure("download_only")
            state = self.wait(lambda s: bool(s["torrents"]) and s["torrents"][0]["finished"])
            self.send("forget", id=state["torrents"][0]["id"])
            self.wait(lambda s: s["torrents"] == [] and s["preview"] is None)
            self.assertEqual(output.read_bytes(), payload)
        finally:
            finished.set()
            listener.close()
            tracker.shutdown()
            tracker.server_close()
            peer.join(timeout=6)


if __name__ == "__main__":
    unittest.main()
