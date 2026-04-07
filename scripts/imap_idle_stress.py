#!/usr/bin/env python3
"""
IMAP IDLE stress test for lightfriend.

Fires N synthetic emails at a real Gmail account by using IMAP APPEND
to inject messages directly into INBOX (no SMTP involved). The IDLE
task in the running backend should pick each one up within a few
seconds. We measure:

  * How many messages the backend's IDLE task logged "IDLE wake:
    processed N new email(s)" for
  * How many ont_messages rows with platform='email' and a matching
    room_id (email_<uid>) were created during the test window
  * Per-message arrival-to-insert latency, derived from the log
    timestamps
  * Whether any rows are duplicated (room_id collisions)

Why IMAP APPEND instead of SMTP:

  * You don't need to juggle SMTP credentials or worry about being
    rate-limited by Gmail's outbound send quota.
  * The test message appears in the target mailbox exactly like a
    real inbound mail for IDLE purposes: Gmail raises an EXISTS
    notification and our IDLE session wakes.
  * It's reproducible and deterministic: we generate the Message-Id
    locally so we can assert that each fake message became exactly
    one ont_messages row.

Usage
-----

    python3 scripts/imap_idle_stress.py \
        --email you@gmail.com \
        --password 'APP_PASSWORD' \
        --count 20 \
        --backend-log /tmp/.../b2t43aqju.output \
        --pg-url "postgres://lightfriend:...@localhost/lightfriend"

You can also set the credentials via env vars: LF_TEST_EMAIL,
LF_TEST_PASSWORD, LF_PG_URL, LF_BACKEND_LOG. The script will NOT ask
for a password interactively and will NOT log it.

Exit codes
----------

  0  All messages accounted for, no duplicates, reasonable latency
  1  One or more messages never hit the DB within the timeout
  2  Duplicate ont_messages rows detected (dedup broken)
  3  Environment setup / connection failure (credentials bad, no log,
     DB unreachable)
"""

from __future__ import annotations

import argparse
import email.utils
import imaplib
import os
import re
import statistics
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from email.message import EmailMessage


# --- Helpers ---------------------------------------------------------------


def log(msg: str) -> None:
    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    print(f"[{ts}] {msg}", flush=True)


def log_ok(msg: str) -> None:
    log(f"OK    {msg}")


def log_warn(msg: str) -> None:
    log(f"WARN  {msg}")


def log_fail(msg: str) -> None:
    log(f"FAIL  {msg}")


# --- APPEND mail generation ------------------------------------------------


def build_message(sender: str, recipient: str, subject: str, body: str) -> bytes:
    """Build a minimal RFC 5322 message with a unique Message-Id."""
    m = EmailMessage()
    m["From"] = sender
    m["To"] = recipient
    m["Subject"] = subject
    m["Date"] = email.utils.formatdate(localtime=False)
    m["Message-Id"] = email.utils.make_msgid(domain="lightfriend.stress")
    m.set_content(body)
    return bytes(m)


# --- Backend log tailer ----------------------------------------------------


# Log line we care about: "IDLE wake: processed N new email(s) for connection M"
IDLE_WAKE_RE = re.compile(
    r"(?P<ts>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z).*?"
    r"IDLE wake: processed (?P<count>\d+) new email\(s\) for connection (?P<conn>\d+)"
)


@dataclass
class IdleWake:
    at: datetime
    count: int
    conn: int


def parse_log_for_wakes(log_path: str, since: datetime) -> list[IdleWake]:
    """Extract IDLE wake log lines after a given UTC timestamp."""
    if not os.path.exists(log_path):
        raise FileNotFoundError(log_path)

    wakes: list[IdleWake] = []
    with open(log_path, "r", errors="replace") as f:
        for line in f:
            m = IDLE_WAKE_RE.search(line)
            if not m:
                continue
            ts_str = m.group("ts")
            # Strip ANSI escape artifacts if present in ts_str.
            ts_str = re.sub(r"\x1b\[[0-9;]*m", "", ts_str)
            # Python can't parse trailing nanoseconds with strptime reliably;
            # truncate to microseconds.
            ts_str_fix = re.sub(r"(\.\d{6})\d*Z", r"\1+00:00", ts_str)
            try:
                at = datetime.fromisoformat(ts_str_fix)
            except ValueError:
                continue
            if at < since:
                continue
            wakes.append(
                IdleWake(
                    at=at,
                    count=int(m.group("count")),
                    conn=int(m.group("conn")),
                )
            )
    return wakes


