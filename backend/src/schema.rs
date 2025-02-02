// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Integer,
        username -> Text,
        password_hash -> Text,
        email -> Text,
        phone_number -> Nullable<Text>,
        nickname -> Nullable<Text>,
    }
}

diesel::table! {
    phone_verification_otps (id) {
        id -> Integer,
        user_id -> Integer,
        phone_number -> Text,
        otp_code -> Text,
        created_at -> Timestamp,
        expires_at -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    users,
    phone_verification_otps,
);
