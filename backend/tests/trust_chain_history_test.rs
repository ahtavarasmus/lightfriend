use backend::handlers::trust_chain_handlers::{
    compare_history_order_desc, decode_image_proposed_activates_at, HistoryOrderKey,
};
use std::cmp::Ordering;

#[test]
fn newer_block_sorts_before_older_timestamped_build() {
    let newer_without_timestamp = HistoryOrderKey {
        block_number: Some(0x1c53550b),
        log_index: Some(0),
        timestamp: None,
    };
    let older_with_timestamp = HistoryOrderKey {
        block_number: Some(0x1b375603),
        log_index: Some(0),
        timestamp: Some("2026-04-26T17:30:46+00:00".to_string()),
    };

    assert_eq!(
        compare_history_order_desc(&newer_without_timestamp, &older_with_timestamp),
        Ordering::Less
    );
}

#[test]
fn unknown_blocks_fall_back_to_timestamp_order() {
    let newer = HistoryOrderKey {
        block_number: None,
        log_index: None,
        timestamp: Some("2026-06-19T19:30:06+00:00".to_string()),
    };
    let older = HistoryOrderKey {
        block_number: None,
        log_index: None,
        timestamp: Some("2026-04-26T17:30:46+00:00".to_string()),
    };

    assert_eq!(compare_history_order_desc(&newer, &older), Ordering::Less);
}

#[test]
fn image_proposed_data_decodes_activates_at_as_timestamp_hint() {
    let data = concat!(
        "0x",
        "0000000000000000000000000000000000000000000000000000000000000040",
        "000000000000000000000000000000000000000000000000000000006a3598be"
    );

    assert_eq!(
        decode_image_proposed_activates_at(data).as_deref(),
        Some("2026-06-19T19:30:06+00:00")
    );
}
