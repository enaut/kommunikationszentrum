#!/usr/bin/env python3
"""
Spacetime OIDC login (Authorization Code + PKCE only).

Minimal inputs via environment variables (e.g., .env.oidc):
    - OIDC_CLIENT_ID  (required)
    - OIDC_TIMEOUT    (seconds, optional, default: 180)

Fixed constants (not overridable):
    - AUTH URL:    http://127.0.0.1:8000/o/authorize/
    - TOKEN URL:   http://127.0.0.1:8000/o/token/
    - SCOPE:       "openid profile email offline_access"
    - REDIRECT URI: http://127.0.0.1:8765/callback

Flow:
    1) Start local HTTP server and wait for callback.
    2) Open browser to the authorization URL (PKCE).
    3) Exchange code for tokens.
    4) Run `spacetime login --token <id|access>`.

Only the authorization code flow is implemented.
"""
from __future__ import annotations

import base64
import hashlib
import http.server
import json
import os
import socket
import socketserver
import subprocess
import sys
import threading
import time
import urllib.parse
import urllib.request
import webbrowser
from dataclasses import dataclass
from typing import Optional

# Fixed constants (not read from env)
AUTH_URL = "http://127.0.0.1:8000/o/authorize/"
TOKEN_URL = "http://127.0.0.1:8000/o/token/"
SCOPE = "openid"
REDIRECT_URI = "http://127.0.0.1:8765/callback"


def env(name: str, default: Optional[str] = None) -> str:
    val = os.environ.get(name)
    if val is None:
        return "" if default is None else default
    return val


def b64url_no_pad(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode().rstrip("=")


def make_pkce_pair() -> tuple[str, str]:
    # RFC 7636: code_verifier length 43-128 chars; use a urlsafe random string
    # Use 64 bytes entropy -> ~86 chars after base64url (within limits)
    verifier = b64url_no_pad(os.urandom(64))
    challenge = b64url_no_pad(hashlib.sha256(verifier.encode()).digest())
    return verifier, challenge


@dataclass
class Config:
    client_id: str
    auth_url: str
    token_url: str
    scope: str
    redirect_uri: str
    timeout: int

    @staticmethod
    def from_env() -> "Config":
        client_id_val = env("OIDC_CLIENT_ID")
        if not client_id_val:
            print(
                "Missing OIDC_CLIENT_ID environment variable.",
                file=sys.stderr,
            )
            sys.exit(2)
        # Use sane constants, do not read these from env
        auth_url_val = AUTH_URL
        token_url_val = TOKEN_URL
        scope_val = SCOPE
        redirect_uri_val = REDIRECT_URI
        timeout_str = env("OIDC_TIMEOUT", "180") or "180"
        try:
            timeout_val = int(timeout_str)
        except ValueError:
            timeout_val = 180
        return Config(
            client_id=client_id_val,
            auth_url=auth_url_val,
            token_url=token_url_val,
            scope=scope_val,
            redirect_uri=redirect_uri_val,
            timeout=timeout_val,
        )


class CodeReceiver:
    def __init__(self, host: str, port: int, path: str, state: str):
        self.host = host
        self.port = port
        self.path = path
        self.state = state
        self.code: Optional[str] = None
        self.recv_state: Optional[str] = None
        self._server: Optional[socketserver.TCPServer] = None
        self._thread: Optional[threading.Thread] = None

    def start(self) -> None:
        receiver = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):  # type: ignore[override]
                parsed = urllib.parse.urlparse(self.path)
                if parsed.path != receiver.path:
                    self.send_response(404)
                    self.end_headers()
                    self.wfile.write(b"Not Found")
                    return
                q = urllib.parse.parse_qs(parsed.query)
                receiver.code = (q.get("code") or [""])[0]
                receiver.recv_state = (q.get("state") or [""])[0]
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"Login complete. You can close this tab.")
                # Shutdown server in a separate thread to avoid deadlock
                threading.Thread(
                    target=self.server.shutdown,
                    daemon=True,
                ).start()

            def log_message(self, format, *args):
                return  # silence

        # Ensure port is free (quick check)
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                s.bind((self.host, self.port))
            except OSError as e:
                msg = f"Cannot bind callback server on " f"{self.host}:{self.port}: {e}"
                print(msg, file=sys.stderr)
                sys.exit(2)
        # Start server
        self._server = socketserver.TCPServer((self.host, self.port), Handler)
        self._thread = threading.Thread(
            target=self._server.serve_forever,
            daemon=True,
        )
        self._thread.start()

    def wait_for_code(self, timeout: int) -> tuple[Optional[str], Optional[str]]:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.code and self.recv_state:
                break
            time.sleep(0.25)
        self.stop()
        return self.code, self.recv_state

    def stop(self) -> None:
        if self._server:
            try:
                self._server.shutdown()
                self._server.server_close()
            except Exception:
                pass
            self._server = None
        if self._thread and self._thread.is_alive():
            try:
                self._thread.join(timeout=1.0)
            except Exception:
                pass
            self._thread = None


