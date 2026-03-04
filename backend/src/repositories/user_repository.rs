use crate::utils::encryption::{decrypt, encrypt};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use rand;
use serde::Serialize;

#[derive(Serialize, PartialEq)]
pub struct UsageDataPoint {
    pub timestamp: i32,
    pub credits: f32,
}

use crate::{
    pg_models::{
        NewPgBridge, NewPgContactProfile, NewPgContactProfileException, NewPgImapConnection,
        NewPgUsageLog, PgBridge, PgContactProfile, PgContactProfileException,
    },
    pg_schema::{contact_profile_exceptions, contact_profiles, usage_logs},
    PgDbPool,
};

/// Type alias for IMAP credentials: (description, password, server, port)
pub type ImapCredentials = (String, String, Option<String>, Option<i32>);

/// Parameters for logging usage
pub struct LogUsageParams {
    pub user_id: i32,
    pub sid: Option<String>,
    pub activity_type: String,
    pub credits: Option<f32>,
    pub time_consumed: Option<i32>,
    pub success: Option<bool>,
    pub reason: Option<String>,
    pub status: Option<String>,
    pub recharge_threshold_timestamp: Option<i32>,
    pub zero_credits_timestamp: Option<i32>,
}

/// Parameters for updating a contact profile
pub struct UpdateContactProfileParams {
    pub user_id: i32,
    pub profile_id: i32,
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: i32,
    pub whatsapp_room_id: Option<String>,
    pub telegram_room_id: Option<String>,
    pub signal_room_id: Option<String>,
    pub notes: Option<String>,
}

pub struct UserRepository {
    pub pool: PgDbPool,
    /// SQLite pool for subscription/billing methods that access users/refund_info tables
    pub db_pool: crate::DbPool,
}

impl UserRepository {
    pub fn new(pool: PgDbPool, db_pool: crate::DbPool) -> Self {
        Self { pool, db_pool }
    }

    pub fn get_conversation_history(
        &self,
        user_id: i32,
        limit: i64,
        include_tools: bool,
    ) -> Result<Vec<crate::pg_models::PgMessageHistory>, diesel::result::Error> {
        use crate::pg_schema::message_history;
        use crate::utils::encryption;
        use diesel::prelude::*;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // First, get the user messages to establish time boundaries
        let user_messages = if include_tools {
            message_history::table
                .filter(message_history::user_id.eq(user_id))
                .filter(message_history::role.eq("user"))
                .order_by(message_history::created_at.desc())
                .limit(limit)
                .load::<crate::pg_models::PgMessageHistory>(&mut conn)?
        } else {
            message_history::table
                .filter(message_history::user_id.eq(user_id))
                .filter(message_history::role.ne("tool"))
                .filter(message_history::role.eq("user"))
                .order_by(message_history::created_at.desc())
                .limit(limit)
                .load::<crate::pg_models::PgMessageHistory>(&mut conn)?
        };
        if user_messages.is_empty() {
            return Ok(Vec::new());
        }
        // Get the timestamp of the oldest user message
        let oldest_timestamp = user_messages.last().map(|msg| msg.created_at).unwrap_or(0);
        // Now get all messages from the oldest user message onwards
        let encrypted_messages = message_history::table
            .filter(message_history::user_id.eq(user_id))
            .filter(message_history::created_at.ge(oldest_timestamp))
            .order_by(message_history::created_at.desc())
            .load::<crate::pg_models::PgMessageHistory>(&mut conn)?;
        // Decrypt the content of each message and filter out empty assistant messages
        let mut decrypted_messages = Vec::new();
        for mut msg in encrypted_messages {
            match encryption::decrypt(&msg.encrypted_content) {
                Ok(decrypted_content) => {
                    if msg.role == "assistant"
                        && decrypted_content.is_empty()
                        && msg.tool_calls_json.is_none()
                    {
                        // Skip empty assistant messages
                        continue;
                    }
                    msg.encrypted_content = decrypted_content;
                    decrypted_messages.push(msg);
                }
                Err(e) => {
                    tracing::error!("Failed to decrypt message content: {:?}", e);
                    // Skip messages that fail to decrypt
                    continue;
                }
            }
        }
        Ok(decrypted_messages)
    }

