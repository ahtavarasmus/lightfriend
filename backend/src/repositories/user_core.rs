use crate::{
    models::user_models::{NewUserSettings, User, UserSettings},
    pg_models::{NewPgUserInfo, PgUserInfo},
    pg_schema::{user_info, user_settings, users},
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::Text;
use std::error::Error;

use diesel::dsl::sql;
use diesel::sql_types::BigInt;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// Hash a magic token with SHA-256 so that only the digest is stored in the database,
/// not the raw token that appears in the email link.
fn hash_magic_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

define_sql_function! {
    fn lower(x: Text) -> Text;
}

fn normalize_phone(phone: &str) -> String {
    phone
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect()
}

/// Type alias for tier3 settings
pub type Tier3Settings = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Parameters for updating a user profile
pub struct UpdateProfileParams<'a> {
    pub user_id: i32,
    pub email: &'a str,
    pub phone_number: &'a str,
    pub nickname: &'a str,
    pub info: &'a str,
    pub timezone: &'a str,
    pub timezone_auto: &'a bool,
    pub notification_type: Option<&'a str>,
    pub save_context: Option<i32>,
    pub location: &'a str,
    pub nearby_places: &'a str,
    pub preferred_number: Option<&'a str>,
}

/// Trait defining UserCore operations for dependency injection and testing.
pub trait UserCoreOps: Send + Sync {
    // Core queries
    fn find_by_id(&self, user_id: i32) -> Result<Option<User>, DieselError>;
    fn find_by_email(&self, email: &str) -> Result<Option<User>, DieselError>;
    fn find_by_phone_number(&self, phone: &str) -> Result<Option<User>, DieselError>;
    fn find_by_magic_token(&self, token: &str) -> Result<Option<User>, DieselError>;
    fn find_by_valid_magic_token(
        &self,
        token: &str,
        now_ts: i32,
    ) -> Result<Option<User>, DieselError>;
    fn get_all_users(&self) -> Result<Vec<User>, DieselError>;
    fn get_users_by_tier(&self, tier: &str) -> Result<Vec<User>, DieselError>;

    // User CRUD
    fn create_user(&self, new_user: crate::handlers::auth_dtos::NewUser)
        -> Result<(), DieselError>;
    fn delete_user(&self, user_id: i32) -> Result<(), DieselError>;

    // Core field updates
    fn update_password(&self, user_id: i32, password_hash: &str) -> Result<(), DieselError>;
    fn clear_magic_token(&self, user_id: i32) -> Result<(), DieselError>;
    fn update_phone_number(&self, user_id: i32, phone: &str) -> Result<(), DieselError>;
    fn update_nickname(&self, user_id: i32, nickname: &str) -> Result<(), DieselError>;
    fn update_preferred_number(&self, user_id: i32, number: &str) -> Result<(), DieselError>;
    fn set_refresh_token_hash(&self, user_id: i32, token_hash: &str) -> Result<(), DieselError>;
    fn mark_refresh_token_compromised(&self, user_id: i32) -> Result<(), DieselError>;

    // User info
    fn ensure_user_info_exists(&self, user_id: i32) -> Result<(), DieselError>;
    fn get_user_info(&self, user_id: i32) -> Result<PgUserInfo, DieselError>;
    fn update_info(&self, user_id: i32, info: &str) -> Result<(), DieselError>;
    fn update_location(&self, user_id: i32, location: &str) -> Result<(), DieselError>;
    fn update_user_coordinates(&self, user_id: i32, lat: f32, lon: f32) -> Result<(), DieselError>;
    fn update_nearby_places(&self, user_id: i32, places: &str) -> Result<(), DieselError>;
    fn update_timezone(&self, user_id: i32, tz: &str) -> Result<(), DieselError>;
    fn update_timezone_auto(&self, user_id: i32, auto: bool) -> Result<(), DieselError>;

    // User settings
    fn ensure_user_settings_exist(&self, user_id: i32) -> Result<(), DieselError>;
    fn get_user_settings(&self, user_id: i32) -> Result<UserSettings, DieselError>;
    fn update_notify(&self, user_id: i32, notify: bool) -> Result<(), DieselError>;
    fn update_agent_language(&self, user_id: i32, lang: &str) -> Result<(), DieselError>;
    fn update_save_context(&self, user_id: i32, ctx: i32) -> Result<(), DieselError>;
    fn update_notification_type(
        &self,
        user_id: i32,
        ntype: Option<&str>,
    ) -> Result<(), DieselError>;
    fn update_llm_provider(&self, user_id: i32, provider: &str) -> Result<(), DieselError>;
    fn get_llm_provider(&self, user_id: i32) -> Result<Option<String>, DieselError>;

