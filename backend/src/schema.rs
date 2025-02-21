// @generated automatically by Diesel CLI.

diesel::table! {
    conversations (id) {
        id -> Integer,
        user_id -> Integer,
        conversation_sid -> Text,
        service_sid -> Text,
        created_at -> Integer,
        active -> Bool,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        email -> Text,
        password_hash -> Text,
        phone_number -> Text,
        nickname -> Nullable<Text>,
        time_to_live -> Nullable<Integer>,
        verified -> Bool,
        iq -> Integer,
        notify_credits -> Bool,
        locality -> Text,
        info -> Nullable<Text>,
        preferred_number -> Nullable<Text>,
    }
}

diesel::joinable!(conversations -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    conversations,
    users,
);