    pub fn create_message_history(
        &self,
        new_message: &crate::pg_models::NewPgMessageHistory,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::message_history;
        use crate::utils::encryption;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Create a new message with encrypted content
        let encrypted_content =
            encryption::encrypt(&new_message.encrypted_content).map_err(|e| {
                tracing::error!("Failed to encrypt message content: {:?}", e);
                DieselError::RollbackTransaction
            })?;

        let encrypted_message = crate::pg_models::NewPgMessageHistory {
            user_id: new_message.user_id,
            role: new_message.role.clone(),
            encrypted_content,
            tool_name: new_message.tool_name.clone(),
            tool_call_id: new_message.tool_call_id.clone(),
            tool_calls_json: new_message.tool_calls_json.clone(),
            created_at: new_message.created_at,
            conversation_id: new_message.conversation_id.clone(),
        };

        diesel::insert_into(message_history::table)
            .values(&encrypted_message)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_old_message_history(
        &self,
        user_id: i32,
        save_context_limit: i64,
    ) -> Result<usize, diesel::result::Error> {
        use crate::pg_schema::message_history;
        use diesel::prelude::*;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Start a transaction
        conn.transaction(|conn| {
            // Find the oldest timestamp to keep based on the most recent messages
            let oldest_keep_timestamp: Option<i32> = {
                let base_query = message_history::table
                    .filter(message_history::user_id.eq(user_id))
                    .filter(message_history::role.eq("user"));

                base_query
                    .order_by(message_history::created_at.desc())
                    .limit(save_context_limit)
                    .select(message_history::created_at)
                    .load::<i32>(conn)?
                    .last()
                    .cloned()
            };

            match oldest_keep_timestamp {
                Some(timestamp) => {
                    // Build delete query
                    let base_delete = diesel::delete(message_history::table)
                        .filter(message_history::user_id.eq(user_id))
                        .filter(message_history::created_at.lt(timestamp));

                    base_delete.execute(conn)
                }
                None => Ok(0),
            }
        })
    }

    pub fn set_imap_credentials(
        &self,
        user_id: i32,
        email: &str,
        password: &str,
        imap_server: Option<&str>,
        imap_port: Option<u16>,
    ) -> Result<(), diesel::result::Error> {
        use crate::pg_schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt password
        let encrypted_password =
            encrypt(password).map_err(|_| diesel::result::Error::RollbackTransaction)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // First, delete any existing connections for this user
        diesel::delete(imap_connection::table)
            .filter(imap_connection::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Create new connection
        let new_connection = NewPgImapConnection {
            user_id,
            method: imap_server
                .map(|s| s.to_string())
                .unwrap_or("gmail".to_string()),
            encrypted_password,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: email.to_string(),
            imap_server: imap_server.map(|s| s.to_string()),
            imap_port: imap_port.map(|p| p as i32),
        };

        // Insert the new connection
        diesel::insert_into(imap_connection::table)
            .values(&new_connection)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_imap_credentials(
        &self,
        user_id: i32,
    ) -> Result<Option<ImapCredentials>, diesel::result::Error> {
        use crate::pg_schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get the active IMAP connection for the user
        let imap_conn = imap_connection::table
            .filter(imap_connection::user_id.eq(user_id))
            .filter(imap_connection::status.eq("active"))
            .first::<crate::pg_models::PgImapConnection>(&mut conn)
            .optional()?;

        if let Some(conn) = imap_conn {
            // Decrypt the password
            match decrypt(&conn.encrypted_password) {
                Ok(decrypted_password) => Ok(Some((
                    conn.description,
                    decrypted_password,
                    conn.imap_server,
                    conn.imap_port,
                ))),
                Err(_) => Err(diesel::result::Error::RollbackTransaction),
            }
        } else {
            Ok(None)
        }
    }

    pub fn delete_imap_credentials(&self, user_id: i32) -> Result<(), diesel::result::Error> {
        use crate::pg_schema::imap_connection;
        let connection = &mut self.pool.get().unwrap();

        diesel::delete(imap_connection::table.filter(imap_connection::user_id.eq(user_id)))
            .execute(connection)?;

        Ok(())
    }

    // log the usage. activity_type either 'call' or 'sms', or the new 'notification'
    // NOTE: User existence verification removed - users table is in SQLite, not PG.
    // Callers should verify user existence via UserCore before calling this.
    pub fn log_usage(&self, params: LogUsageParams) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_log = NewPgUsageLog {
            user_id: params.user_id,
            sid: params.sid,
            activity_type: params.activity_type,
            credits: params.credits,
            created_at: current_time,
            time_consumed: params.time_consumed,
            success: params.success,
            reason: params.reason,
            status: params.status,
            recharge_threshold_timestamp: params.recharge_threshold_timestamp,
            zero_credits_timestamp: params.zero_credits_timestamp,
        };

        diesel::insert_into(usage_logs::table)
            .values(&new_log)
            .execute(&mut conn)?;
        Ok(())
    }

    // TODO(pg-migration): is_credits_under_threshold moved to UserCore - it queries
    // the users table which remains in SQLite. Callers should use UserCore instead.

    pub fn get_usage_data(
        &self,
        _user_id: i32,
        from_timestamp: i32,
    ) -> Result<Vec<UsageDataPoint>, DieselError> {
        // Check if we're in development mode
        if std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
            != "development"
        {
            // Generate example data for the last 30 days
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            let mut example_data = Vec::new();
            let day_in_seconds = 24 * 60 * 60;

            // Generate random usage data for each day
            for i in 0..30 {
                let timestamp = now - (i * day_in_seconds);
                if timestamp >= from_timestamp {
                    // Random usage between 50 and 500
                    let usage = rand::random::<f32>() % 451.00 + 50.00;
                    example_data.push(UsageDataPoint {
                        timestamp,
                        credits: usage,
                    });

                    // Sometimes add multiple entries per day
                    if rand::random::<f32>() > 0.7 {
                        let credit_usage = rand::random::<f32>() % 301.00 + 20.00;
                        example_data.push(UsageDataPoint {
                            timestamp: timestamp + 3600, // 1 hour later
                            credits: credit_usage,
                        });
                    }
                }
            }

            example_data.sort_by_key(|point| point.timestamp);
            println!("returning example data");
            return Ok(example_data);
        }
        println!("getting real usage data");
        use crate::pg_schema::usage_logs::dsl::*;

        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Query usage logs for the user within the time range
        let usage_data = usage_logs
            .filter(user_id.eq(user_id))
            .filter(created_at.ge(from_timestamp))
            .select((created_at, credits))
            .order_by(created_at.asc())
            .load::<(i32, Option<f32>)>(&mut conn)?
            .into_iter()
            .filter_map(|(timestamp, credit_amount)| {
                credit_amount.map(|credit_value| UsageDataPoint {
                    timestamp,
                    credits: credit_value,
                })
            })
            .collect();

        Ok(usage_data)
    }

    // Fetch the ongoing usage log for a user
    pub fn get_ongoing_usage(
        &self,
        user_id: i32,
    ) -> Result<Option<crate::pg_models::PgUsageLog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let ongoing_log = usage_logs::table
            .filter(usage_logs::user_id.eq(user_id))
            .filter(usage_logs::status.eq("ongoing"))
            .first::<crate::pg_models::PgUsageLog>(&mut conn)
            .optional()?;
        Ok(ongoing_log)
    }

    pub fn update_usage_log_fields(
        &self,
        user_id: i32,
        sid: &str,
        status: &str,
        success: bool,
        reason: &str,
        call_duration: Option<i32>,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(usage_logs::table)
            .filter(usage_logs::user_id.eq(user_id))
            .filter(usage_logs::sid.eq(sid))
            .set((
                usage_logs::success.eq(success),
                usage_logs::reason.eq(reason),
                usage_logs::status.eq(status),
                usage_logs::call_duration.eq(call_duration),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Expire stale "ongoing" call records older than the given cutoff timestamp.
    /// These are calls that never received a webhook callback.
    pub fn expire_stale_ongoing_calls(&self, cutoff_ts: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(
            usage_logs::table
                .filter(usage_logs::status.eq("ongoing"))
                .filter(usage_logs::created_at.lt(cutoff_ts)),
        )
        .set((
            usage_logs::status.eq("expired"),
            usage_logs::success.eq(false),
            usage_logs::reason.eq("No webhook callback received"),
        ))
        .execute(&mut conn)?;
        Ok(count)
    }

    pub fn get_all_usage_logs(&self) -> Result<Vec<crate::pg_models::PgUsageLog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get all usage logs ordered by creation time (newest first)
        let logs = usage_logs::table
            .order_by(usage_logs::created_at.desc())
            .load::<crate::pg_models::PgUsageLog>(&mut conn)?;

        Ok(logs)
    }

    pub fn has_recent_notification(
        &self,
        user_id: i32,
        activity_type: &str,
        seconds_ago: i32,
    ) -> Result<bool, DieselError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        let cutoff_time = now_secs - seconds_ago;

        let count: i64 = usage_logs::table
            .filter(usage_logs::user_id.eq(user_id))
            .filter(usage_logs::activity_type.eq(activity_type))
            .filter(usage_logs::created_at.gt(cutoff_time))
            .count()
            .get_result(&mut conn)?;

        Ok(count > 0)
    }

    // TODO(pg-migration): delete_old_message_status_logs moved to UserCore - it queries
    // the message_status_log table which remains in SQLite. Callers should use UserCore instead.

    // Contact Profiles methods
    pub fn create_contact_profile(
        &self,
        new_profile: &NewPgContactProfile,
    ) -> Result<PgContactProfile, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(contact_profiles::table)
            .values(new_profile)
            .execute(&mut conn)?;

        // Return the created profile
        contact_profiles::table
            .filter(contact_profiles::user_id.eq(new_profile.user_id))
            .filter(contact_profiles::nickname.eq(&new_profile.nickname))
            .order(contact_profiles::id.desc())
            .first::<PgContactProfile>(&mut conn)
    }

    pub fn get_contact_profiles(&self, user_id: i32) -> Result<Vec<PgContactProfile>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        contact_profiles::table
            .filter(contact_profiles::user_id.eq(user_id))
            .order(contact_profiles::nickname.asc())
            .load::<PgContactProfile>(&mut conn)
    }