    // Phone service
    fn update_phone_service_active(&self, user_id: i32, active: bool) -> Result<(), DieselError>;
    fn get_phone_service_active(&self, user_id: i32) -> Result<bool, DieselError>;

    // Auto-create items
    fn update_auto_create_items(&self, user_id: i32, value: bool) -> Result<(), DieselError>;
    fn get_auto_create_items(&self, user_id: i32) -> Result<bool, DieselError>;

    // System important notify
    fn update_system_important_notify(&self, user_id: i32, value: bool) -> Result<(), DieselError>;
    fn get_system_important_notify(&self, user_id: i32) -> Result<bool, DieselError>;

    // Digest settings
    fn update_digest_enabled(&self, user_id: i32, value: bool) -> Result<(), DieselError>;
    fn update_digest_time(&self, user_id: i32, value: Option<&str>) -> Result<(), DieselError>;

    // System-level auto tracking
    fn update_auto_track_items_system(&self, user_id: i32, value: bool) -> Result<(), DieselError>;
    fn update_auto_confirm_tracked_items(
        &self,
        user_id: i32,
        value: bool,
    ) -> Result<(), DieselError>;

    // Notification settings
    fn get_default_notification_mode(&self, user_id: i32) -> Result<String, DieselError>;
    fn set_default_notification_mode(&self, user_id: i32, mode: &str) -> Result<(), DieselError>;
    fn get_default_notification_type(&self, user_id: i32) -> Result<String, DieselError>;
    fn set_default_notification_type(&self, user_id: i32, ntype: &str) -> Result<(), DieselError>;
    fn get_default_notify_on_call(&self, user_id: i32) -> Result<bool, DieselError>;
    fn set_default_notify_on_call(&self, user_id: i32, notify: bool) -> Result<(), DieselError>;

    // Phone contact notification settings (Tier 2)
    fn get_phone_contact_notification_mode(&self, user_id: i32) -> Result<String, DieselError>;
    fn set_phone_contact_notification_mode(
        &self,
        user_id: i32,
        mode: &str,
    ) -> Result<(), DieselError>;
    fn get_phone_contact_notification_type(&self, user_id: i32) -> Result<String, DieselError>;
    fn set_phone_contact_notification_type(
        &self,
        user_id: i32,
        ntype: &str,
    ) -> Result<(), DieselError>;
    fn get_phone_contact_notify_on_call(&self, user_id: i32) -> Result<bool, DieselError>;
    fn set_phone_contact_notify_on_call(
        &self,
        user_id: i32,
        notify: bool,
    ) -> Result<(), DieselError>;

    fn get_call_notify(&self, user_id: i32) -> Result<bool, DieselError>;
    fn update_call_notify(&self, user_id: i32, notify: bool) -> Result<(), DieselError>;

    fn update_critical_enabled(
        &self,
        user_id: i32,
        enabled: Option<String>,
    ) -> Result<(), DieselError>;
    fn update_action_on_critical_message(
        &self,
        user_id: i32,
        action: Option<String>,
    ) -> Result<(), DieselError>;

    // Complex notification info
    fn get_critical_notification_info(
        &self,
        user_id: i32,
    ) -> Result<crate::handlers::profile_handlers::CriticalNotificationInfo, DieselError>;

    // Profile (complex transaction)
    fn update_profile(&self, params: UpdateProfileParams<'_>) -> Result<(), DieselError>;

    // BYOT check
    fn is_byot_user(&self, user_id: i32) -> bool;

    // Subscription & billing
    fn update_subscription_tier(&self, user_id: i32, tier: Option<&str>)
        -> Result<(), DieselError>;
    fn update_next_billing_date(&self, user_id: i32, ts: i32) -> Result<(), DieselError>;
    fn get_next_billing_date(&self, user_id: i32) -> Result<Option<i32>, DieselError>;
    fn update_last_credits_notification(&self, user_id: i32, ts: i32) -> Result<(), DieselError>;
    fn clear_last_credits_notification(&self, user_id: i32) -> Result<(), DieselError>;
    fn update_auto_topup(
        &self,
        user_id: i32,
        active: bool,
        amount: Option<f32>,
    ) -> Result<(), DieselError>;
    // Validation
    fn email_exists(&self, email: &str) -> Result<bool, DieselError>;
    fn phone_number_exists(&self, phone: &str) -> Result<bool, DieselError>;
    fn is_admin(&self, user_id: i32) -> Result<bool, DieselError>;

    // Country & phone assignment
    fn update_sub_country(&self, user_id: i32, country: Option<&str>) -> Result<(), DieselError>;
    fn set_preferred_number_to_us_default(
        &self,
        user_id: i32,
    ) -> Result<String, Box<dyn Error + Send + Sync>>;
    fn set_preferred_number_for_country(
        &self,
        user_id: i32,
        country: &str,
    ) -> Result<Option<String>, Box<dyn Error + Send + Sync>>;

    // Magic token
    fn set_magic_token(&self, user_id: i32, token: &str) -> Result<(), DieselError>;
}

pub struct UserCore {
    pg_pool: PgDbPool,
}

impl UserCore {
    pub fn new(pg_pool: PgDbPool) -> Self {
        Self { pg_pool }
    }
}

/// Check if a quiet rule's tag conditions match a notification context.
impl UserCoreOps for UserCore {
    fn create_user(
        &self,
        new_user: crate::handlers::auth_dtos::NewUser,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::insert_into(users::table)
            .values(&new_user)
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn find_by_email(&self, search_email: &str) -> Result<Option<User>, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let user = users::table
            .filter(lower(users::email).eq(lower(search_email)))
            .first::<User>(&mut pg_conn)
            .optional()?;
        Ok(user)
    }

