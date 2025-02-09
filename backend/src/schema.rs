// @generated automatically by Diesel CLI.

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

diesel::table! {
    users (id) {
        id -> Integer,
        username -> Text,
        password_hash -> Text,
        email -> Nullable<Text>,
        phone_number -> Text,
        nickname -> Nullable<Text>,
    }
}

diesel::joinable!(phone_verification_otps -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    phone_verification_otps,
    users,
);
