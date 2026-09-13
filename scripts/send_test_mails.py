#!/usr/bin/env python3
"""
Send generated test emails to a mailing list via standard SMTP.

Reads email files from a directory (e.g. test-data/mail_XXXX.txt) where:
- Line 1 is the Subject
- Remainder is the email body text

Features:
- Standard SMTP transmission (supports plain, STARTTLS, SSL/TLS).
- CLI arguments for target list, sender address, and credentials (no defaults).
- Pacing/throttling control (--delay).
- Dry-run mode to inspect messages without sending.
- Automatic reconnection on connection drop.
- Live progress reporting and summary metrics.
"""

from __future__ import annotations

import argparse
import email.utils
import os
import smtplib
import sys
import time
from email.message import EmailMessage
from pathlib import Path
from typing import Optional


def parse_email_file(file_path: Path) -> tuple[str, str]:
    """Parse an email text file into (subject, body)."""
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()

    lines = content.splitlines()
    if not lines:
        return ("No Subject", "")

    first_line = lines[0].strip()
    # Strip leading 'Subject:' or 'Betreff:' if present
    if first_line.lower().startswith("subject:"):
        subject = first_line[len("subject:"):].strip()
    elif first_line.lower().startswith("betreff:"):
        subject = first_line[len("betreff:"):].strip()
    else:
        subject = first_line

    # Body is everything after the first line (skipping leading blank lines)
    body_lines = lines[1:]
    while body_lines and not body_lines[0].strip():
        body_lines.pop(0)

    body = "\n".join(body_lines).strip()
    return (subject or "No Subject", body)


