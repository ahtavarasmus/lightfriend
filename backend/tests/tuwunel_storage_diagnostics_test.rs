use backend::utils::tuwunel_storage_diagnostics::extract_correlated_database_files_report;
use serde_json::{json, Value};

fn reply(event_id: &str, sender: &str, timestamp: u64, target: &str, body: &str) -> Value {
    json!({
        "type": "m.room.message",
        "event_id": event_id,
        "sender": sender,
        "origin_server_ts": timestamp,
        "content": {
            "msgtype": "m.notice",
            "body": body,
            "m.relates_to": {
                "m.in_reply_to": {
                    "event_id": target
                }
            }
        }
    })
}

#[test]
fn collects_only_server_reply_chain_and_orders_segments() {
    let command_event_id = "$command";
    let server_user = "@conduit:localhost";
    let first = reply(
        "$first",
        server_user,
        100,
        command_event_id,
        "Database files:\n| lev | sst | keys | dels | size | column |\n\
         | ---: | :--- | ---: | ---: | ---: | :--- |\n\
         | 6 | 000001.sst | 100+ | 10- | 1000 | pduid_pdu |",
    );
    let second = reply(
        "$second",
        server_user,
        200,
        "$first",
        "| 0 | 000002.sst | 50+ | 5- | 500 | pduid_pdu |",
    );
    let unrelated_sender = reply(
        "$attacker",
        "@someone:localhost",
        150,
        command_event_id,
        "| 0 | fake.sst | 1+ | 0- | 999999 | attacker_column |",
    );
    let unrelated_command = reply(
        "$unrelated",
        server_user,
        50,
        "$different-command",
        "| lev | sst | keys | dels | size | column |\n\
         | ---: | :--- | ---: | ---: | ---: | :--- |\n\
         | 0 | stale.sst | 1+ | 0- | 777 | stale_column |",
    );

    let report = extract_correlated_database_files_report(
        &[second, unrelated_sender, unrelated_command, first],
        command_event_id,
        server_user,
    )
    .expect("valid correlated report");

    assert!(report.contains("1000 | pduid_pdu"));
    assert!(report.contains("500 | pduid_pdu"));
    assert!(!report.contains("attacker_column"));
    assert!(!report.contains("stale_column"));
    assert!(
        report.find("1000 | pduid_pdu").expect("first row")
            < report.find("500 | pduid_pdu").expect("second row")
    );
}

#[test]
fn accepts_thread_root_relation_from_server_user() {
    let event = json!({
        "type": "m.room.message",
        "event_id": "$thread",
        "sender": "@conduit:localhost",
        "origin_server_ts": 100,
        "content": {
            "msgtype": "m.text",
            "body": "| lev | sst | keys | dels | size | column |\n\
                     | ---: | :--- | ---: | ---: | ---: | :--- |\n\
                     | 1 | 000003.sst | 12+ | 2- | 345 | eventid_pduid |",
            "m.relates_to": {
                "rel_type": "m.thread",
                "event_id": "$command"
            }
        }
    });

    let report =
        extract_correlated_database_files_report(&[event], "$command", "@conduit:localhost")
            .expect("thread response should be accepted");
    assert!(report.contains("345 | eventid_pduid"));
}

#[test]
fn rejects_incomplete_or_malformed_report() {
    let no_rows = reply(
        "$no-rows",
        "@conduit:localhost",
        100,
        "$command",
        "| lev | sst | keys | dels | size | column |\n\
         | ---: | :--- | ---: | ---: | ---: | :--- |",
    );
    assert!(
        extract_correlated_database_files_report(&[no_rows], "$command", "@conduit:localhost")
            .is_none()
    );

    let malformed = reply(
        "$malformed",
        "@conduit:localhost",
        100,
        "$command",
        "| lev | sst | keys | dels | size | column |\n\
         | ---: | :--- | ---: | ---: | ---: | :--- |\n\
         | nope | fake.sst | secret | data | here | content |",
    );
    assert!(extract_correlated_database_files_report(
        &[malformed],
        "$command",
        "@conduit:localhost"
    )
    .is_none());
}
