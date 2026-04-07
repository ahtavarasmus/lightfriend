# IMAP IDLE chaos / pre-deploy checklist

Run these manually against a local backend pointed at a real test Gmail
account before deploying the IDLE change to prod. The stress script at
`scripts/imap_idle_stress.py` covers the happy path; this checklist
covers the edge cases that are too brittle or too manual to automate.

For every test, replace `<PG_URL>` with your test database URL and
`<LOG>` with the running backend's stdout log file.

## 0. Environment setup

- Backend running locally with the async-imap + IDLE code
- One IMAP connection (Gmail) hooked up in the DB
- `psql` and `python3` on `$PATH`
- `scripts/imap_idle_stress.py` runnable
- Grep helpers:
  ```bash
  tail_idle () { tail -f "$1" | grep --line-buffered -E "imap_idle|IDLE" ; }
  ```

---

## 1. Smoke: the IDLE task is actually running

```bash
grep -E "IDLE established|Spawned IDLE task" <LOG>
```

**Expected**: one `Spawned IDLE task for connection <id>` line and one
`IDLE established for connection <id>` line per active IMAP
connection.

**Fails if**: no lines found, `init() failed`, `open session` errors,
or the task exits with `server does not advertise IDLE`.

---

## 2. Burst: 20 APPENDs in one pass

```bash
python3 scripts/imap_idle_stress.py \
    --email "$LF_TEST_EMAIL" \
    --password "$LF_TEST_PASSWORD" \
    --count 20 \
    --backend-log "$LF_BACKEND_LOG" \
    --pg-url "$LF_PG_URL" \
    --wait 60
```

**Expected**: exit code `0`. Script reports `DB row count: 20 >= 20`
and `No duplicate room_ids`. IDLE wake events (possibly several,
possibly one bulk wake) account for all 20.

**Fails if**:
- Fewer than 20 rows appear before the `--wait` deadline (IDLE is
  dropping notifications or Gmail is throttling APPENDs — check for
  `ConnectionError` or `FetchError` in the log).
- Duplicate `room_id`s exist (dedup broken — check
  `get_message_by_email_room_id` against a time-windowed race with
  the cron).

---

## 3. Cron + IDLE concurrent dedup

Verifies option D dedup (`get_message_by_email_room_id`) when the
polling cron fires at the same moment IDLE is processing a push.

1. Set your system clock or wait until the next `:00`, `:10`, `:20`...
   (the cron is `0 */10 * * * *` — every 10 minutes).
2. ~5 seconds before the cron tick, run
   `scripts/imap_idle_stress.py --count 5`.
3. Watch the log for both `IDLE wake: processed N new email(s)` and
   `Message monitor job` activity in the same minute.
4. Verify no duplicates:
   ```bash
   psql "$LF_PG_URL" -c "
     SELECT room_id, COUNT(*)
     FROM ont_messages
     WHERE platform = 'email' AND room_id LIKE 'email_%'
     GROUP BY room_id
     HAVING COUNT(*) > 1;
   "
   ```

**Expected**: zero rows returned (no duplicates).

**Fails if**: any row returned. Means the
`get_message_by_email_room_id` check raced with itself — either the
insertion must use a `UNIQUE` constraint (not done in this PR) or the
check needs to be inside a transaction with the insert.

---

## 4. Network blip: connection drops mid-IDLE

Requires `sudo` and `pfctl`. **Only on macOS.** On Linux use `iptables`
equivalents.

1. Confirm IDLE is established: `grep "IDLE established" <LOG>`
2. Block outbound port 993:
   ```bash
   echo "block drop out proto tcp to any port 993" | sudo pfctl -ef -
   ```
3. Watch the log for errors:
   ```bash
   tail -f <LOG> | grep -E "IDLE|imap"
   ```
   You should see one of:
   - `IDLE wait error for connection X: ...; reconnecting`
   - `done() failed for connection X`
   - `failed to open session for connection X`
4. Wait 30 seconds, then unblock:
   ```bash
   sudo pfctl -F all -f /etc/pf.conf && sudo pfctl -d
   ```
5. Inject a test message while blocked (or right after unblocking):
   ```bash
   python3 scripts/imap_idle_stress.py --count 1 --wait 90
   ```
6. Check that the resync catches the missed message via
   `get_max_processed_uid`:
   ```bash
   grep -E "IDLE initial resync|IDLE established" <LOG> | tail
   ```

**Expected**: an `IDLE initial resync for connection X processed 1
new email(s)` line after the reconnect. Exit code 0 from the stress
script.

**Fails if**:
- Task doesn't reconnect (stuck in outer backoff → check the
  `backoff_secs` logic).
- Resync misses the injected message (`MAX(email_uid)` query bug or
  the message was marked processed but never inserted).

---

## 5. 28-minute IDLE refresh

The slowest manual test — run it in the background while doing
something else. Verifies the RFC 2177 re-IDLE cycle and keeps the
connection alive across Gmail's idle eviction.

1. Confirm IDLE is established.
2. Leave it alone for ~30 minutes.
3. Search for the refresh log:
   ```bash
   grep "IDLE timeout (28m)" <LOG>
   ```
4. Inject a test message:
   ```bash
   python3 scripts/imap_idle_stress.py --count 1 --wait 15
   ```