def build_smtp_connection(
    host: str,
    port: int,
    use_ssl: bool,
    use_starttls: bool,
    username: Optional[str],
    password: Optional[str],
    timeout: float = 30.0,
) -> smtplib.SMTP:
    """Create and authenticate an SMTP connection."""
    if use_ssl:
        server: smtplib.SMTP = smtplib.SMTP_SSL(host, port, timeout=timeout)
    else:
        server = smtplib.SMTP(host, port, timeout=timeout)

    server.ehlo()

    if use_starttls and not use_ssl:
        server.starttls()
        server.ehlo()

    if username and password:
        server.login(username, password)

    return server


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Send test emails to a mailing list via standard SMTP."
    )
    # Required parameters (no defaults)
    parser.add_argument(
        "--to",
        "--list",
        dest="to_addr",
        required=True,
        type=str,
        help="Target mailing list email address (required, e.g. it-ag@solawis.de)",
    )
    parser.add_argument(
        "--from",
        "--from-addr",
        dest="from_addr",
        required=True,
        type=str,
        help="Sender email address (required, e.g. member@solawis.de)",
    )

    # SMTP server options
    parser.add_argument(
        "--host",
        type=str,
        default=os.environ.get("SMTP_HOST", "127.0.0.1"),
        help="SMTP server hostname or IP (default: 127.0.0.1 or $SMTP_HOST)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=int(os.environ.get("SMTP_PORT", "25")),
        help="SMTP server port (default: 25 or $SMTP_PORT)",
    )
    parser.add_argument(
        "--username",
        "-u",
        type=str,
        default=os.environ.get("SMTP_USERNAME") or os.environ.get("SMTP_USER"),
        help="SMTP authentication username (optional or $SMTP_USERNAME)",
    )
    parser.add_argument(
        "--password",
        "-p",
        type=str,
        default=os.environ.get("SMTP_PASSWORD") or os.environ.get("SMTP_PASS"),
        help="SMTP authentication password (optional or $SMTP_PASSWORD)",
    )
    parser.add_argument(
        "--starttls",
        action="store_true",
        help="Upgrade connection to STARTTLS",
    )
    parser.add_argument(
        "--ssl",
        action="store_true",
        help="Connect over SSL/TLS directly (e.g. port 465)",
    )

    # Input and pacing options
    parser.add_argument(
        "--input-dir",
        "-i",
        type=Path,
        default=Path("test-data"),
        help="Directory containing email text files (default: test-data)",
    )
    parser.add_argument(
        "--pattern",
        type=str,
        default="mail_*.txt",
        help="Glob pattern for email files (default: mail_*.txt)",
    )
    parser.add_argument(
        "--count",
        "-c",
        type=int,
        default=None,
        help="Maximum number of emails to send (default: all files found)",
    )
    parser.add_argument(
        "--start-index",
        type=int,
        default=1,
        help="1-based offset index to start sending from (default: 1)",
    )
    parser.add_argument(
        "--delay",
        "-d",
        type=float,
        default=0.0,
        help="Delay in seconds between sending emails (default: 0.0)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate sending without connecting to SMTP server",
    )

    args = parser.parse_args()

    input_dir = args.input_dir.resolve()
    if not input_dir.exists() or not input_dir.is_dir():
        print(f"Error: Input directory not found: {input_dir}", file=sys.stderr)
        return 1

    # Find and sort files
    all_files = sorted(input_dir.glob(args.pattern))
    if not all_files:
        # Fallback to *.txt if pattern didn't match
        all_files = sorted(input_dir.glob("*.txt"))

    if not all_files:
        print(f"Error: No email files matching '{args.pattern}' found in {input_dir}", file=sys.stderr)
        return 1

    # Slice based on start-index and count
    start_offset = max(0, args.start_index - 1)
    files_to_send = all_files[start_offset:]
    if args.count is not None and args.count > 0:
        files_to_send = files_to_send[:args.count]

    total = len(files_to_send)
    print(f"Found {len(all_files)} total files. Selected {total} emails to send.")
    print(f"From: {args.from_addr} -> To: {args.to_addr}")

    if args.dry_run:
        print("\n--- DRY RUN MODE (No emails will be sent) ---")
        for i, fpath in enumerate(files_to_send, start=1):
            subject, body = parse_email_file(fpath)
            snippet = body.replace("\n", " ")[:80]
            print(f"[{i}/{total}] {fpath.name}")
            print(f"       Subject: {subject}")
            print(f"       Body:    {snippet}...")
        print(f"\nDry run complete. Validated {total} email files.")
        return 0

    print(f"Connecting to SMTP server at {args.host}:{args.port} (SSL: {args.ssl}, STARTTLS: {args.starttls})...")
    server: Optional[smtplib.SMTP] = None

    try:
        server = build_smtp_connection(
            args.host,
            args.port,
            args.ssl,
            args.starttls,
            args.username,
            args.password,
        )
        print("Connected and authenticated successfully.")
    except Exception as e:
        print(f"Failed to connect to SMTP server at {args.host}:{args.port}: {e}", file=sys.stderr)
        return 1

    sent_count = 0
    failed_count = 0
    start_time = time.time()

    try:
        for i, file_path in enumerate(files_to_send, start=1):
            subject, body = parse_email_file(file_path)

            msg = EmailMessage()
            msg["From"] = args.from_addr
            msg["To"] = args.to_addr
            msg["Subject"] = subject
            msg["Date"] = email.utils.formatdate(localtime=True)
            msg["Message-ID"] = email.utils.make_msgid(domain="test-runner.local")
            msg["X-Mailer"] = "SoLaWi-Test-Script/1.0"
            msg.set_content(body)

            send_ok = False
            for attempt in range(2):
                try:
                    if server is None:
                        server = build_smtp_connection(
                            args.host,
                            args.port,
                            args.ssl,
                            args.starttls,
                            args.username,
                            args.password,
                        )
                    server.send_message(msg)
                    send_ok = True
                    break
                except (smtplib.SMTPServerDisconnected, smtplib.SMTPConnectError, BrokenPipeError, ConnectionResetError) as net_err:
                    print(f"Connection lost on {file_path.name} (attempt {attempt+1}): {net_err}. Reconnecting...", file=sys.stderr)
                    server = None
                    time.sleep(1.0)
                except Exception as send_err:
                    print(f"Failed to send {file_path.name}: {send_err}", file=sys.stderr)
                    break

            if send_ok:
                sent_count += 1
                pct = (i / total) * 100
                print(f"[{i}/{total} - {pct:5.1f}%] Sent {file_path.name} | Subject: {subject[:45]}")
            else:
                failed_count += 1
                print(f"[{i}/{total}] FAILED {file_path.name}", file=sys.stderr)

            if args.delay > 0.0 and i < total:
                time.sleep(args.delay)

    finally:
        if server is not None:
            try:
                server.quit()
            except Exception:
                pass

    elapsed = time.time() - start_time
    rate = (sent_count / elapsed) if elapsed > 0 else 0.0
    print(f"\nCompleted sending run in {elapsed:.2f}s ({rate:.1f} mails/sec).")
    print(f"Successfully sent: {sent_count}, Failed: {failed_count}, Total: {total}.")

    return 0 if failed_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