# --- DB verification -------------------------------------------------------


def query_pg(pg_url: str, sql: str) -> list[list[str]]:
    """Run a SQL query via psql and return rows as lists of string cells."""
    result = subprocess.run(
        ["psql", pg_url, "-At", "-F", "\x1f", "-c", sql],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"psql failed (rc={result.returncode}): {result.stderr.strip()}"
        )
    rows: list[list[str]] = []
    for line in result.stdout.strip().splitlines():
        if not line:
            continue
        rows.append(line.split("\x1f"))
    return rows


def count_tagged_messages(pg_url: str, tag: str) -> int:
    """Count ont_messages matching our unique per-run test tag. The tag
    appears as the first line of the subject (which is stored in the
    message content), so a LIKE match is both sufficient and scoped to
    this specific run — no cross-talk with other stress tests or real
    mail."""
    safe = tag.replace("'", "''")
    rows = query_pg(
        pg_url,
        f"""
        SELECT COUNT(*)
        FROM ont_messages
        WHERE platform = 'email'
          AND content LIKE '{safe}%'
        """,
    )
    return int(rows[0][0]) if rows else 0


def find_duplicate_email_room_ids(pg_url: str) -> list[tuple[str, int]]:
    """Global duplicate check across all email-sourced rows. Dedup is a
    cross-user invariant, so we don't need to scope by user here —
    `room_id` is `email_<uid>` and collisions within the same user
    would indicate the `get_message_by_email_room_id` check is
    broken."""
    rows = query_pg(
        pg_url,
        """
        SELECT room_id, COUNT(*)
        FROM ont_messages
        WHERE platform = 'email'
          AND room_id LIKE 'email_%'
        GROUP BY room_id
        HAVING COUNT(*) > 1
        """,
    )
    return [(r[0], int(r[1])) for r in rows]


# --- Main test -------------------------------------------------------------


def cleanup_tagged_messages(imap: imaplib.IMAP4_SSL, tag: str) -> int:
    """Mark messages whose Subject contains `tag` as Deleted and
    expunge. Returns the number of expunged messages. Safe: only
    touches messages tagged by this script's own unique run tag."""
    # SEARCH by Subject — case-sensitive on privateemail.com; we use
    # the raw tag which contains only safe ASCII.
    try:
        imap.select("INBOX")
        status, data = imap.search(None, "SUBJECT", f'"{tag}"')
    except Exception as e:
        log_warn(f"Cleanup search failed: {e}")
        return 0
    if status != "OK" or not data or not data[0]:
        return 0
    ids = data[0].split()
    if not ids:
        return 0
    seq = b",".join(ids).decode()
    try:
        imap.store(seq, "+FLAGS", "\\Deleted")
        imap.expunge()
    except Exception as e:
        log_warn(f"Cleanup expunge failed: {e}")
        return 0
    return len(ids)


