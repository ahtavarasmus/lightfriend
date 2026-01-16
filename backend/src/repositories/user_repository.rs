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
    models::user_models::{
        Bridge, ContactProfile, ContactProfileException, Keyword, NewBridge, NewContactProfile,
        NewContactProfileException, NewGoogleCalendar, NewImapConnection, NewKeyword,
        NewPrioritySender, NewTask, NewUber, NewUsageLog, PrioritySender, Task, User,
    },
    schema::{
        contact_profile_exceptions, contact_profiles, keywords, message_status_log,
        priority_senders, tasks, usage_logs, users,
    },
    DbPool,
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
}

pub struct UserRepository {
    pub pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn get_conversation_history(
        &self,
        user_id: i32,
        limit: i64,
        include_tools: bool,
    ) -> Result<Vec<crate::models::user_models::MessageHistory>, diesel::result::Error> {
        use crate::schema::message_history;
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
                .load::<crate::models::user_models::MessageHistory>(&mut conn)?
        } else {
            message_history::table
                .filter(message_history::user_id.eq(user_id))
                .filter(message_history::role.ne("tool"))
                .filter(message_history::role.eq("user"))
                .order_by(message_history::created_at.desc())
                .limit(limit)
                .load::<crate::models::user_models::MessageHistory>(&mut conn)?
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
            .load::<crate::models::user_models::MessageHistory>(&mut conn)?;
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
        new_message: &crate::models::user_models::NewMessageHistory,
    ) -> Result<(), DieselError> {
        use crate::schema::message_history;
        use crate::utils::encryption;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Create a new message with encrypted content
        let encrypted_content =
            encryption::encrypt(&new_message.encrypted_content).map_err(|e| {
                tracing::error!("Failed to encrypt message content: {:?}", e);
                DieselError::RollbackTransaction
            })?;

        let encrypted_message = crate::models::user_models::NewMessageHistory {
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
        use crate::schema::message_history;
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
        use crate::schema::imap_connection;
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
        let new_connection = NewImapConnection {
            user_id,
            method: imap_server
                .map(|s| s.to_string())
                .unwrap_or("gmail".to_string()),
            encrypted_password,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: email.to_string(),
            expires_in: 0,
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
        use crate::schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get the active IMAP connection for the user
        let imap_conn = imap_connection::table
            .filter(imap_connection::user_id.eq(user_id))
            .filter(imap_connection::status.eq("active"))
            .first::<crate::models::user_models::ImapConnection>(&mut conn)
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
        use crate::schema::imap_connection;
        let connection = &mut self.pool.get().unwrap();

        diesel::delete(imap_connection::table.filter(imap_connection::user_id.eq(user_id)))
            .execute(connection)?;

        Ok(())
    }

    // log the usage. activity_type either 'call' or 'sms', or the new 'notification'
    pub fn log_usage(&self, params: LogUsageParams) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Verify user exists
        users::table.find(params.user_id).first::<User>(&mut conn)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_log = NewUsageLog {
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

    pub fn is_credits_under_threshold(&self, user_id: i32) -> Result<bool, DieselError> {
        let charge_back_threshold = std::env::var("CHARGE_BACK_THRESHOLD")
            .unwrap_or_else(|_| "2.0".to_string())
            .parse::<f32>()
            .unwrap_or(2.00);

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table.find(user_id).first::<User>(&mut conn)?;

        // Only trigger auto-recharge if BOTH credits AND credits_left are low
        // This prevents unnecessary charges when user still has monthly allowance
        Ok(user.credits < charge_back_threshold && user.credits_left < charge_back_threshold)
    }

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
        use crate::schema::usage_logs::dsl::*;

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
    ) -> Result<Option<crate::models::user_models::UsageLog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let ongoing_log = usage_logs::table
            .filter(usage_logs::user_id.eq(user_id))
            .filter(usage_logs::status.eq("ongoing"))
            .first::<crate::models::user_models::UsageLog>(&mut conn)
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

    pub fn get_all_usage_logs(
        &self,
    ) -> Result<Vec<crate::models::user_models::UsageLog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get all usage logs ordered by creation time (newest first)
        let logs = usage_logs::table
            .order_by(usage_logs::created_at.desc())
            .load::<crate::models::user_models::UsageLog>(&mut conn)?;

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

    // Task methods
    const MAX_ACTIVE_TASKS_PER_USER: i64 = 50;

    pub fn create_task(&self, new_task: &NewTask) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Check task limit before creating
        let count: i64 = tasks::table
            .filter(tasks::user_id.eq(new_task.user_id))
            .filter(tasks::status.eq("active"))
            .count()
            .get_result(&mut conn)?;

        if count >= Self::MAX_ACTIVE_TASKS_PER_USER {
            return Err(DieselError::DatabaseError(
                diesel::result::DatabaseErrorKind::CheckViolation,
                Box::new(format!(
                    "Task limit reached: maximum {} active tasks per user",
                    Self::MAX_ACTIVE_TASKS_PER_USER
                )),
            ));
        }

        diesel::insert_into(tasks::table)
            .values(new_task)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_user_tasks(&self, user_id: i32) -> Result<Vec<Task>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        tasks::table
            .filter(tasks::user_id.eq(user_id))
            .filter(tasks::status.eq("active"))
            .order(tasks::created_at.desc())
            .load::<Task>(&mut conn)
    }

    pub fn get_due_once_tasks(&self, now: i32) -> Result<Vec<Task>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        // Get all active once_* tasks where timestamp <= now
        tasks::table
            .filter(tasks::status.eq("active"))
            .filter(tasks::trigger.like("once_%"))
            .load::<Task>(&mut conn)
            .map(|task_list| {
                task_list
                    .into_iter()
                    .filter(|task| {
                        if let Some(ts_str) = task.trigger.strip_prefix("once_") {
                            if let Ok(ts) = ts_str.parse::<i32>() {
                                return ts <= now;
                            }
                        }
                        false
                    })
                    .collect()
            })
    }

