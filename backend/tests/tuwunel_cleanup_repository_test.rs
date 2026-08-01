use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use diesel::prelude::*;
use serial_test::serial;

#[derive(Debug, QueryableByName)]
struct StoredRoomPurge {
    #[diesel(sql_type = diesel::sql_types::Text)]
    room_id: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    cutoff_ts: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    submitted_cutoff_ts: Option<i32>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    status: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    attempt_count: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    last_error: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    last_discovered_at: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    completed_at: Option<i32>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    updated_at: i32,
}

fn load_server_census_rows(state: &backend::AppState, user_id: i32) -> Vec<StoredRoomPurge> {
    let mut conn = state.pg_pool.get().unwrap();
    diesel::sql_query(
        "SELECT room_id, cutoff_ts, submitted_cutoff_ts, status, attempt_count, last_error, \
                last_discovered_at, completed_at, updated_at \
         FROM tuwunel_room_history_purges \
         WHERE user_id = $1 AND service = 'server_all_rooms' \
         ORDER BY room_id",
    )
    .bind::<diesel::sql_types::Integer, _>(user_id)
    .load(&mut conn)
    .unwrap()
}

#[test]
#[serial]
fn repeated_server_census_only_queues_new_rooms() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = &state.tuwunel_cleanup_repository;
    let original_rooms = vec!["!a:localhost".to_string(), "!b:localhost".to_string()];

    assert_eq!(
        repository
            .record_server_census_rooms_once(user.id, "server_all_rooms", &original_rooms, 40, 100,)
            .unwrap(),
        2
    );

    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::sql_query(
            "UPDATE tuwunel_room_history_purges \
             SET submitted_cutoff_ts = 40, status = 'succeeded', attempt_count = 2, \
                 completed_at = 150, updated_at = 150 \
             WHERE user_id = $1 AND service = 'server_all_rooms' AND room_id = '!a:localhost'",
        )
        .bind::<diesel::sql_types::Integer, _>(user.id)
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "UPDATE tuwunel_room_history_purges \
             SET attempt_count = 1, last_error = 'transient', updated_at = 120 \
             WHERE user_id = $1 AND service = 'server_all_rooms' AND room_id = '!b:localhost'",
        )
        .bind::<diesel::sql_types::Integer, _>(user.id)
        .execute(&mut conn)
        .unwrap();
    }

    let rescanned_rooms = vec![
        "!a:localhost".to_string(),
        "!b:localhost".to_string(),
        "!c:localhost".to_string(),
    ];
    assert_eq!(
        repository
            .record_server_census_rooms_once(
                user.id,
                "server_all_rooms",
                &rescanned_rooms,
                500,
                600,
            )
            .unwrap(),
        1
    );
    assert_eq!(
        repository
            .record_server_census_rooms_once(
                user.id,
                "server_all_rooms",
                &rescanned_rooms,
                700,
                800,
            )
            .unwrap(),
        0
    );

    let rows = load_server_census_rows(&state, user.id);
    assert_eq!(rows.len(), 3);

    assert_eq!(rows[0].room_id, "!a:localhost");
    assert_eq!(rows[0].cutoff_ts, 40);
    assert_eq!(rows[0].submitted_cutoff_ts, Some(40));
    assert_eq!(rows[0].status, "succeeded");
    assert_eq!(rows[0].attempt_count, 2);
    assert_eq!(rows[0].last_discovered_at, 100);
    assert_eq!(rows[0].completed_at, Some(150));
    assert_eq!(rows[0].updated_at, 150);

    assert_eq!(rows[1].room_id, "!b:localhost");
    assert_eq!(rows[1].cutoff_ts, 40);
    assert_eq!(rows[1].submitted_cutoff_ts, None);
    assert_eq!(rows[1].status, "pending");
    assert_eq!(rows[1].attempt_count, 1);
    assert_eq!(rows[1].last_error.as_deref(), Some("transient"));
    assert_eq!(rows[1].last_discovered_at, 100);
    assert_eq!(rows[1].updated_at, 120);

    assert_eq!(rows[2].room_id, "!c:localhost");
    assert_eq!(rows[2].cutoff_ts, 500);
    assert_eq!(rows[2].status, "pending");
    assert_eq!(rows[2].attempt_count, 0);
    assert_eq!(rows[2].last_discovered_at, 600);
    assert_eq!(rows[2].updated_at, 600);
}