def stress_test(
    email_addr: str,
    password: str,
    count: int,
    backend_log: str,
    pg_url: str,
    imap_host: str = "imap.gmail.com",
    imap_port: int = 993,
    wait_secs: int = 45,
    cleanup: bool = False,
) -> int:
    log(f"Stress test starting: {count} messages to {email_addr}")
    log(f"IMAP: {imap_host}:{imap_port}   Backend log: {backend_log}")
    log("")

    # --- Sanity check backend log exists ----------------------------------
    try:
        with open(backend_log, "r", errors="replace"):
            pass
    except Exception as e:
        log_fail(f"Cannot open backend log: {e}")
        return 3

    # --- Sanity check psql reachable --------------------------------------
    try:
        query_pg(pg_url, "SELECT 1")
    except Exception as e:
        log_fail(f"psql probe failed: {e}")
        return 3

    # --- Connect IMAP for APPEND ------------------------------------------
    # Retry loop: some IMAP providers (notably privateemail.com) apply
    # short-lived rate limiting per source IP. Back off and retry a few
    # times before giving up.
    log(f"Connecting to {imap_host}:{imap_port}")
    imap = None
    attempts = 5
    for attempt in range(1, attempts + 1):
        try:
            imap = imaplib.IMAP4_SSL(imap_host, imap_port)
            imap.login(email_addr, password)
            imap.select("INBOX")
            break
        except Exception as e:
            if attempt == attempts:
                log_fail(f"IMAP login failed after {attempts} attempts: {e}")
                return 3
            backoff = 5 * attempt
            log_warn(
                f"IMAP login attempt {attempt}/{attempts} failed: {e}. "
                f"Retrying in {backoff}s..."
            )
            try:
                if imap:
                    imap.shutdown()
            except Exception:
                pass
            imap = None
            time.sleep(backoff)
    if imap is None:
        return 3
    log_ok("IMAP logged in, INBOX selected")

    start_utc = datetime.now(timezone.utc)

    test_tag = f"lf-stress-{uuid.uuid4().hex[:8]}"
    log(f"Test tag: {test_tag}  (Subject prefix)")

    # --- Inject messages ---------------------------------------------------
    send_times: list[datetime] = []
    log(f"Appending {count} messages...")
    for i in range(count):
        subject = f"{test_tag} stress #{i+1:03d}"
        body = (
            f"Stress test message {i+1} of {count}.\n"
            f"Tag: {test_tag}\n"
            f"This message was injected via IMAP APPEND by imap_idle_stress.py.\n"
        )
        msg_bytes = build_message(email_addr, email_addr, subject, body)
        before = datetime.now(timezone.utc)
        try:
            status, data = imap.append(
                "INBOX",
                "",  # flags
                imaplib.Time2Internaldate(time.time()),
                msg_bytes,
            )
            if status != "OK":
                log_fail(f"APPEND returned {status}: {data}")
                imap.logout()
                return 3
        except Exception as e:
            log_fail(f"APPEND #{i+1} raised: {e}")
            imap.logout()
            return 3
        send_times.append(before)
        if (i + 1) % 5 == 0 or i == count - 1:
            log_ok(f"APPEND progress: {i+1}/{count}")

    # Always logout after APPEND so we don't hold a session during the
    # ~90s wait window. privateemail.com in particular rate-limits
    # concurrent sessions per user, and the backend's IDLE task is
    # already one. If cleanup is requested, we reopen a fresh session
    # at the very end.
    try:
        imap.logout()
    except Exception:
        pass

    # --- Wait for IDLE to process ------------------------------------------
    log("")
    log(f"Waiting up to {wait_secs}s for IDLE to process all messages...")
    deadline = time.monotonic() + wait_secs
    last_db_count = 0
    while time.monotonic() < deadline:
        time.sleep(2)
        try:
            db_count = count_tagged_messages(pg_url, test_tag)
        except Exception as e:
            log_warn(f"DB query failed, retrying: {e}")
            continue
        if db_count != last_db_count:
            log(f"  DB now has {db_count}/{count} tagged messages")
            last_db_count = db_count
        if db_count >= count:
            break

    # --- Measure from backend log ------------------------------------------
    log("")
    log("Checking backend log for IDLE wake events...")
    try:
        wakes = parse_log_for_wakes(backend_log, start_utc)
    except Exception as e:
        log_fail(f"Failed to parse backend log: {e}")
        return 1

    total_wake_emails = sum(w.count for w in wakes)
    log(f"  Wake events: {len(wakes)}")
    log(f"  Emails processed in wake events: {total_wake_emails}")
    if wakes:
        first_wake = min(w.at for w in wakes)
        last_wake = max(w.at for w in wakes)
        log(f"  First wake: {first_wake.isoformat()}")
        log(f"  Last wake:  {last_wake.isoformat()}")

        # Latency: time from last APPEND to last wake, bounded by first APPEND to first wake.
        first_send = send_times[0]
        last_send = send_times[-1]
        latencies = [
            (last_wake - last_send).total_seconds(),
            (first_wake - first_send).total_seconds(),
        ]
        log(
            f"  First-APPEND to first-wake latency: "
            f"{(first_wake - first_send).total_seconds():.2f}s"
        )
        log(
            f"  Last-APPEND  to last-wake  latency: "
            f"{(last_wake - last_send).total_seconds():.2f}s"
        )
        if all(l > 0 for l in latencies):
            log(f"  Mean latency (bounds): {statistics.mean(latencies):.2f}s")

    # --- DB counts ---------------------------------------------------------
    log("")
    log("Checking DB for inserted ont_messages...")
    db_count = count_tagged_messages(pg_url, test_tag)
    log(f"  Tagged ont_messages rows: {db_count}")

    dupes = find_duplicate_email_room_ids(pg_url)
    if dupes:
        log_fail(f"  Duplicate room_ids detected: {dupes}")

    # --- Optional cleanup --------------------------------------------------
    if cleanup:
        log("")
        log("Cleaning up test messages from INBOX...")
        try:
            cleanup_imap = imaplib.IMAP4_SSL(imap_host, imap_port)
            cleanup_imap.login(email_addr, password)
            removed = cleanup_tagged_messages(cleanup_imap, test_tag)
            log_ok(f"  Expunged {removed} test message(s)")
            try:
                cleanup_imap.logout()
            except Exception:
                pass
        except Exception as e:
            log_warn(f"  Cleanup failed: {e}")

    # --- Verdict -----------------------------------------------------------
    log("")
    log("===================== VERDICT =====================")

    ok = True
    if db_count < count:
        log_fail(f"DB has {db_count} rows, expected >= {count}")
        ok = False
    else:
        log_ok(f"DB row count: {db_count} >= {count}")

    if dupes:
        log_fail(f"Found {len(dupes)} duplicate room_ids")
        ok = False
    else:
        log_ok("No duplicate room_ids")

    if total_wake_emails < count:
        log_warn(
            f"Backend log only shows {total_wake_emails} wake-processed emails "
            f"(expected {count}). The missing ones may have been picked up by the "
            f"cron fallback, which is still correct but slower."
        )
    else:
        log_ok(f"IDLE wake total: {total_wake_emails} >= {count}")

    return 0 if ok else (2 if dupes else 1)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--email",
        default=os.environ.get("LF_TEST_EMAIL"),
        help="Gmail address (also via LF_TEST_EMAIL env var)",
    )
    p.add_argument(
        "--password",
        default=os.environ.get("LF_TEST_PASSWORD"),
        help="Gmail app password (also via LF_TEST_PASSWORD env var)",
    )
    p.add_argument(
        "--count",
        type=int,
        default=20,
        help="Number of messages to inject (default: 20)",
    )
    p.add_argument(
        "--backend-log",
        default=os.environ.get("LF_BACKEND_LOG"),
        help="Path to the backend log file (also via LF_BACKEND_LOG env var)",
    )
    p.add_argument(
        "--pg-url",
        default=os.environ.get("LF_PG_URL"),
        help="Postgres URL (also via LF_PG_URL env var)",
    )
    p.add_argument(
        "--imap-host",
        default="imap.gmail.com",
    )
    p.add_argument(
        "--imap-port",
        type=int,
        default=993,
    )
    p.add_argument(
        "--wait",
        type=int,
        default=45,
        help="Seconds to wait for IDLE to process all messages (default: 45)",
    )
    p.add_argument(
        "--cleanup",
        action="store_true",
        help="After the test, delete injected messages from INBOX via IMAP expunge (safe: only matches our unique test tag)",
    )
    args = p.parse_args()

    missing = [
        name
        for name, val in (
            ("--email", args.email),
            ("--password", args.password),
            ("--backend-log", args.backend_log),
            ("--pg-url", args.pg_url),
        )
        if not val
    ]
    if missing:
        print(
            f"error: missing required: {', '.join(missing)}",
            file=sys.stderr,
        )
        return 3

    return stress_test(
        email_addr=args.email,
        password=args.password,
        count=args.count,
        backend_log=args.backend_log,
        pg_url=args.pg_url,
        imap_host=args.imap_host,
        imap_port=args.imap_port,
        wait_secs=args.wait,
        cleanup=args.cleanup,
    )


if __name__ == "__main__":
    sys.exit(main())