    pub fn get_recurring_tasks_for_user(
        &self,
        user_id: i32,
        trigger_type: &str,
    ) -> Result<Vec<Task>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        tasks::table
            .filter(tasks::user_id.eq(user_id))
            .filter(tasks::status.eq("active"))
            .filter(tasks::trigger.eq(trigger_type))
            .load::<Task>(&mut conn)
    }

    pub fn update_task_status(&self, task_id: i32, status: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let completed_at = if status == "completed" {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32,
            )
        } else {
            None
        };
        diesel::update(tasks::table.filter(tasks::id.eq(task_id)))
            .set((
                tasks::status.eq(status),
                tasks::completed_at.eq(completed_at),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn cancel_task(&self, user_id: i32, task_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(
            tasks::table
                .filter(tasks::id.eq(task_id))
                .filter(tasks::user_id.eq(user_id))
                .filter(tasks::status.eq("active")),
        )
        .set(tasks::status.eq("cancelled"))
        .execute(&mut conn)?;
        Ok(count > 0)
    }

    pub fn update_task_permanence(
        &self,
        user_id: i32,
        task_id: i32,
        is_permanent: bool,
        recurrence_rule: Option<String>,
        recurrence_time: Option<String>,
    ) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(
            tasks::table
                .filter(tasks::id.eq(task_id))
                .filter(tasks::user_id.eq(user_id)),
        )
        .set((
            tasks::is_permanent.eq(if is_permanent { 1 } else { 0 }),
            tasks::recurrence_rule.eq(recurrence_rule),
            tasks::recurrence_time.eq(recurrence_time),
        ))
        .execute(&mut conn)?;
        Ok(count > 0)
    }

    /// Update task trigger for rescheduling permanent tasks
    pub fn reschedule_task(&self, task_id: i32, new_trigger: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(tasks::table.filter(tasks::id.eq(task_id)))
            .set((
                tasks::trigger.eq(new_trigger),
                tasks::status.eq("active"),
                tasks::completed_at.eq::<Option<i32>>(None),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Complete a task - if permanent with recurrence, reschedules; otherwise marks completed
    /// Returns true if task was rescheduled, false if completed normally
    pub fn complete_or_reschedule_task(
        &self,
        task: &Task,
        user_timezone: &str,
    ) -> Result<bool, DieselError> {
        let task_id = task.id.ok_or(DieselError::NotFound)?;

        // Check if task is permanent with recurrence settings
        let is_permanent = task.is_permanent.unwrap_or(0) == 1;
        let has_recurrence = task.recurrence_rule.is_some() && task.recurrence_time.is_some();

        if is_permanent && has_recurrence && task.trigger.starts_with("once_") {
            // Calculate next occurrence
            if let Some(new_trigger) = self.calculate_next_trigger(task, user_timezone) {
                tracing::debug!(
                    "Rescheduling permanent task {} with new trigger: {}",
                    task_id,
                    new_trigger
                );
                self.reschedule_task(task_id, &new_trigger)?;
                return Ok(true);
            }
        }

        // Default: mark as completed
        self.update_task_status(task_id, "completed")?;
        Ok(false)
    }

    /// Calculate the next trigger timestamp based on recurrence rule and time
    fn calculate_next_trigger(&self, task: &Task, user_timezone: &str) -> Option<String> {
        use chrono::{Datelike, Duration, TimeZone};

        let recurrence_rule = task.recurrence_rule.as_ref()?;
        let recurrence_time = task.recurrence_time.as_ref()?;

        // Parse timezone
        let tz: chrono_tz::Tz = user_timezone.parse().unwrap_or(chrono_tz::UTC);

        // Parse time (HH:MM)
        let parts: Vec<&str> = recurrence_time.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let target_hour: u32 = parts[0].parse().ok()?;
        let target_minute: u32 = parts[1].parse().ok()?;

        // Get current time in user's timezone
        let now = chrono::Utc::now().with_timezone(&tz);

        let next_date = if recurrence_rule == "daily" {
            // Tomorrow at the specified time

            now.date_naive() + Duration::days(1)
        } else if let Some(days_str) = recurrence_rule.strip_prefix("weekly:") {
            // Parse days (1=Mon, 7=Sun)
            let target_days: Vec<u32> =
                days_str.split(',').filter_map(|s| s.parse().ok()).collect();

            if target_days.is_empty() {
                return None;
            }

            // Find next matching day
            let current_weekday = now.weekday().num_days_from_monday() + 1; // 1=Mon
            let mut days_ahead = 1u32;

            loop {
                let check_day = ((current_weekday + days_ahead - 1) % 7) + 1;
                if target_days.contains(&check_day) {
                    break;
                }
                days_ahead += 1;
                if days_ahead > 7 {
                    // Fallback: use first target day next week
                    days_ahead = (7 - current_weekday + target_days[0]) % 7;
                    if days_ahead == 0 {
                        days_ahead = 7;
                    }
                    break;
                }
            }

            now.date_naive() + Duration::days(days_ahead as i64)
        } else if let Some(day_str) = recurrence_rule.strip_prefix("monthly:") {
            // Parse target day of month
            let target_day: u32 = day_str.parse().ok()?;

            // Get next month's date with that day
            let current_day = now.day();

            if current_day < target_day {
                // Later this month
                now.date_naive().with_day(target_day.min(31))?
            } else {
                // Next month

                if now.month() == 12 {
                    chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, target_day.min(31))?
                } else {
                    chrono::NaiveDate::from_ymd_opt(
                        now.year(),
                        now.month() + 1,
                        target_day.min(31),
                    )?
                }
            }
        } else {
            return None;
        };

        // Combine date with target time
        let next_naive = next_date.and_hms_opt(target_hour, target_minute, 0)?;
        let next_local = tz.from_local_datetime(&next_naive).single()?;
        let next_utc = next_local.with_timezone(&chrono::Utc);

        Some(format!("once_{}", next_utc.timestamp()))
    }

    pub fn delete_old_tasks(&self, before_timestamp: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(
            tasks::table
                .filter(tasks::status.ne("active"))
                .filter(tasks::completed_at.lt(before_timestamp)),
        )
        .execute(&mut conn)
    }

    /// Delete old message status logs (for database cleanup)
    /// Keeps logs for diagnostics but removes old entries to prevent bloat
    pub fn delete_old_message_status_logs(&self, before_timestamp: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(
            message_status_log::table
                .filter(message_status_log::created_at.lt(before_timestamp))
        ).execute(&mut conn)
    }

    // Priority Senders methods
    pub fn create_priority_sender(
        &self,
        new_sender: &NewPrioritySender,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(priority_senders::table)
            .values(new_sender)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_priority_sender(
        &self,
        user_id: i32,
        service_type: &str,
        sender: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(priority_senders::table)
            .filter(priority_senders::user_id.eq(user_id))
            .filter(priority_senders::service_type.eq(service_type))
            .filter(priority_senders::sender.eq(sender))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_priority_senders_all(
        &self,
        user_id: i32,
    ) -> Result<Vec<PrioritySender>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        priority_senders::table
            .filter(priority_senders::user_id.eq(user_id))
            .load::<PrioritySender>(&mut conn)
    }

    pub fn get_priority_senders(
        &self,
        user_id: i32,
        service_type: &str,
    ) -> Result<Vec<PrioritySender>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        priority_senders::table
            .filter(priority_senders::user_id.eq(user_id))
            .filter(priority_senders::service_type.eq(service_type))
            .load::<PrioritySender>(&mut conn)
    }

    // Contact Profiles methods
    pub fn create_contact_profile(
        &self,
        new_profile: &NewContactProfile,
    ) -> Result<ContactProfile, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(contact_profiles::table)
            .values(new_profile)
            .execute(&mut conn)?;

        // Return the created profile
        contact_profiles::table
            .filter(contact_profiles::user_id.eq(new_profile.user_id))
            .filter(contact_profiles::nickname.eq(&new_profile.nickname))
            .order(contact_profiles::id.desc())
            .first::<ContactProfile>(&mut conn)
    }

    pub fn get_contact_profiles(&self, user_id: i32) -> Result<Vec<ContactProfile>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        contact_profiles::table
            .filter(contact_profiles::user_id.eq(user_id))
            .order(contact_profiles::nickname.asc())
            .load::<ContactProfile>(&mut conn)
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
            ))
            .execute(&mut conn)?;
        Ok(())
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
    ) -> Result<Vec<ContactProfileException>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        contact_profile_exceptions::table
            .filter(contact_profile_exceptions::profile_id.eq(profile_id))
            .select(ContactProfileException::as_select())
            .load::<ContactProfileException>(&mut conn)
    }

    pub fn get_profile_exception_for_platform(
        &self,
        profile_id: i32,
        platform: &str,
    ) -> Result<Option<ContactProfileException>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        contact_profile_exceptions::table
            .filter(contact_profile_exceptions::profile_id.eq(profile_id))
            .filter(contact_profile_exceptions::platform.eq(platform))
            .select(ContactProfileException::as_select())
            .first::<ContactProfileException>(&mut conn)
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
            let new_exception = NewContactProfileException {
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

    // Keywords methods
    pub fn create_keyword(&self, new_keyword: &NewKeyword) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(keywords::table)
            .values(new_keyword)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_keyword(
        &self,
        user_id: i32,
        service_type: &str,
        keyword: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(keywords::table)
            .filter(keywords::user_id.eq(user_id))
            .filter(keywords::service_type.eq(service_type))
            .filter(keywords::keyword.eq(keyword))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_keywords(
        &self,
        user_id: i32,
        service_type: &str,
    ) -> Result<Vec<Keyword>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        keywords::table
            .filter(keywords::user_id.eq(user_id))
            .filter(keywords::service_type.eq(service_type))
            .load::<Keyword>(&mut conn)
    }

    pub fn has_active_google_calendar(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = google_calendar::table
            .filter(google_calendar::user_id.eq(user_id))
            .filter(google_calendar::status.eq("active"))
            .first::<crate::models::user_models::GoogleCalendar>(&mut conn)
            .optional()?;

        Ok(connection.is_some())
    }
    pub fn get_google_calendar_tokens(
        &self,
        user_id: i32,
    ) -> Result<Option<(String, String)>, DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = google_calendar::table
            .filter(google_calendar::user_id.eq(user_id))
            .filter(google_calendar::status.eq("active"))
            .first::<crate::models::user_models::GoogleCalendar>(&mut conn)
            .optional()?;

        if let Some(connection) = connection {
            // Decrypt access token
            let access_token = match decrypt(&connection.encrypted_access_token) {
                Ok(token) => {
                    tracing::debug!("Successfully decrypted access token");
                    token
                }
                Err(e) => {
                    tracing::error!("Failed to decrypt access token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            // Decrypt refresh token
            let refresh_token = match decrypt(&connection.encrypted_refresh_token) {
                Ok(token) => {
                    tracing::debug!("Successfully decrypted refresh token");
                    token
                }
                Err(e) => {
                    tracing::error!("Failed to decrypt refresh token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            Ok(Some((access_token, refresh_token)))
        } else {
            tracing::info!("No active calendar connection found for user {}", user_id);
            Ok(None)
        }
    }

    pub fn update_google_calendar_access_token(
        &self,
        user_id: i32,
        new_access_token: &str,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token =
            encrypt(new_access_token).map_err(|_| DieselError::RollbackTransaction)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        diesel::update(google_calendar::table)
            .filter(google_calendar::user_id.eq(user_id))
            .filter(google_calendar::status.eq("active"))
            .set((
                google_calendar::encrypted_access_token.eq(encrypted_access_token),
                google_calendar::expires_in.eq(expires_in),
                google_calendar::last_update.eq(current_time),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_google_calendar_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(google_calendar::table)
            .filter(google_calendar::user_id.eq(user_id))
            .execute(&mut conn)?;

        Ok(())
    }

    // YouTube integration methods
    pub fn has_active_youtube(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = youtube::table
            .filter(youtube::user_id.eq(user_id))
            .filter(youtube::status.eq("active"))
            .first::<crate::models::user_models::YouTube>(&mut conn)
            .optional()?;

        Ok(connection.is_some())
    }

    pub fn get_youtube_tokens(
        &self,
        user_id: i32,
    ) -> Result<Option<(String, String)>, DieselError> {
        use crate::schema::youtube;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = youtube::table
            .filter(youtube::user_id.eq(user_id))
            .filter(youtube::status.eq("active"))
            .first::<crate::models::user_models::YouTube>(&mut conn)
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
        use crate::schema::youtube;
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

        let new_youtube = crate::models::user_models::NewYouTube {
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
        use crate::schema::youtube;
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
        use crate::schema::youtube;
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
        use crate::schema::youtube;
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
        use crate::schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user_ids = imap_connection::table
            .filter(imap_connection::status.eq("active"))
            .select(imap_connection::user_id)
            .load::<i32>(&mut conn)?;

        Ok(user_ids)
    }

    pub fn has_active_uber(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = uber::table
            .filter(uber::user_id.eq(user_id))
            .filter(uber::status.eq("active"))
            .first::<crate::models::user_models::Uber>(&mut conn)
            .optional()?;
        Ok(connection.is_some())
    }

    pub fn create_uber_connection(
        &self,
        user_id: i32,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let encrypted_access_token =
            encrypt(access_token).map_err(|_| DieselError::RollbackTransaction)?;
        let encrypted_refresh_token = refresh_token
            .map(encrypt)
            .transpose()
            .map_err(|_| DieselError::RollbackTransaction)?
            .unwrap_or_default();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        let new_connection = NewUber {
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            expires_in,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: "Uber Connection".to_string(),
        };
        // First, delete any existing connections for this user
        diesel::delete(uber::table)
            .filter(uber::user_id.eq(user_id))
            .execute(&mut conn)?;
        // Then insert the new connection
        diesel::insert_into(uber::table)
            .values(&new_connection)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_uber_tokens(&self, user_id: i32) -> Result<Option<(String, String)>, DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = uber::table
            .filter(uber::user_id.eq(user_id))
            .filter(uber::status.eq("active"))
            .first::<crate::models::user_models::Uber>(&mut conn)
            .optional()?;
        if let Some(connection) = connection {
            // Decrypt access token
            let access_token = match decrypt(&connection.encrypted_access_token) {
                Ok(token) => {
                    tracing::debug!("Successfully decrypted access token");
                    token
                }
                Err(e) => {
                    tracing::error!("Failed to decrypt access token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };
            // Decrypt refresh token
            let refresh_token = match decrypt(&connection.encrypted_refresh_token) {
                Ok(token) => {
                    tracing::debug!("Successfully decrypted refresh token");
                    token
                }
                Err(e) => {
                    tracing::error!("Failed to decrypt refresh token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };
            Ok(Some((access_token, refresh_token)))
        } else {
            tracing::info!("No active Uber connection found for user {}", user_id);
            Ok(None)
        }
    }

    pub fn delete_uber_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(uber::table)
            .filter(uber::user_id.eq(user_id))
            .execute(&mut conn)?;
        Ok(())
    }

    // Tesla repository methods
    pub fn has_active_tesla(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .first::<crate::models::user_models::Tesla>(&mut conn)
            .optional()?;
        Ok(connection.is_some())
    }

    pub fn create_tesla_connection(
        &self,
        new_connection: crate::models::user_models::NewTesla,
    ) -> Result<(), DieselError> {
        use crate::schema::tesla;
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
        use crate::schema::tesla;
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
        use crate::schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .first::<crate::models::user_models::Tesla>(&mut conn)?;

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
        use crate::schema::tesla;
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
        use crate::schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = tesla::table
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .select(tesla::region)
            .first::<String>(&mut conn)?;

        Ok(connection)
    }

    pub fn get_selected_vehicle_vin(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        use crate::schema::tesla;
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
        use crate::schema::tesla;
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
        use crate::schema::tesla;
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
        use crate::schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let paired_value = if paired { 1 } else { 0 };

        diesel::update(tesla::table.filter(tesla::user_id.eq(user_id)))
            .set(tesla::virtual_key_paired.eq(paired_value))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_tesla_key_paired_status(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::tesla;
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
        use crate::schema::tesla;
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
        use crate::schema::tesla;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(tesla::table)
            .filter(tesla::user_id.eq(user_id))
            .filter(tesla::status.eq("active"))
            .set(tesla::granted_scopes.eq(Some(scopes)))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn set_matrix_credentials(
        &self,
        user_id: i32,
        username: &str,
        access_token: &str,
        device_id: &str,
        password: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt the access token before storing
        let encrypted_token = crate::utils::encryption::encrypt(access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;

        // Encrypt the password before storing
        let encrypted_password = crate::utils::encryption::encrypt(password)
            .map_err(|_| DieselError::RollbackTransaction)?;

        diesel::update(users::table.find(user_id))
            .set((
                users::matrix_username.eq(username),
                users::encrypted_matrix_access_token.eq(encrypted_token),
                users::matrix_device_id.eq(device_id),
                users::encrypted_matrix_password.eq(encrypted_password),
            ))
            .execute(&mut conn)?;

        Ok(())
    }
    pub fn set_matrix_device_id_and_access_token(
        &self,
        user_id: i32,
        access_token: &str,
        device_id: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt the access token before storing
        let encrypted_token = crate::utils::encryption::encrypt(access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;

        diesel::update(users::table.find(user_id))
            .set((
                users::encrypted_matrix_access_token.eq(encrypted_token),
                users::matrix_device_id.eq(device_id),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    /// Clear all Matrix credentials for a user - used when Matrix auth fails and user needs to re-register
    pub fn clear_matrix_credentials(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(users::table.find(user_id))
            .set((
                users::matrix_username.eq(None::<String>),
                users::encrypted_matrix_access_token.eq(None::<String>),
                users::matrix_device_id.eq(None::<String>),
                users::encrypted_matrix_password.eq(None::<String>),
                users::encrypted_matrix_secret_storage_recovery_key.eq(None::<String>),
            ))
            .execute(&mut conn)?;

        tracing::info!("Cleared Matrix credentials for user {}", user_id);
        Ok(())
    }

    pub fn update_bridge_last_seen_online(
        &self,
        user_id: i32,
        service_type: &str,
        last_seen_online: i32,
    ) -> Result<usize, DieselError> {
        use crate::schema::bridges;
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
        use crate::schema::bridges;
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
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let rows = diesel::update(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service_type))
            .set(bridges::status.eq(status))
            .execute(&mut conn)?;
        Ok(rows)
    }

    pub fn create_bridge(&self, new_bridge: NewBridge) -> Result<(), DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(bridges::table)
            .values(&new_bridge)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_bridge(&self, user_id: i32, service: &str) -> Result<(), DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_bridge(&self, user_id: i32, service: &str) -> Result<Option<Bridge>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service))
            .first::<Bridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }

    pub fn has_active_bridges(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::bridges;
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
    ) -> Result<std::collections::HashMap<i32, Vec<Bridge>>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let all_bridges: Vec<Bridge> = bridges::table
            .filter(bridges::status.eq("connected"))
            .load(&mut conn)?;

        let mut result: std::collections::HashMap<i32, Vec<Bridge>> =
            std::collections::HashMap::new();
        for bridge in all_bridges {
            result.entry(bridge.user_id).or_default().push(bridge);
        }
        Ok(result)
    }

    pub fn get_users_with_matrix_bridge_connections(&self) -> Result<Vec<i32>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get distinct user_ids that have at least one bridge connection
        let user_ids = bridges::table
            .select(bridges::user_id)
            .distinct()
            .load::<i32>(&mut conn)?;

        Ok(user_ids)
    }

    pub fn get_user_by_matrix_user_id(
        &self,
        matrix_user_id: &str,
    ) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user = users::table
            .filter(users::matrix_username.eq(matrix_user_id))
            .first::<User>(&mut conn)
            .optional()?;

        Ok(user)
    }

    // Bridge disconnection event methods
    pub fn record_bridge_disconnection(
        &self,
        user_id: i32,
        bridge_type: &str,
        detected_at: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::bridge_disconnection_events;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let new_event = crate::models::user_models::NewBridgeDisconnectionEvent {
            user_id,
            bridge_type: bridge_type.to_string(),
            detected_at,
        };

        diesel::insert_into(bridge_disconnection_events::table)
            .values(&new_event)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_pending_disconnection_events(
        &self,
        user_id: i32,
    ) -> Result<Vec<crate::models::user_models::BridgeDisconnectionEvent>, DieselError> {
        use crate::schema::bridge_disconnection_events;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let events = bridge_disconnection_events::table
            .filter(bridge_disconnection_events::user_id.eq(user_id))
            .load::<crate::models::user_models::BridgeDisconnectionEvent>(&mut conn)?;

        Ok(events)
    }

    pub fn delete_disconnection_events(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::bridge_disconnection_events;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(
            bridge_disconnection_events::table
                .filter(bridge_disconnection_events::user_id.eq(user_id)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    // Mark an email as processed
    pub fn mark_email_as_processed(
        &self,
        user_id: i32,
        email_uid: &str,
    ) -> Result<(), DieselError> {
        use crate::schema::processed_emails;
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

        let new_processed_email = crate::models::user_models::NewProcessedEmail {
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
        use crate::schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let processed = processed_emails::table
            .filter(processed_emails::user_id.eq(user_id))
            .filter(processed_emails::email_uid.eq(email_uid))
            .first::<crate::models::user_models::ProcessedEmail>(&mut conn)
            .optional()?;

        Ok(processed.is_some())
    }

    // Get all processed emails for a user
    pub fn get_processed_emails(
        &self,
        user_id: i32,
    ) -> Result<Vec<crate::models::user_models::ProcessedEmail>, DieselError> {
        use crate::schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let processed = processed_emails::table
            .filter(processed_emails::user_id.eq(user_id))
            .order_by(processed_emails::processed_at.desc())
            .load::<crate::models::user_models::ProcessedEmail>(&mut conn)?;

        Ok(processed)
    }

    // Delete a single processed email record
    pub fn delete_processed_email(&self, user_id: i32, email_uid: &str) -> Result<(), DieselError> {
        use crate::schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(processed_emails::table)
            .filter(processed_emails::user_id.eq(user_id))
            .filter(processed_emails::email_uid.eq(email_uid))
            .execute(&mut conn)?;

        Ok(())
    }

    // Delete email judgments older than 30 days
    pub fn delete_old_email_judgments(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::email_judgments;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Calculate timestamp for 30 days ago
        let thirty_days_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32
            - (30 * 24 * 60 * 60); // 30 days in seconds

        diesel::delete(email_judgments::table)
            .filter(email_judgments::user_id.eq(user_id))
            .filter(email_judgments::processed_at.lt(thirty_days_ago))
            .execute(&mut conn)?;

        Ok(())
    }

    // Get all email judgments for a specific user
    pub fn get_user_email_judgments(
        &self,
        user_id: i32,
    ) -> Result<Vec<crate::models::user_models::EmailJudgment>, DieselError> {
        use crate::schema::email_judgments;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let judgments = email_judgments::table
            .filter(email_judgments::user_id.eq(user_id))
            .order_by(email_judgments::processed_at.desc())
            .load::<crate::models::user_models::EmailJudgment>(&mut conn)?;

        Ok(judgments)
    }

    // Clean up old calendar notifications
    pub fn cleanup_old_calendar_notifications(
        &self,
        older_than_timestamp: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::calendar_notifications;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(calendar_notifications::table)
            .filter(calendar_notifications::notification_time.lt(older_than_timestamp))
            .execute(&mut conn)?;

        Ok(())
    }

    // Create a new calendar notification
    pub fn create_calendar_notification(
        &self,
        new_notification: &crate::models::user_models::NewCalendarNotification,
    ) -> Result<(), DieselError> {
        use crate::schema::calendar_notifications;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(calendar_notifications::table)
            .values(new_notification)
            .execute(&mut conn)?;

        Ok(())
    }

    // Check if a calendar notification exists
    pub fn check_calendar_notification_exists(
        &self,
        user_id: i32,
        event_id: &str,
    ) -> Result<bool, DieselError> {
        use crate::schema::calendar_notifications;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let count = calendar_notifications::table
            .filter(calendar_notifications::user_id.eq(user_id))
            .filter(calendar_notifications::event_id.eq(event_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(count > 0)
    }

    pub fn create_google_calendar_connection(
        &self,
        user_id: i32,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token =
            encrypt(access_token).map_err(|_| DieselError::RollbackTransaction)?;
        let encrypted_refresh_token = refresh_token
            .map(encrypt)
            .transpose()
            .map_err(|_| DieselError::RollbackTransaction)?
            .unwrap_or_default();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_connection = NewGoogleCalendar {
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            expires_in,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: "Google Calendar Connection".to_string(),
        };

        // First, delete any existing connections for this user
        diesel::delete(google_calendar::table)
            .filter(google_calendar::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Then insert the new connection
        diesel::insert_into(google_calendar::table)
            .values(&new_connection)
            .execute(&mut conn)?;

        println!("Successfully created google calendar connection");
        Ok(())
    }
}