    pub fn update_contact_profile(
        &self,
        params: UpdateContactProfileParams,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(contact_profiles::table)
            .filter(contact_profiles::user_id.eq(params.user_id))
            .filter(contact_profiles::id.eq(params.profile_id))
            .set((
                contact_profiles::nickname.eq(params.nickname),
                contact_profiles::whatsapp_chat.eq(params.whatsapp_chat),
                contact_profiles::telegram_chat.eq(params.telegram_chat),
                contact_profiles::signal_chat.eq(params.signal_chat),
                contact_profiles::email_addresses.eq(params.email_addresses),
                contact_profiles::notification_mode.eq(params.notification_mode),
                contact_profiles::notification_type.eq(params.notification_type),
                contact_profiles::notify_on_call.eq(params.notify_on_call),
                contact_profiles::whatsapp_room_id.eq(params.whatsapp_room_id),
                contact_profiles::telegram_room_id.eq(params.telegram_room_id),
                contact_profiles::signal_room_id.eq(params.signal_room_id),
                contact_profiles::notes.eq(params.notes),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Update only the room_id for a specific service on a contact profile.
    /// Used to auto-capture room_id on first name-based match from the bridge.
    pub fn update_profile_room_id(
        &self,
        profile_id: i32,
        service: &str,
        room_id: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        match service {
            "whatsapp" => {
                diesel::update(contact_profiles::table.filter(contact_profiles::id.eq(profile_id)))
                    .set(contact_profiles::whatsapp_room_id.eq(Some(room_id)))
                    .execute(&mut conn)?;
            }
            "telegram" => {
                diesel::update(contact_profiles::table.filter(contact_profiles::id.eq(profile_id)))
                    .set(contact_profiles::telegram_room_id.eq(Some(room_id)))
                    .execute(&mut conn)?;
            }
            "signal" => {
                diesel::update(contact_profiles::table.filter(contact_profiles::id.eq(profile_id)))
                    .set(contact_profiles::signal_room_id.eq(Some(room_id)))
                    .execute(&mut conn)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Find contact profiles that use any of the given room IDs.
    /// Returns a map of room_id -> nickname for rooms that are already assigned.
    pub fn find_profiles_by_room_ids(
        &self,
        user_id: i32,
        room_ids: &[String],
        exclude_profile_id: Option<i32>,
    ) -> Result<std::collections::HashMap<String, String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let mut query = contact_profiles::table
            .filter(contact_profiles::user_id.eq(user_id))
            .into_boxed();

        if let Some(excl_id) = exclude_profile_id {
            query = query.filter(contact_profiles::id.ne(excl_id));
        }

        let profiles: Vec<PgContactProfile> = query.load(&mut conn)?;

        let mut result = std::collections::HashMap::new();
        for profile in profiles {
            for rid in room_ids {
                if profile.whatsapp_room_id.as_deref() == Some(rid.as_str())
                    || profile.telegram_room_id.as_deref() == Some(rid.as_str())
                    || profile.signal_room_id.as_deref() == Some(rid.as_str())
                {
                    result.insert(rid.clone(), profile.nickname.clone());
                }
            }
        }
        Ok(result)
    }

    pub fn delete_contact_profile(&self, user_id: i32, profile_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(contact_profiles::table)
            .filter(contact_profiles::user_id.eq(user_id))
            .filter(contact_profiles::id.eq(profile_id))
            .execute(&mut conn)?;
        Ok(())
    }

    // Contact Profile Exception methods
    pub fn get_profile_exceptions(
        &self,
        profile_id: i32,
    ) -> Result<Vec<PgContactProfileException>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        contact_profile_exceptions::table
            .filter(contact_profile_exceptions::profile_id.eq(profile_id))
            .select(PgContactProfileException::as_select())
            .load::<PgContactProfileException>(&mut conn)
    }

    pub fn get_profile_exception_for_platform(
        &self,
        profile_id: i32,
        platform: &str,
    ) -> Result<Option<PgContactProfileException>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        contact_profile_exceptions::table
            .filter(contact_profile_exceptions::profile_id.eq(profile_id))
            .filter(contact_profile_exceptions::platform.eq(platform))
            .select(PgContactProfileException::as_select())
            .first::<PgContactProfileException>(&mut conn)
            .optional()
    }

    pub fn set_profile_exception(
        &self,
        profile_id: i32,
        platform: &str,
        notification_mode: &str,
        notification_type: &str,
        notify_on_call: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Try to update first, if no rows updated, insert
        let updated = diesel::update(contact_profile_exceptions::table)
            .filter(contact_profile_exceptions::profile_id.eq(profile_id))
            .filter(contact_profile_exceptions::platform.eq(platform))
            .set((
                contact_profile_exceptions::notification_mode.eq(notification_mode),
                contact_profile_exceptions::notification_type.eq(notification_type),
                contact_profile_exceptions::notify_on_call.eq(notify_on_call),
            ))
            .execute(&mut conn)?;

        if updated == 0 {
            // Insert new exception
            let new_exception = NewPgContactProfileException {
                profile_id,
                platform: platform.to_string(),
                notification_mode: notification_mode.to_string(),
                notification_type: notification_type.to_string(),
                notify_on_call,
            };
            diesel::insert_into(contact_profile_exceptions::table)
                .values(&new_exception)
                .execute(&mut conn)?;
        }
        Ok(())
    }

    pub fn delete_all_profile_exceptions(&self, profile_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(contact_profile_exceptions::table)
            .filter(contact_profile_exceptions::profile_id.eq(profile_id))
            .execute(&mut conn)?;
        Ok(())
    }

    // YouTube integration methods
    pub fn has_active_youtube(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::pg_schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = youtube::table
            .filter(youtube::user_id.eq(user_id))
            .filter(youtube::status.eq("active"))
            .first::<crate::pg_models::PgYouTube>(&mut conn)
            .optional()?;

        Ok(connection.is_some())
    }

    pub fn get_youtube_tokens(
        &self,
        user_id: i32,
    ) -> Result<Option<(String, String)>, DieselError> {
        use crate::pg_schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = youtube::table
            .filter(youtube::user_id.eq(user_id))
            .filter(youtube::status.eq("active"))
            .first::<crate::pg_models::PgYouTube>(&mut conn)
            .optional()?;

        if let Some(connection) = connection {
            let access_token = match decrypt(&connection.encrypted_access_token) {
                Ok(token) => token,
                Err(e) => {
                    tracing::error!("Failed to decrypt YouTube access token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            let refresh_token = match decrypt(&connection.encrypted_refresh_token) {
                Ok(token) => token,
                Err(e) => {
                    tracing::error!("Failed to decrypt YouTube refresh token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            Ok(Some((access_token, refresh_token)))
        } else {
            Ok(None)
        }
    }

    pub fn create_youtube_connection_with_scope(
        &self,
        user_id: i32,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i32,
        scope: &str,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token =
            encrypt(access_token).map_err(|_| DieselError::RollbackTransaction)?;
        let encrypted_refresh_token =
            encrypt(refresh_token.unwrap_or("")).map_err(|_| DieselError::RollbackTransaction)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Delete any existing connection first
        diesel::delete(youtube::table)
            .filter(youtube::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Store scope in description field
        let description = format!("scope:{}", scope);

        let new_youtube = crate::pg_models::NewPgYouTube {
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            status: "active".to_string(),
            expires_in,
            last_update: current_time,
            created_on: current_time,
            description,
        };

        diesel::insert_into(youtube::table)
            .values(&new_youtube)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_youtube_scope(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        use crate::pg_schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let result = youtube::table
            .filter(youtube::user_id.eq(user_id))
            .filter(youtube::status.eq("active"))
            .select(youtube::description)
            .first::<String>(&mut conn)
            .optional()?;

        Ok(result.map(|desc| {
            // Format: "scope:write" or "scope:readonly" or legacy formats
            if desc.starts_with("scope:write") {
                "write".to_string()
            } else if desc.starts_with("scope:") {
                desc.replace("scope:", "")
            } else {
                "readonly".to_string() // Default for old connections
            }
        }))
    }

    pub fn delete_youtube_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::pg_schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(youtube::table)
            .filter(youtube::user_id.eq(user_id))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn update_youtube_access_token(
        &self,
        user_id: i32,
        new_access_token: &str,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token =
            encrypt(new_access_token).map_err(|_| DieselError::RollbackTransaction)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        diesel::update(youtube::table)
            .filter(youtube::user_id.eq(user_id))
            .filter(youtube::status.eq("active"))
            .set((
                youtube::encrypted_access_token.eq(encrypted_access_token),
                youtube::expires_in.eq(expires_in),
                youtube::last_update.eq(current_time),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_active_imap_connection_users(&self) -> Result<Vec<i32>, DieselError> {
        use crate::pg_schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user_ids = imap_connection::table
            .filter(imap_connection::status.eq("active"))
            .select(imap_connection::user_id)
            .load::<i32>(&mut conn)?;

        Ok(user_ids)
    }

    // Tesla repository methods
    pub fn has_active_tesla(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .first::<crate::pg_models::PgTesla>(&mut conn)
            .optional()?;
        Ok(connection.is_some())
    }

    pub fn create_tesla_connection(
        &self,
        new_connection: crate::pg_models::NewPgTesla,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user_id = new_connection.user_id;

        // Delete any existing connection for this user
        diesel::delete(tesla::table)
            .filter(tesla::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Insert new connection
        diesel::insert_into(tesla::table)
            .values(&new_connection)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_tesla_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(tesla::table)
            .filter(tesla::user_id.eq(user_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_tesla_token_info(
        &self,
        user_id: i32,
    ) -> Result<(String, String, i32, i32), DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .first::<crate::pg_models::PgTesla>(&mut conn)?;

        // Return encrypted tokens - let the caller decrypt them if needed
        Ok((
            connection.encrypted_access_token,
            connection.encrypted_refresh_token,
            connection.expires_in,
            connection.last_update,
        ))
    }

    pub fn update_tesla_access_token(
        &self,
        user_id: i32,
        encrypted_access_token: String,
        encrypted_refresh_token: String,
        expires_in: i32,
        last_update: i32,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(tesla::table)
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .set((
                tesla::encrypted_access_token.eq(encrypted_access_token),
                tesla::encrypted_refresh_token.eq(encrypted_refresh_token),
                tesla::expires_in.eq(expires_in),
                tesla::last_update.eq(last_update),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_tesla_region(&self, user_id: i32) -> Result<String, DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .select(tesla::region)
            .first::<String>(&mut conn)?;

        Ok(connection)
    }

    pub fn get_selected_vehicle_vin(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let vehicle_vin = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .select(tesla::selected_vehicle_vin)
            .first::<Option<String>>(&mut conn)?;

        Ok(vehicle_vin)
    }

    pub fn set_selected_vehicle(
        &self,
        user_id: i32,
        vin: String,
        name: String,
        vehicle_id: String,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(tesla::table.filter(tesla::user_id.eq(user_id)))
            .set((
                tesla::selected_vehicle_vin.eq(Some(vin)),
                tesla::selected_vehicle_name.eq(Some(name)),
                tesla::selected_vehicle_id.eq(Some(vehicle_id)),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_selected_vehicle_info(
        &self,
        user_id: i32,
    ) -> Result<Option<(String, String, String)>, DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let result = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .select((
                tesla::selected_vehicle_vin,
                tesla::selected_vehicle_name,
                tesla::selected_vehicle_id,
            ))
            .first::<(Option<String>, Option<String>, Option<String>)>(&mut conn)?;

        match result {
            (Some(vin), Some(name), Some(id)) => Ok(Some((vin, name, id))),
            _ => Ok(None),
        }
    }

    pub fn mark_tesla_key_paired(&self, user_id: i32, paired: bool) -> Result<(), DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let paired_value = if paired { 1 } else { 0 };

        diesel::update(tesla::table.filter(tesla::user_id.eq(user_id)))
            .set(tesla::virtual_key_paired.eq(paired_value))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_tesla_key_paired_status(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let paired = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .select(tesla::virtual_key_paired)
            .first::<i32>(&mut conn)?;

        Ok(paired == 1)
    }

    /// Get the granted scopes for a user's Tesla connection
    pub fn get_tesla_granted_scopes(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let scopes = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .select(tesla::granted_scopes)
            .first::<Option<String>>(&mut conn)?;

        Ok(scopes)
    }

    /// Update the granted scopes for a user's Tesla connection
    pub fn update_tesla_granted_scopes(
        &self,
        user_id: i32,
        scopes: String,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(tesla::table)
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .set(tesla::granted_scopes.eq(Some(scopes)))
            .execute(&mut conn)?;

        Ok(())
    }

    // TODO(pg-migration): The following Matrix credential methods were moved to UserCore
    // because they query the users table which remains in SQLite:
    // - set_matrix_credentials
    // - set_matrix_device_id_and_access_token
    // - clear_matrix_credentials
    // - set_matrix_e2ee_enabled
    // - set_matrix_secret_storage_recovery_key

    pub fn update_bridge_last_seen_online(
        &self,
        user_id: i32,
        service_type: &str,
        last_seen_online: i32,
    ) -> Result<usize, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let rows = diesel::update(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service_type))
            .set(bridges::last_seen_online.eq(Some(last_seen_online)))
            .execute(&mut conn)?;
        Ok(rows)
    }

    /// Update bridge data field (e.g., connected account/phone number)
    pub fn update_bridge_data(
        &self,
        user_id: i32,
        service_type: &str,
        data: &str,
    ) -> Result<usize, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let rows = diesel::update(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service_type))
            .set(bridges::data.eq(Some(data)))
            .execute(&mut conn)?;
        Ok(rows)
    }

    /// Update bridge status (e.g., "connected", "connecting", "cleaning_up")
    pub fn update_bridge_status(
        &self,
        user_id: i32,
        service_type: &str,
        status: &str,
    ) -> Result<usize, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let rows = diesel::update(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service_type))
            .set(bridges::status.eq(status))
            .execute(&mut conn)?;
        Ok(rows)
    }

    // NOTE(pg-migration): The "mark user as migrated" step was removed because the
    // users table is in SQLite. Callers should update migrated_to_new_server via UserCore.
    pub fn create_bridge(&self, new_bridge: NewPgBridge) -> Result<(), DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(bridges::table)
            .values(&new_bridge)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_bridge(&self, user_id: i32, service: &str) -> Result<(), DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_bridge(&self, user_id: i32, service: &str) -> Result<Option<PgBridge>, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service))
            .first::<PgBridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }

    pub fn get_first_connected_bridge(
        &self,
        user_id: i32,
    ) -> Result<Option<PgBridge>, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::status.eq("connected"))
            .order(bridges::id.asc())
            .first::<PgBridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }

    pub fn has_active_bridges(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::status.eq("connected"))
            .count()
            .get_result::<i64>(&mut conn)?;
        Ok(count > 0)
    }

    pub fn get_users_with_active_bridges(
        &self,
    ) -> Result<std::collections::HashMap<i32, Vec<PgBridge>>, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let all_bridges: Vec<PgBridge> = bridges::table
            .filter(bridges::status.eq("connected"))
            .load(&mut conn)?;

        let mut result: std::collections::HashMap<i32, Vec<PgBridge>> =
            std::collections::HashMap::new();
        for bridge in all_bridges {
            result.entry(bridge.user_id).or_default().push(bridge);
        }
        Ok(result)
    }

    pub fn get_users_with_matrix_bridge_connections(&self) -> Result<Vec<i32>, DieselError> {
        use crate::pg_schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get distinct user_ids that have at least one bridge connection
        let user_ids = bridges::table
            .select(bridges::user_id)
            .distinct()
            .load::<i32>(&mut conn)?;

        Ok(user_ids)
    }

    /// Check if user credits are below their auto-charge threshold.
    /// Returns true when `charge_when_under` is enabled and credits are at or
    /// below the `charge_back_to` target (meaning the user should be auto-charged).
    /// Queries SQLite `users` table.
    pub fn is_credits_under_threshold(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::users;
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        let (credits, charge_enabled, charge_back_to): (f32, bool, Option<f32>) = users::table
            .find(user_id)
            .select((
                users::credits,
                users::charge_when_under,
                users::charge_back_to,
            ))
            .first(&mut conn)?;
        if !charge_enabled {
            return Ok(false);
        }
        let threshold = charge_back_to.unwrap_or(5.0);
        Ok(credits < threshold)
    }

    /// Delete old message status logs older than a given timestamp.
    /// Queries SQLite `message_status_log` table.
    pub fn delete_old_message_status_logs(
        &self,
        before_timestamp: i32,
    ) -> Result<usize, DieselError> {
        use crate::schema::message_status_log;
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::delete(
            message_status_log::table.filter(message_status_log::created_at.lt(before_timestamp)),
        )
        .execute(&mut conn)
    }

    /// Store or update Matrix credentials in PG `user_secrets` table.
    /// Upserts matrix_username, encrypted_matrix_access_token, matrix_device_id,
    /// and encrypted_matrix_password.
    pub fn set_matrix_credentials(
        &self,
        user_id: i32,
        matrix_username: &str,
        access_token: &str,
        device_id: &str,
        password: &str,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::user_secrets;
        let mut conn = self.pool.get().expect("Failed to get PG connection");

        let encrypted_access_token =
            encrypt(access_token).map_err(|_| DieselError::RollbackTransaction)?;
        let encrypted_password = encrypt(password).map_err(|_| DieselError::RollbackTransaction)?;

        // Try update first
        let rows = diesel::update(user_secrets::table.filter(user_secrets::user_id.eq(user_id)))
            .set((
                user_secrets::matrix_username.eq(Some(matrix_username)),
                user_secrets::encrypted_matrix_access_token.eq(Some(&encrypted_access_token)),
                user_secrets::matrix_device_id.eq(Some(device_id)),
                user_secrets::encrypted_matrix_password.eq(Some(&encrypted_password)),
            ))
            .execute(&mut conn)?;

        if rows == 0 {
            // Insert new row
            diesel::insert_into(user_secrets::table)
                .values(&crate::pg_models::NewUserSecrets {
                    user_id,
                    matrix_username: Some(matrix_username.to_string()),
                    matrix_device_id: Some(device_id.to_string()),
                    encrypted_matrix_access_token: Some(encrypted_access_token.clone()),
                    encrypted_matrix_password: Some(encrypted_password.clone()),
                    encrypted_matrix_secret_storage_recovery_key: None,
                    encrypted_twilio_account_sid: None,
                    encrypted_twilio_auth_token: None,
                })
                .execute(&mut conn)?;
        }

        // Also update the SQLite users table so existing reads still work
        {
            use crate::schema::users;
            let mut sqlite_conn = self.db_pool.get().expect("Failed to get SQLite connection");
            diesel::update(users::table.find(user_id))
                .set((
                    users::matrix_username.eq(Some(matrix_username)),
                    users::matrix_device_id.eq(Some(device_id)),
                    users::encrypted_matrix_access_token.eq(Some(&encrypted_access_token)),
                    users::encrypted_matrix_password.eq(Some(&encrypted_password)),
                ))
                .execute(&mut sqlite_conn)?;
        }

        Ok(())
    }

    /// Update Matrix device_id and access_token in PG `user_secrets` table.
    pub fn set_matrix_device_id_and_access_token(
        &self,
        user_id: i32,
        access_token: &str,
        device_id: &str,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::user_secrets;
        let mut conn = self.pool.get().expect("Failed to get PG connection");

        let encrypted_access_token =
            encrypt(access_token).map_err(|_| DieselError::RollbackTransaction)?;

        let rows = diesel::update(user_secrets::table.filter(user_secrets::user_id.eq(user_id)))
            .set((
                user_secrets::matrix_device_id.eq(Some(device_id)),
                user_secrets::encrypted_matrix_access_token.eq(Some(&encrypted_access_token)),
            ))
            .execute(&mut conn)?;

        if rows == 0 {
            diesel::insert_into(user_secrets::table)
                .values(&crate::pg_models::NewUserSecrets {
                    user_id,
                    matrix_username: None,
                    matrix_device_id: Some(device_id.to_string()),
                    encrypted_matrix_access_token: Some(encrypted_access_token.clone()),
                    encrypted_matrix_password: None,
                    encrypted_matrix_secret_storage_recovery_key: None,
                    encrypted_twilio_account_sid: None,
                    encrypted_twilio_auth_token: None,
                })
                .execute(&mut conn)?;
        }

        // Also update SQLite
        {
            use crate::schema::users;
            let mut sqlite_conn = self.db_pool.get().expect("Failed to get SQLite connection");
            diesel::update(users::table.find(user_id))
                .set((
                    users::matrix_device_id.eq(Some(device_id)),
                    users::encrypted_matrix_access_token.eq(Some(&encrypted_access_token)),
                ))
                .execute(&mut sqlite_conn)?;
        }

        Ok(())
    }

    /// Clear all Matrix credentials in PG `user_secrets` table.
    pub fn clear_matrix_credentials(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::pg_schema::user_secrets;
        let mut conn = self.pool.get().expect("Failed to get PG connection");

        diesel::update(user_secrets::table.filter(user_secrets::user_id.eq(user_id)))
            .set((
                user_secrets::matrix_username.eq(None::<String>),
                user_secrets::matrix_device_id.eq(None::<String>),
                user_secrets::encrypted_matrix_access_token.eq(None::<String>),
                user_secrets::encrypted_matrix_password.eq(None::<String>),
                user_secrets::encrypted_matrix_secret_storage_recovery_key.eq(None::<String>),
            ))
            .execute(&mut conn)?;

        // Also clear in SQLite
        {
            use crate::schema::users;
            let mut sqlite_conn = self.db_pool.get().expect("Failed to get SQLite connection");
            diesel::update(users::table.find(user_id))
                .set((
                    users::matrix_username.eq(None::<String>),
                    users::matrix_device_id.eq(None::<String>),
                    users::encrypted_matrix_access_token.eq(None::<String>),
                    users::encrypted_matrix_password.eq(None::<String>),
                    users::encrypted_matrix_secret_storage_recovery_key.eq(None::<String>),
                ))
                .execute(&mut sqlite_conn)?;
        }

        Ok(())
    }

    /// Set the Matrix secret storage recovery key in PG `user_secrets` table.
    pub fn set_matrix_secret_storage_recovery_key(
        &self,
        user_id: i32,
        recovery_key: &str,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::user_secrets;
        let mut conn = self.pool.get().expect("Failed to get PG connection");

        let encrypted_key = encrypt(recovery_key).map_err(|_| DieselError::RollbackTransaction)?;

        let rows = diesel::update(user_secrets::table.filter(user_secrets::user_id.eq(user_id)))
            .set(
                user_secrets::encrypted_matrix_secret_storage_recovery_key.eq(Some(&encrypted_key)),
            )
            .execute(&mut conn)?;

        if rows == 0 {
            diesel::insert_into(user_secrets::table)
                .values(&crate::pg_models::NewUserSecrets {
                    user_id,
                    matrix_username: None,
                    matrix_device_id: None,
                    encrypted_matrix_access_token: None,
                    encrypted_matrix_password: None,
                    encrypted_matrix_secret_storage_recovery_key: Some(encrypted_key.clone()),
                    encrypted_twilio_account_sid: None,
                    encrypted_twilio_auth_token: None,
                })
                .execute(&mut conn)?;
        }

        // Also update SQLite
        {
            use crate::schema::users;
            let mut sqlite_conn = self.db_pool.get().expect("Failed to get SQLite connection");
            diesel::update(users::table.find(user_id))
                .set(users::encrypted_matrix_secret_storage_recovery_key.eq(Some(&encrypted_key)))
                .execute(&mut sqlite_conn)?;
        }

        Ok(())
    }

    /// Set matrix_e2ee_enabled flag in SQLite `users` table.
    pub fn set_matrix_e2ee_enabled(&self, user_id: i32, enabled: bool) -> Result<(), DieselError> {
        use crate::schema::users;
        let mut conn = self.db_pool.get().expect("Failed to get SQLite connection");
        diesel::update(users::table.find(user_id))
            .set(users::matrix_e2ee_enabled.eq(enabled))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Look up a user by their Matrix username.
    /// Queries PG `user_secrets` for the user_id, then SQLite `users` for the full User.
    pub fn get_user_by_matrix_user_id(
        &self,
        matrix_user_id: &str,
    ) -> Result<Option<crate::models::user_models::User>, DieselError> {
        use crate::pg_schema::user_secrets;
        let mut pg_conn = self.pool.get().expect("Failed to get PG connection");

        let maybe_uid: Option<i32> = user_secrets::table
            .filter(user_secrets::matrix_username.eq(matrix_user_id))
            .select(user_secrets::user_id)
            .first(&mut pg_conn)
            .optional()?;

        match maybe_uid {
            Some(uid) => {
                use crate::schema::users;
                let mut sqlite_conn = self.db_pool.get().expect("Failed to get SQLite connection");
                let user = users::table
                    .find(uid)
                    .first::<crate::models::user_models::User>(&mut sqlite_conn)
                    .optional()?;
                Ok(user)
            }
            None => Ok(None),
        }
    }

    // Bridge disconnection event methods
    pub fn record_bridge_disconnection(
        &self,
        user_id: i32,
        bridge_type: &str,
        detected_at: i32,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::bridge_disconnection_events;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let new_event = crate::pg_models::NewPgBridgeDisconnectionEvent {
            user_id,
            bridge_type: bridge_type.to_string(),
            detected_at,
        };

        diesel::insert_into(bridge_disconnection_events::table)
            .values(&new_event)
            .execute(&mut conn)?;

        Ok(())
    }

    // Mark an email as processed
    pub fn mark_email_as_processed(
        &self,
        user_id: i32,
        email_uid: &str,
    ) -> Result<(), DieselError> {
        use crate::pg_schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // First check if the email is already processed
        let already_processed = self.is_email_processed(user_id, email_uid)?;
        if already_processed {
            tracing::debug!(
                "Email {} for user {} is already marked as processed",
                email_uid,
                user_id
            );
            return Ok(());
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_processed_email = crate::pg_models::NewPgProcessedEmail {
            user_id,
            email_uid: email_uid.to_string(),
            processed_at: current_time,
        };

        match diesel::insert_into(processed_emails::table)
            .values(&new_processed_email)
            .execute(&mut conn)
        {
            Ok(_) => {
                tracing::debug!(
                    "Successfully marked email {} as processed for user {}",
                    email_uid,
                    user_id
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "Failed to mark email {} as processed for user {}: {}",
                    email_uid,
                    user_id,
                    e
                );
                Err(e)
            }
        }
    }

    // Check if an email is processed
    pub fn is_email_processed(&self, user_id: i32, email_uid: &str) -> Result<bool, DieselError> {
        use crate::pg_schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let processed = processed_emails::table
            .filter(processed_emails::user_id.eq(user_id))
            .filter(processed_emails::email_uid.eq(email_uid))
            .first::<crate::pg_models::PgProcessedEmail>(&mut conn)
            .optional()?;

        Ok(processed.is_some())
    }

    // Get all processed emails for a user
    pub fn get_processed_emails(
        &self,
        user_id: i32,
    ) -> Result<Vec<crate::pg_models::PgProcessedEmail>, DieselError> {
        use crate::pg_schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let processed = processed_emails::table
            .filter(processed_emails::user_id.eq(user_id))
            .order_by(processed_emails::processed_at.desc())
            .load::<crate::pg_models::PgProcessedEmail>(&mut conn)?;

        Ok(processed)
    }

    // Delete a single processed email record
    pub fn delete_processed_email(&self, user_id: i32, email_uid: &str) -> Result<(), DieselError> {
        use crate::pg_schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(processed_emails::table)
            .filter(processed_emails::user_id.eq(user_id))
            .filter(processed_emails::email_uid.eq(email_uid))
            .execute(&mut conn)?;

        Ok(())
    }
}