**Expected**: log shows `IDLE timeout (28m) for connection X,
refreshing`, followed by a fresh `IDLE wake: processed 1 new email(s)`
after the APPEND. DB has the new row.

**Fails if**:
- The task silently died instead of refreshing.
- The post-refresh IDLE doesn't receive new pushes (the `session =
  handle.done()` return-value handling is wrong).

---

## 6. Delete connection during IDLE

Verifies `abort_idle_task` cleans up cleanly and doesn't panic or
leave orphaned DashMap entries.

1. Confirm IDLE is established.
2. Call the delete endpoint:
   ```bash
   curl -X DELETE http://localhost:3000/api/auth/imap/disconnect \
     -H "Cookie: $(cat ~/.lf_test_cookie)" \
     -H "Content-Type: application/json" \
     -d '{"email":"your-test@gmail.com"}'
   ```
   *(or delete via the dashboard UI at `/profile/settings`)*
3. Watch the log:
   ```bash
   grep -E "Aborted IDLE task|connection.*no longer exists" <LOG>
   ```

**Expected**:
- `Aborted IDLE task for connection <id>` line.
- Optional follow-up: `connection <id> no longer exists, exiting` if
  the task was mid-outer-loop.
- No panics, no `unwrap` failures.
- The `imap_idle_tasks` DashMap no longer has the entry (not directly
  observable — best verified by re-adding the same account and
  confirming a fresh `Spawned IDLE task` line with a new connection
  id).

**Fails if**:
- Log shows a panic in `run_idle_loop`.
- Re-adding the connection doesn't spawn a new task.

---

## 7. Backend restart with mail in flight

Verifies that the initial resync on startup pulls in anything that
arrived during the downtime.

1. Confirm IDLE is running.
2. Stop the backend (`Ctrl+C`).
3. Inject messages via the webmail UI or another SMTP client
   (NOT the stress script — it needs the backend for DB verification):
   Open gmail.com, send yourself 3 test emails.
4. Wait 30s.
5. Start the backend again.
6. Watch for:
   ```bash
   grep -E "IDLE initial resync|IDLE established" <LOG>
   ```
7. Verify the 3 messages are in the DB:
   ```bash
   psql "$LF_PG_URL" -c "
     SELECT COUNT(*) FROM ont_messages
     WHERE platform = 'email'
       AND created_at >= $(date -u -v-2M +%s);
   "
   ```

**Expected**: `IDLE initial resync for connection X processed 3 new
email(s)` line. DB count matches.

**Fails if**:
- Resync fetches 0 (the `MAX(email_uid)` query returned a stale value
  or NULL and the "first-ever" fallback only fetches the last N where
  N < 3).
- Only some of the 3 messages arrive (UID range computation is off-by-one).

---

## 8. Health endpoint doesn't stall under load

Runs in parallel with test 2 (burst).

In one terminal:
```bash
while true; do
  curl -w '%{time_total}\n' -s -o /dev/null http://localhost:3000/api/health
  sleep 1
done
```

In another terminal: run the burst stress test (`scripts/imap_idle_stress.py
--count 50`).

**Expected**: every `/api/health` response stays well under 1 second
throughout the burst (typically <100ms). No 5xx responses.

**Fails if**: responses climb above a few seconds or start returning
503 — means the DB pool is exhausted or the scheduler is hogging a
connection. This was the regression we fixed in the earlier hotfix;
this test makes sure IDLE didn't reintroduce it.

---

## 9. Task health after panic

Simulates a bug in `process_new_emails` that panics. Requires a
temporary code injection.

1. In `src/handlers/imap_handlers.rs::process_new_emails`, add
   `panic!("chaos");` at the top (temporarily).
2. `cargo check` (don't run).
3. Restart the backend.
4. Inject a test message.
5. Watch the log — the task should panic.
6. Verify the DashMap entry is cleaned up: add a new IMAP connection
   via the UI, confirm a fresh `Spawned IDLE task` line appears.
7. Remove the panic, restart, confirm normal operation resumes.

**Expected**: even after a panic, the system remains responsive and
new connections can be added. The dead task's DashMap entry is either
removed by `is_finished()` detection on the next `spawn_idle_task_for_connection`
or replaced.

**Fails if**: the backend crashes, or the DashMap grows without bound
after repeated panics.

---

## 10. Multi-account isolation (optional)

Only if you have a second test Gmail handy.

1. Add a second IMAP connection via the UI.
2. Verify both `Spawned IDLE task for connection N` lines appear with
   different N values.
3. Run the stress script twice in parallel — once per account — and
   confirm both DB row counts hit the expected count.

**Expected**: both tasks run independently. No cross-contamination
between the two users' `ont_messages`.

---

## Pass criteria for ship

All of:

- [ ] Test 1 (smoke) passes
- [ ] Test 2 (burst 20) passes
- [ ] Test 3 (cron+IDLE) passes — no duplicates in DB
- [ ] Test 4 (network blip) passes — reconnect + resync catches the missed mail
- [ ] Test 6 (delete during IDLE) passes — no panic, clean abort
- [ ] Test 7 (restart resync) passes — no lost mail across restart
- [ ] Test 8 (health under load) passes — `/api/health` stays fast

Tests 5 and 9 are slower / harder to run and can be deferred to a
follow-up if needed. Test 10 is only relevant once we have real
multi-account users.