    fn find_by_id(&self, user_id: i32) -> Result<Option<User>, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let user = users::table
            .find(user_id)
            .first::<User>(&mut pg_conn)
            .optional()?;
        Ok(user)
    }

    fn find_by_phone_number(&self, phone_number: &str) -> Result<Option<User>, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let cleaned_phone = normalize_phone(phone_number);
        let matches = users::table
            .filter(users::phone_number.eq(cleaned_phone))
            .limit(2)
            .load::<User>(&mut pg_conn)?;
        match matches.len() {
            0 => Ok(None),
            1 => Ok(matches.into_iter().next()),
            _ => Err(DieselError::RollbackTransaction),
        }
    }

    fn find_by_magic_token(&self, token: &str) -> Result<Option<User>, DieselError> {
        let hashed = hash_magic_token(token);
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let user = users::table
            .filter(users::magic_token.eq(&hashed))
            .first::<User>(&mut pg_conn)
            .optional()?;
        Ok(user)
    }

    fn find_by_valid_magic_token(
        &self,
        token: &str,
        now_ts: i32,
    ) -> Result<Option<User>, DieselError> {
        let hashed = hash_magic_token(token);
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let user = users::table
            .filter(users::magic_token.eq(&hashed))
            .filter(users::magic_token_expires_at.is_not_null())
            .filter(users::magic_token_expires_at.gt(now_ts))
            .first::<User>(&mut pg_conn)
            .optional()?;
        Ok(user)
    }

    fn set_magic_token(&self, user_id: i32, token: &str) -> Result<(), DieselError> {
        let hashed = hash_magic_token(token);
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32
            + (24 * 60 * 60);
        diesel::update(users::table.find(user_id))
            .set((
                users::magic_token.eq(Some(&hashed)),
                users::magic_token_expires_at.eq(Some(expiry)),
            ))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_all_users(&self) -> Result<Vec<User>, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let users_list = users::table.load::<User>(&mut pg_conn)?;
        Ok(users_list)
    }

    fn get_users_by_tier(&self, tier: &str) -> Result<Vec<User>, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let users_list = users::table
            .filter(users::sub_tier.eq(Some(tier)))
            .load::<User>(&mut pg_conn)?;
        Ok(users_list)
    }

    fn delete_user(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::pg_schema::{
            bridges, imap_connection, message_history, processed_emails, refund_info, tesla,
            totp_backup_codes, totp_secrets, usage_logs, user_secrets, webauthn_challenges,
            webauthn_credentials, youtube,
        };

        // Delete from PG tables
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::delete(bridges::table.filter(bridges::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(imap_connection::table.filter(imap_connection::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(message_history::table.filter(message_history::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(processed_emails::table.filter(processed_emails::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(tesla::table.filter(tesla::user_id.eq(user_id))).execute(&mut pg_conn)?;
        diesel::delete(totp_backup_codes::table.filter(totp_backup_codes::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(totp_secrets::table.filter(totp_secrets::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(usage_logs::table.filter(usage_logs::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(user_info::table.filter(user_info::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(user_secrets::table.filter(user_secrets::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(webauthn_challenges::table.filter(webauthn_challenges::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(
            webauthn_credentials::table.filter(webauthn_credentials::user_id.eq(user_id)),
        )
        .execute(&mut pg_conn)?;
        diesel::delete(youtube::table.filter(youtube::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(refund_info::table.filter(refund_info::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;
        diesel::delete(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .execute(&mut pg_conn)?;

        // Finally delete the user
        diesel::delete(users::table.find(user_id)).execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_password(&self, user_id: i32, password_hash: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table)
            .filter(users::id.eq(user_id))
            .set(users::password_hash.eq(password_hash))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn clear_magic_token(&self, user_id: i32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table)
            .filter(users::id.eq(user_id))
            .set((
                users::magic_token.eq::<Option<String>>(None),
                users::magic_token_expires_at.eq::<Option<i32>>(None),
            ))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn set_refresh_token_hash(&self, user_id: i32, token_hash: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table)
            .filter(users::id.eq(user_id))
            .set((
                users::refresh_token_hash.eq(Some(token_hash)),
                users::refresh_token_compromised.eq(false),
            ))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn mark_refresh_token_compromised(&self, user_id: i32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table)
            .filter(users::id.eq(user_id))
            .set((
                users::refresh_token_hash.eq::<Option<String>>(None),
                users::refresh_token_compromised.eq(true),
            ))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_phone_number(&self, user_id: i32, phone: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table)
            .filter(users::id.eq(user_id))
            .set(users::phone_number.eq(phone))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    // Helper function to ensure user_info exists (PG)
    fn ensure_user_info_exists(&self, user_id: i32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        let exists = user_info::table
            .filter(user_info::user_id.eq(user_id))
            .first::<PgUserInfo>(&mut pg_conn)
            .optional()?
            .is_some();

        if !exists {
            let new_user_info = NewPgUserInfo {
                user_id,
                location: None,
                info: None,
                timezone: None,
                nearby_places: None,
            };

            diesel::insert_into(user_info::table)
                .values(&new_user_info)
                .execute(&mut pg_conn)?;
        }

        Ok(())
    }

    // Get user_info, ensuring it exists first (PG)
    fn get_user_info(&self, user_id: i32) -> Result<PgUserInfo, DieselError> {
        self.ensure_user_info_exists(user_id)?;

        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        let info = user_info::table
            .filter(user_info::user_id.eq(user_id))
            .first::<PgUserInfo>(&mut pg_conn)?;

        Ok(info)
    }

    // User settings operations
    fn get_user_settings(&self, user_id: i32) -> Result<UserSettings, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        let settings = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .first::<UserSettings>(&mut pg_conn)
            .optional()?;

        match settings {
            Some(settings) => Ok(settings),
            None => {
                let new_settings = NewUserSettings {
                    user_id,
                    notify: true,
                    notification_type: None,
                    timezone_auto: None,
                    agent_language: "en".to_string(),
                    sub_country: None,
                    save_context: Some(5),
                    critical_enabled: Some("sms".to_string()),
                    notify_about_calls: true,
                };

                diesel::insert_into(user_settings::table)
                    .values(&new_settings)
                    .execute(&mut pg_conn)?;

                let created_settings = user_settings::table
                    .filter(user_settings::user_id.eq(user_id))
                    .first::<UserSettings>(&mut pg_conn)?;

                Ok(created_settings)
            }
        }
    }

    fn get_default_notification_mode(&self, user_id: i32) -> Result<String, DieselError> {
        let settings = self.get_user_settings(user_id)?;
        Ok(settings
            .default_notification_mode
            .unwrap_or_else(|| "critical".to_string()))
    }

    fn set_default_notification_mode(&self, user_id: i32, mode: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(user_settings::table)
            .filter(user_settings::user_id.eq(user_id))
            .set(user_settings::default_notification_mode.eq(mode))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_default_notification_type(&self, user_id: i32) -> Result<String, DieselError> {
        let settings = self.get_user_settings(user_id)?;
        Ok(settings
            .default_notification_type
            .unwrap_or_else(|| "sms".to_string()))
    }

    fn set_default_notification_type(
        &self,
        user_id: i32,
        noti_type: &str,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(user_settings::table)
            .filter(user_settings::user_id.eq(user_id))
            .set(user_settings::default_notification_type.eq(noti_type))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_default_notify_on_call(&self, user_id: i32) -> Result<bool, DieselError> {
        let settings = self.get_user_settings(user_id)?;
        Ok(settings.default_notify_on_call != 0)
    }

    fn set_default_notify_on_call(&self, user_id: i32, notify: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(user_settings::table)
            .filter(user_settings::user_id.eq(user_id))
            .set(user_settings::default_notify_on_call.eq(if notify { 1 } else { 0 }))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_phone_contact_notification_mode(&self, user_id: i32) -> Result<String, DieselError> {
        let settings = self.get_user_settings(user_id)?;
        Ok(settings
            .phone_contact_notification_mode
            .unwrap_or_else(|| "critical".to_string()))
    }

    fn set_phone_contact_notification_mode(
        &self,
        user_id: i32,
        mode: &str,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(user_settings::table)
            .filter(user_settings::user_id.eq(user_id))
            .set(user_settings::phone_contact_notification_mode.eq(mode))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_phone_contact_notification_type(&self, user_id: i32) -> Result<String, DieselError> {
        let settings = self.get_user_settings(user_id)?;
        Ok(settings
            .phone_contact_notification_type
            .unwrap_or_else(|| "sms".to_string()))
    }

    fn set_phone_contact_notification_type(
        &self,
        user_id: i32,
        ntype: &str,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(user_settings::table)
            .filter(user_settings::user_id.eq(user_id))
            .set(user_settings::phone_contact_notification_type.eq(ntype))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_phone_contact_notify_on_call(&self, user_id: i32) -> Result<bool, DieselError> {
        let settings = self.get_user_settings(user_id)?;
        Ok(settings.phone_contact_notify_on_call != 0)
    }

    fn set_phone_contact_notify_on_call(
        &self,
        user_id: i32,
        notify: bool,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(user_settings::table)
            .filter(user_settings::user_id.eq(user_id))
            .set(user_settings::phone_contact_notify_on_call.eq(if notify { 1 } else { 0 }))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    // Helper function to ensure user settings exist
    fn ensure_user_settings_exist(&self, user_id: i32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        let settings_exist = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .first::<UserSettings>(&mut pg_conn)
            .optional()?
            .is_some();

        if !settings_exist {
            let new_settings = NewUserSettings {
                user_id,
                notify: true,
                notification_type: None,
                timezone_auto: None,
                agent_language: "en".to_string(),
                sub_country: None,
                save_context: Some(5),
                critical_enabled: Some("sms".to_string()),
                notify_about_calls: true,
            };

            diesel::insert_into(user_settings::table)
                .values(&new_settings)
                .execute(&mut pg_conn)?;
        }
        Ok(())
    }

    // Basic validation methods
    fn email_exists(&self, search_email: &str) -> Result<bool, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let existing_user: Option<User> = users::table
            .filter(lower(users::email).eq(lower(search_email)))
            .first::<User>(&mut pg_conn)
            .optional()?;
        Ok(existing_user.is_some())
    }

    fn phone_number_exists(&self, search_phone: &str) -> Result<bool, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let existing_user: Option<User> = users::table
            .filter(users::phone_number.eq(normalize_phone(search_phone)))
            .first::<User>(&mut pg_conn)
            .optional()?;
        Ok(existing_user.is_some())
    }

    fn is_admin(&self, user_id: i32) -> Result<bool, DieselError> {
        let admin_emails =
            std::env::var("ADMIN_EMAILS").expect("ADMIN_EMAILS environment variable must be set");

        // Parse comma-separated list, trim whitespace
        let admin_list: Vec<&str> = admin_emails
            .split(',')
            .map(|e| e.trim())
            .filter(|e| !e.is_empty())
            .collect();

        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        let user = users::table.find(user_id).first::<User>(&mut pg_conn)?;
        Ok(admin_list.contains(&user.email.as_str()))
    }

    fn update_sub_country(&self, user_id: i32, country: Option<&str>) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Update the settings
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::sub_country.eq(country))
            .execute(&mut pg_conn)?;

        Ok(())
    }

    fn update_preferred_number(
        &self,
        user_id: i32,
        preferred_number: &str,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table.find(user_id))
            .set(users::preferred_number.eq(preferred_number))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_agent_language(&self, user_id: i32, language: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::agent_language.eq(language))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    // Individual field update methods for inline editing
    fn update_nickname(&self, user_id: i32, nickname: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table.find(user_id))
            .set(users::nickname.eq(nickname))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_info(&self, user_id: i32, info: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_info_exists(user_id)?;
        diesel::update(user_info::table.filter(user_info::user_id.eq(user_id)))
            .set(user_info::info.eq(info))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_location(&self, user_id: i32, location: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_info_exists(user_id)?;
        diesel::update(user_info::table.filter(user_info::user_id.eq(user_id)))
            .set(user_info::location.eq(location))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_user_coordinates(&self, user_id: i32, lat: f32, lon: f32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_info_exists(user_id)?;
        diesel::update(user_info::table.filter(user_info::user_id.eq(user_id)))
            .set((user_info::latitude.eq(lat), user_info::longitude.eq(lon)))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_nearby_places(&self, user_id: i32, nearby_places: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_info_exists(user_id)?;
        diesel::update(user_info::table.filter(user_info::user_id.eq(user_id)))
            .set(user_info::nearby_places.eq(nearby_places))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_timezone_auto(&self, user_id: i32, timezone_auto: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::timezone_auto.eq(timezone_auto))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_phone_service_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::phone_service_active.eq(active))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_phone_service_active(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        let result = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::phone_service_active)
            .first::<bool>(&mut pg_conn)?;
        Ok(result)
    }

    fn update_auto_create_items(&self, user_id: i32, value: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::auto_create_items.eq(value))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_auto_create_items(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        let result = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::auto_create_items)
            .first::<bool>(&mut pg_conn)?;
        Ok(result)
    }

    fn update_system_important_notify(&self, user_id: i32, value: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::system_important_notify.eq(value))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_system_important_notify(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        let result = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::system_important_notify)
            .first::<bool>(&mut pg_conn)?;
        Ok(result)
    }

    fn update_digest_enabled(&self, user_id: i32, value: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::digest_enabled.eq(value))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_digest_time(&self, user_id: i32, value: Option<&str>) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::digest_time.eq(value))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_auto_track_items_system(&self, user_id: i32, value: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::auto_track_items_system.eq(value))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_auto_confirm_tracked_items(
        &self,
        user_id: i32,
        value: bool,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::auto_confirm_tracked_items.eq(value))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_llm_provider(&self, user_id: i32, provider: &str) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::llm_provider.eq(provider))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_llm_provider(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        let result = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::llm_provider)
            .first::<Option<String>>(&mut pg_conn)?;
        Ok(result)
    }

    fn update_notification_type(
        &self,
        user_id: i32,
        notification_type: Option<&str>,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::notification_type.eq(notification_type))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_save_context(&self, user_id: i32, save_context: i32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        self.ensure_user_settings_exist(user_id)?;
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::save_context.eq(save_context))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn set_preferred_number_to_us_default(
        &self,
        user_id: i32,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let preferred_number = std::env::var("USA_PHONE").expect("USA_PHONE not found");

        // Update the user's preferred number in the database
        self.update_preferred_number(user_id, &preferred_number)?;

        Ok(preferred_number)
    }

    /// Set preferred number based on user's country code
    /// Returns the phone number that was set, or None if country doesn't have a dedicated number
    fn set_preferred_number_for_country(
        &self,
        user_id: i32,
        country_code: &str,
    ) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        let preferred_number = match country_code {
            "US" => std::env::var("USA_PHONE").ok(),
            "CA" => std::env::var("CAN_PHONE").ok(),
            "FI" => std::env::var("FIN_PHONE").ok(),
            "NL" => std::env::var("NL_PHONE").ok(),
            "GB" | "UK" => std::env::var("GB_PHONE").ok(),
            "AU" => std::env::var("AUS_PHONE").ok(),
            // Notification-only countries use US messaging service, set USA_PHONE as preferred
            _ if crate::utils::country::is_notification_only_country_code(country_code) => {
                std::env::var("USA_PHONE").ok()
            }
            _ => None,
        };

        if let Some(ref number) = preferred_number {
            self.update_preferred_number(user_id, number)?;
        }

        Ok(preferred_number)
    }

    // Update user's profile
    fn update_profile(&self, params: UpdateProfileParams<'_>) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        println!(
            "Repository: Updating user {} with notification type: {:?}",
            params.user_id, params.notification_type
        );

        // Start a PG transaction
        pg_conn.transaction(|pg_conn| {
            // Check if phone number exists for a different user
            let existing_phone = users::table
                .filter(users::phone_number.eq(params.phone_number))
                .filter(users::id.ne(params.user_id))
                .first::<User>(pg_conn)
                .optional()?;

            if existing_phone.is_some() {
                return Err(DieselError::RollbackTransaction);
            }
            // Check if email exists for a different user
            let existing_email = users::table
                .filter(users::email.eq(params.email.to_lowercase()))
                .filter(users::id.ne(params.user_id))
                .first::<User>(pg_conn)
                .optional()?;

            if existing_email.is_some() {
                return Err(DieselError::NotFound);
            }
            // Update user table
            diesel::update(users::table.find(params.user_id))
                .set((
                    users::email.eq(params.email),
                    users::phone_number.eq(params.phone_number),
                    users::nickname.eq(params.nickname),
                    users::preferred_number.eq(params.preferred_number),
                ))
                .execute(pg_conn)?;
            // Ensure user settings exist
            let settings_exist = user_settings::table
                .filter(user_settings::user_id.eq(params.user_id))
                .first::<UserSettings>(pg_conn)
                .optional()?
                .is_some();
            if !settings_exist {
                let new_settings = NewUserSettings {
                    user_id: params.user_id,
                    notify: true,
                    notification_type: None,
                    timezone_auto: None,
                    agent_language: "en".to_string(),
                    sub_country: None,
                    save_context: Some(5),
                    critical_enabled: Some("sms".to_string()),
                    notify_about_calls: true,
                };
                diesel::insert_into(user_settings::table)
                    .values(&new_settings)
                    .execute(pg_conn)?;
            }
            // Update the settings
            diesel::update(user_settings::table.filter(user_settings::user_id.eq(params.user_id)))
                .set((
                    user_settings::timezone_auto.eq(params.timezone_auto),
                    user_settings::notification_type
                        .eq(params.notification_type.map(|s| s.to_string())),
                    user_settings::save_context.eq(params.save_context),
                ))
                .execute(pg_conn)?;
            // Update user_info within the same PG transaction
            let info_exists = user_info::table
                .filter(user_info::user_id.eq(params.user_id))
                .first::<PgUserInfo>(pg_conn)
                .optional()?
                .is_some();
            if !info_exists {
                let new_user_info = NewPgUserInfo {
                    user_id: params.user_id,
                    location: None,
                    info: None,
                    timezone: None,
                    nearby_places: None,
                };
                diesel::insert_into(user_info::table)
                    .values(&new_user_info)
                    .execute(pg_conn)?;
            }
            diesel::update(user_info::table.filter(user_info::user_id.eq(params.user_id)))
                .set((
                    user_info::timezone.eq(params.timezone),
                    user_info::info.eq(params.info),
                    user_info::location.eq(params.location),
                    user_info::nearby_places.eq(params.nearby_places),
                ))
                .execute(pg_conn)?;
            Ok(())
        })?;

        Ok(())
    }

    // Update user's notify preference in user_settings
    fn update_notify(&self, user_id: i32, notify: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Update the settings
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::notify.eq(notify))
            .execute(&mut pg_conn)?;

        Ok(())
    }

    fn update_timezone(&self, user_id: i32, timezone: &str) -> Result<(), DieselError> {
        // First fetch the user settings to check timezone_auto
        let user_settings = self.get_user_settings(user_id)?;
        // Only update if timezone_auto is false (manual timezone setting)
        if !user_settings.timezone_auto.unwrap_or(false) {
            let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
            diesel::update(user_info::table)
                .filter(user_info::user_id.eq(user_id))
                .set(user_info::timezone.eq(timezone.to_string()))
                .execute(&mut pg_conn)?;
        }

        Ok(())
    }

    // Update user's auto top-up settings
    fn update_auto_topup(
        &self,
        user_id: i32,
        active: bool,
        amount: Option<f32>,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        // Update the user's auto top-up settings
        diesel::update(users::table.find(user_id))
            .set((
                users::charge_when_under.eq(active),
                users::charge_back_to.eq(amount),
            ))
            .execute(&mut pg_conn)?;

        Ok(())
    }

    fn update_last_credits_notification(
        &self,
        user_id: i32,
        timestamp: i32,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table.find(user_id))
            .set(users::last_credits_notification.eq(timestamp))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    /// Clear the last_credits_notification field when user adds credits.
    /// This allows the notification to be sent again if credits deplete again.
    fn clear_last_credits_notification(&self, user_id: i32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        diesel::update(users::table.find(user_id))
            .set(users::last_credits_notification.eq(None::<i32>))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_critical_enabled(
        &self,
        user_id: i32,
        enabled: Option<String>,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;
        // Update the critical_enabled setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::critical_enabled.eq(enabled))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn update_action_on_critical_message(
        &self,
        user_id: i32,
        action: Option<String>,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;
        // Update the action_on_critical_message setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::action_on_critical_message.eq(action))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_call_notify(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Get the setting
        let call_notify = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::notify_about_calls)
            .first::<bool>(&mut pg_conn)?;

        Ok(call_notify)
    }

    fn update_call_notify(&self, user_id: i32, call_notify: bool) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;
        // Update the call_notify setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::notify_about_calls.eq(call_notify))
            .execute(&mut pg_conn)?;
        Ok(())
    }

    fn get_critical_notification_info(
        &self,
        user_id: i32,
    ) -> Result<crate::handlers::profile_handlers::CriticalNotificationInfo, diesel::result::Error>
    {
        use crate::pg_schema::usage_logs;
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;
        // Get the critical_enabled and call_notify settings
        let (enabled, call_notify, action_on_critical_message) = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select((
                user_settings::critical_enabled,
                user_settings::notify_about_calls.nullable(),
                user_settings::action_on_critical_message,
            ))
            .first::<(Option<String>, Option<bool>, Option<String>)>(&mut pg_conn)?;
        let call_notify = call_notify.unwrap_or(true); // Default to true if not set
                                                       // Get average critical notifications per day
        let average_critical_per_day = {
            let now: i64 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs() as i64;
            let thirty_days_ago: i64 = now - 2_592_000; // 30 * 86_400
            let active_days_count: i64 = usage_logs::table
                .select(sql::<BigInt>("COUNT(DISTINCT created_at / 86400)"))
                .filter(usage_logs::user_id.eq(user_id))
                .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                .first(&mut pg_conn)?;
            if active_days_count < 3 {
                1.0
            } else {
                let oldest_day: i64 = usage_logs::table
                    .select(sql::<BigInt>("CAST(MIN(created_at / 86400) AS bigint)"))
                    .filter(usage_logs::user_id.eq(user_id))
                    .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                    .first(&mut pg_conn)?;
                let current_day: i64 = now / 86_400;
                let num_days = (current_day - oldest_day + 1) as i64;
                if num_days <= 0 {
                    1.0
                } else {
                    let start_timestamp: i64 = oldest_day * 86_400;
                    let end_timestamp: i64 = (current_day + 1) * 86_400;
                    let total_critical: i64 = usage_logs::table
                        .filter(usage_logs::user_id.eq(user_id))
                        .filter(usage_logs::activity_type.like("%_critical"))
                        .filter(usage_logs::created_at.ge(start_timestamp as i32))
                        .filter(usage_logs::created_at.lt(end_timestamp as i32))
                        .count()
                        .get_result(&mut pg_conn)?;
                    if total_critical == 0 {
                        1.0
                    } else {
                        total_critical as f32 / num_days as f32
                    }
                }
            }
        };
        println!("average per day: {}", average_critical_per_day);
        // Get user's phone number to determine country
        let phone_number = self
            .find_by_id(user_id)?
            .map(|user| user.phone_number)
            .ok_or_else(|| diesel::result::Error::NotFound)?;
        // Determine country based on phone number
        let country = if phone_number.starts_with("+1") {
            "US"
        } else if phone_number.starts_with("+358") {
            "FI"
        } else if phone_number.starts_with("+31") {
            "NL"
        } else if phone_number.starts_with("+44") {
            "UK"
        } else if phone_number.starts_with("+61") {
            "AU"
        } else {
            "Other"
        };
        // Calculate estimated monthly price based on country and notification method
        let estimated_monthly_price = if enabled.is_none() {
            0.0
        } else {
            let notifications_per_month = average_critical_per_day * 30.0; // Assume 30 days per month
            match (country, enabled.as_deref()) {
                ("US", Some("sms")) => notifications_per_month * 0.5, // 1/2 message cost
                ("US", Some("call")) => notifications_per_month * 0.5, // 1/2 message cost
                ("FI", Some("sms")) => notifications_per_month * 0.15,
                ("FI", Some("call")) => notifications_per_month * 0.70,
                ("NL", Some("sms")) => notifications_per_month * 0.15,
                ("NL", Some("call")) => notifications_per_month * 0.45,
                ("UK", Some("sms")) => notifications_per_month * 0.15,
                ("UK", Some("call")) => notifications_per_month * 0.15,
                ("AU", Some("sms")) => notifications_per_month * 0.15,
                ("AU", Some("call")) => notifications_per_month * 0.15,
                _ => 0.0, // No pricing for "Other" or disabled
            }
        };
        Ok(
            crate::handlers::profile_handlers::CriticalNotificationInfo {
                enabled,
                average_critical_per_day,
                estimated_monthly_price,
                call_notify,
                action_on_critical_message,
            },
        )
    }

    fn update_next_billing_date(&self, user_id: i32, timestamp: i32) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        diesel::update(users::table.find(user_id))
            .set(users::next_billing_date_timestamp.eq(timestamp))
            .execute(&mut pg_conn)?;

        Ok(())
    }

    fn update_subscription_tier(
        &self,
        user_id: i32,
        tier: Option<&str>,
    ) -> Result<(), DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        diesel::update(users::table.find(user_id))
            .set(users::sub_tier.eq(tier))
            .execute(&mut pg_conn)?;

        Ok(())
    }

    fn get_next_billing_date(&self, user_id: i32) -> Result<Option<i32>, DieselError> {
        let mut pg_conn = self.pg_pool.get().expect("Failed to get PG connection");

        let timestamp = users::table
            .find(user_id)
            .select(users::next_billing_date_timestamp)
            .first::<Option<i32>>(&mut pg_conn)?;

        Ok(timestamp)
    }

    /// Check if user is on BYOT (Bring Your Own Twilio) plan by checking plan_type
    fn is_byot_user(&self, user_id: i32) -> bool {
        let mut pg_conn = match self.pg_pool.get() {
            Ok(c) => c,
            Err(_) => return false,
        };

        users::table
            .filter(users::id.eq(user_id))
            .select(users::plan_type)
            .first::<Option<String>>(&mut pg_conn)
            .map(|pt| pt.as_deref() == Some("byot"))
            .unwrap_or(false)
    }
}