def main() -> int:
    cfg = Config.from_env()

    # Parse redirect target
    ru = urllib.parse.urlparse(cfg.redirect_uri)
    host = ru.hostname or "127.0.0.1"
    port = ru.port or 8765
    path = ru.path or "/callback"

    # PKCE
    code_verifier, code_challenge = make_pkce_pair()
    state = b64url_no_pad(os.urandom(16))

    # Start local receiver
    receiver = CodeReceiver(host, port, path, state)
    receiver.start()

    # Build auth URL
    auth_params = {
        "response_type": "code",
        "client_id": cfg.client_id,
        "redirect_uri": cfg.redirect_uri,
        "scope": cfg.scope,
        "code_challenge": code_challenge,
        "code_challenge_method": "S256",
        "state": state,
    }
    auth_url_full = cfg.auth_url
    sep = "&" if ("?" in cfg.auth_url) else "?"
    auth_url_full += sep + urllib.parse.urlencode(
        auth_params,
        quote_via=urllib.parse.quote,
    )

    # Open browser
    webbrowser.open(auth_url_full)
    print(
        "Opened browser for login. If it didn't open, visit:\n",
        f"{auth_url_full}",
        sep="",
    )

    # Wait for callback
    code, recv_state = receiver.wait_for_code(timeout=cfg.timeout)
    if not code:
        msg = f"No authorization code on {cfg.redirect_uri} (timeout)."
        print(msg, file=sys.stderr)
        return 2
    if recv_state != state:
        print("State mismatch. Aborting.", file=sys.stderr)
        return 2

    # Exchange code for tokens
    token_params = {
        "grant_type": "authorization_code",
        "client_id": cfg.client_id,
        "code": code,
        "redirect_uri": cfg.redirect_uri,
        "code_verifier": code_verifier,
    }
    data = urllib.parse.urlencode(token_params).encode()
    try:
        req = urllib.request.Request(
            cfg.token_url,
            data=data,
            headers={"Content-Type": "application/x-www-form-urlencoded"},
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            body = resp.read().decode()
    except Exception as e:
        print(f"Token request failed: {e}", file=sys.stderr)
        return 3

    try:
        tok = json.loads(body)
    except Exception:
        print("Failed to parse token response:", file=sys.stderr)
        print(body, file=sys.stderr)
        return 3

    token = tok.get("id_token") or tok.get("access_token")
    if not token:
        print("No usable token in response:", file=sys.stderr)
        print(json.dumps(tok, indent=2), file=sys.stderr)
        return 3

    # Login to Spacetime
    try:
        r = subprocess.run(
            ["spacetime", "login", "--token", token],
            check=False,
        )
        if r.returncode == 0:
            print("Spacetime login succeeded via OIDC token.")
            return 0
        else:
            print(
                "Spacetime login with provided token failed. "
                "You may need a Spacetime-issued token or configure "
                "--server-issued-login instead.",
                file=sys.stderr,
            )
            return 4
    except FileNotFoundError:
        print("'spacetime' CLI not found in PATH.", file=sys.stderr)
        return 4


if __name__ == "__main__":
    sys.exit(main())
