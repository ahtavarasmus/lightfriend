use backend::handlers::admin_handlers::parse_broadcast_excluded_emails;

#[test]
fn parses_resend_csv_to_column_for_matching_subject() {
    let csv = "\
id,created_at,subject,from,to,cc,bcc,reply_to,last_event
1,2026-06-19,Set your Lightfriend password,Lightfriend <notifications@lightfriend.ai>,rasmus2@ahtava.com,,,rasmus@lightfriend.ai,delivered
2,2026-06-19,a quick Lightfriend update,Lightfriend <notifications@lightfriend.ai>,User@Example.COM,,,rasmus@lightfriend.ai,delivered
3,2026-06-19,a quick Lightfriend update,Lightfriend <notifications@lightfriend.ai>,second@example.com,,,rasmus@lightfriend.ai,delivered";

    let excluded = parse_broadcast_excluded_emails(csv, Some("a quick Lightfriend update"));

    assert_eq!(excluded.len(), 2);
    assert!(excluded.contains("user@example.com"));
    assert!(excluded.contains("second@example.com"));
    assert!(!excluded.contains("notifications@lightfriend.ai"));
    assert!(!excluded.contains("rasmus@lightfriend.ai"));
    assert!(!excluded.contains("rasmus2@ahtava.com"));
}

#[test]
fn parses_plain_email_lists() {
    let excluded = parse_broadcast_excluded_emails(
        "First <first@example.com>\nsecond@example.com, bad-value; THIRD@EXAMPLE.COM",
        None,
    );

    assert_eq!(excluded.len(), 3);
    assert!(excluded.contains("first@example.com"));
    assert!(excluded.contains("second@example.com"));
    assert!(excluded.contains("third@example.com"));
}
