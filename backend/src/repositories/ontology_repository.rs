use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::Text;

use crate::models::ontology_models::{
    NewOntChangelog, NewOntChannel, NewOntEvent, NewOntLink, NewOntMessage, NewOntPerson,
    NewOntPersonEdit, NewOntRule, OntChangelog, OntChannel, OntEvent, OntLink, OntMessage,
    OntPerson, OntPersonEdit, OntRule, PersonWithChannels,
};
use crate::pg_schema::{
    ont_changelog, ont_channels, ont_events, ont_links, ont_messages, ont_person_edits,
    ont_persons, ont_rules,
};
use crate::PgDbPool;

define_sql_function! {
    fn lower(x: Text) -> Text;
}

#[derive(diesel::QueryableByName, Debug)]
struct SuggestionCandidate {
    #[diesel(sql_type = diesel::sql_types::Text)]
    sender_name: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    room_id: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    platform: String,
    #[diesel(sql_type = diesel::sql_types::Int8)]
    msg_count: i64,
}

fn format_hour(h: usize) -> String {
    match h {
        0 => "12am".to_string(),
        1..=11 => format!("{}am", h),
        12 => "12pm".to_string(),
        13..=23 => format!("{}pm", h - 12),
        _ => format!("{}:00", h),
    }
}

/// Compute temporal anomaly score using Laplace-smoothed surprisal with confidence gating.
/// Returns (score, current_hour) where score scales with both how unusual the hour is AND
/// how much data we have. Handles wildly different message volumes correctly:
/// - 30-msg sender, hour=0: score ~3.2 (not enough data to be confident)
/// - 3000-msg sender, hour=0: score ~12.4 (genuinely anomalous)
fn compute_temporal_score(hour_buckets: &[u32; 24], total: u32, current_hour: usize) -> f64 {
    let total_f = total as f64;
    // Laplace smoothing: add 0.5 pseudocount per bucket (Jeffreys prior)
    let smoothed = (hour_buckets[current_hour] as f64 + 0.5) / (total_f + 12.0);
    let surprisal = -smoothed.log2();
    // Confidence ramps from 0 toward 1; reaches 0.5 at 30 messages
    let confidence = total_f / (total_f + 30.0);
    surprisal * confidence
}

/// Bucket timestamps into 24 hourly bins in the given timezone.
fn bucket_by_hour(timestamps: &[i32], tz_offset_secs: i32) -> [u32; 24] {
    let mut buckets = [0u32; 24];
    for &ts in timestamps {
        let local_ts = ts as i64 + tz_offset_secs as i64;
        let hour = ((local_ts % 86400 + 86400) % 86400 / 3600) as usize;
        if hour < 24 {
            buckets[hour] += 1;
        }
    }
    buckets
}

fn current_local_hour(now: i32, tz_offset_secs: i32) -> usize {
    let local_ts = now as i64 + tz_offset_secs as i64;
    ((local_ts % 86400 + 86400) % 86400 / 3600) as usize
}

/// Find active hours (top hours covering ~80% of activity) for context string.
fn active_hour_range(hour_buckets: &[u32; 24], total: u32) -> Option<(usize, usize)> {
    let mut indexed: Vec<(usize, u32)> = hour_buckets
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .map(|(h, &c)| (h, c))
        .collect();
    if indexed.len() < 2 {
        return None;
    }
    // Sort by count descending, accumulate until 80%
    indexed.sort_by(|a, b| b.1.cmp(&a.1));
    let threshold = (total as f64 * 0.8) as u32;
    let mut acc = 0u32;
    let mut active: Vec<usize> = Vec::new();
    for (h, c) in &indexed {
        acc += c;
        active.push(*h);
        if acc >= threshold {
            break;
        }
    }
    active.sort();
    Some((active[0], active[active.len() - 1]))
}

pub struct OntologyRepository {
    pub pool: PgDbPool,
}

impl OntologyRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    fn now() -> i32 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32
    }

    fn log_change(
        conn: &mut PgConnection,
        user_id: i32,
        entity_type: &str,
        entity_id: i32,
        change_type: &str,
        changed_fields: Option<String>,
        source: &str,
    ) {
        let entry = NewOntChangelog {
            user_id,
            entity_type: entity_type.to_string(),
            entity_id,
            change_type: change_type.to_string(),
            changed_fields,
            source: source.to_string(),
            created_at: Self::now(),
        };
        let _ = diesel::insert_into(ont_changelog::table)
            .values(&entry)
            .execute(conn);
    }

    // -----------------------------------------------------------------------
    // Person CRUD
    // -----------------------------------------------------------------------

    /// Find-or-create a Person + Channel. Returns the Person.
    /// Used by bridge.rs for auto-creation on first message from recognized phone contacts.
    pub fn upsert_person(
        &self,
        user_id: i32,
        name: &str,
        platform: &str,
        handle: Option<&str>,
        room_id: Option<&str>,
    ) -> Result<OntPerson, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        // Try to find existing Person by room_id on any channel first
        if let Some(rid) = room_id {
            let existing_channel: Option<OntChannel> = ont_channels::table
                .filter(ont_channels::user_id.eq(user_id))
                .filter(ont_channels::room_id.eq(rid))
                .first::<OntChannel>(&mut conn)
                .optional()?;

            if let Some(ch) = existing_channel {
                let person = ont_persons::table
                    .filter(ont_persons::id.eq(ch.person_id))
                    .first::<OntPerson>(&mut conn)?;
                return Ok(person);
            }
        }

        // Try to find by name match (case-insensitive)
        let existing_person: Option<OntPerson> = ont_persons::table
            .filter(ont_persons::user_id.eq(user_id))
            .filter(lower(ont_persons::name).eq(lower(name)))
            .first::<OntPerson>(&mut conn)
            .optional()?;

        if let Some(person) = existing_person {
            // Check if a channel for this platform already exists
            let existing_ch: Option<OntChannel> = ont_channels::table
                .filter(ont_channels::person_id.eq(person.id))
                .filter(ont_channels::platform.eq(platform))
                .first::<OntChannel>(&mut conn)
                .optional()?;

            if existing_ch.is_none() {
                // Add new channel for this platform
                let new_ch = NewOntChannel {
                    user_id,
                    person_id: person.id,
                    platform: platform.to_string(),
                    handle: handle.map(|h| h.to_string()),
                    room_id: room_id.map(|r| r.to_string()),
                    notification_mode: "default".to_string(),
                    notification_type: "sms".to_string(),
                    notify_on_call: 1,
                    created_at: now,
                };
                let ch: OntChannel = diesel::insert_into(ont_channels::table)
                    .values(&new_ch)
                    .get_result(&mut conn)?;
                Self::log_change(
                    &mut conn, user_id, "channel", ch.id, "created", None, "pipeline",
                );
            } else if let Some(ch) = existing_ch {
                // Update room_id if not set
                if ch.room_id.is_none() && room_id.is_some() {
                    diesel::update(ont_channels::table.filter(ont_channels::id.eq(ch.id)))
                        .set(ont_channels::room_id.eq(room_id))
                        .execute(&mut conn)?;
                }
            }

            return Ok(person);
        }

        // Create new Person + Channel
        let new_person = NewOntPerson {
            user_id,
            name: name.to_string(),
            created_at: now,
            updated_at: now,
        };
        let person: OntPerson = diesel::insert_into(ont_persons::table)
            .values(&new_person)
            .get_result(&mut conn)?;

        Self::log_change(
            &mut conn, user_id, "person", person.id, "created", None, "pipeline",
        );

        let new_channel = NewOntChannel {
            user_id,
            person_id: person.id,
            platform: platform.to_string(),
            handle: handle.map(|h| h.to_string()),
            room_id: room_id.map(|r| r.to_string()),
            notification_mode: "default".to_string(),
            notification_type: "sms".to_string(),
            notify_on_call: 1,
            created_at: now,
        };
        let channel: OntChannel = diesel::insert_into(ont_channels::table)
            .values(&new_channel)
            .get_result(&mut conn)?;

        Self::log_change(
            &mut conn, user_id, "channel", channel.id, "created", None, "pipeline",
        );

        Ok(person)
    }

    /// Create a Person manually (user action). Returns the new Person.
    pub fn create_person(&self, user_id: i32, name: &str) -> Result<OntPerson, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        let new_person = NewOntPerson {
            user_id,
            name: name.to_string(),
            created_at: now,
            updated_at: now,
        };
        let person: OntPerson = diesel::insert_into(ont_persons::table)
            .values(&new_person)
            .get_result(&mut conn)?;

        Self::log_change(
            &mut conn,
            user_id,
            "person",
            person.id,
            "created",
            None,
            "user_action",
        );

        Ok(person)
    }

    /// Add a channel to an existing Person.
    pub fn add_channel(
        &self,
        user_id: i32,
        person_id: i32,
        platform: &str,
        handle: Option<&str>,
        room_id: Option<&str>,
    ) -> Result<OntChannel, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        let new_ch = NewOntChannel {
            user_id,
            person_id,
            platform: platform.to_string(),
            handle: handle.map(|h| h.to_string()),
            room_id: room_id.map(|r| r.to_string()),
            notification_mode: "default".to_string(),
            notification_type: "sms".to_string(),
            notify_on_call: 1,
            created_at: now,
        };
        let channel: OntChannel = diesel::insert_into(ont_channels::table)
            .values(&new_ch)
            .get_result(&mut conn)?;

        Self::log_change(
            &mut conn,
            user_id,
            "channel",
            channel.id,
            "created",
            None,
            "user_action",
        );

        Ok(channel)
    }

    /// Get all Persons for a user.
    pub fn get_persons(&self, user_id: i32) -> Result<Vec<OntPerson>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_persons::table
            .filter(ont_persons::user_id.eq(user_id))
            .order(ont_persons::name.asc())
            .load::<OntPerson>(&mut conn)
    }

    /// Get all Persons with their Channels and edits for a user.
    pub fn get_persons_with_channels(
        &self,
        user_id: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PersonWithChannels>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let persons = ont_persons::table
            .filter(ont_persons::user_id.eq(user_id))
            .order(ont_persons::name.asc())
            .limit(limit)
            .offset(offset)
            .load::<OntPerson>(&mut conn)?;

        if persons.is_empty() {
            return Ok(Vec::new());
        }

        let person_ids: Vec<i32> = persons.iter().map(|p| p.id).collect();

        let all_channels = ont_channels::table
            .filter(ont_channels::person_id.eq_any(&person_ids))
            .load::<OntChannel>(&mut conn)?;

        let all_edits = ont_person_edits::table
            .filter(ont_person_edits::person_id.eq_any(&person_ids))
            .load::<OntPersonEdit>(&mut conn)?;

        let result = persons
            .into_iter()
            .map(|p| {
                let pid = p.id;
                let channels: Vec<OntChannel> = all_channels
                    .iter()
                    .filter(|c| c.person_id == pid)
                    .cloned()
                    .collect();
                let edits: Vec<OntPersonEdit> = all_edits
                    .iter()
                    .filter(|e| e.person_id == pid)
                    .cloned()
                    .collect();
                PersonWithChannels {
                    person: p,
                    channels,
                    edits,
                }
            })
            .collect();

        Ok(result)
    }

    /// Get a single Person with Channels and edits.
    pub fn get_person_with_channels(
        &self,
        user_id: i32,
        person_id: i32,
    ) -> Result<PersonWithChannels, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let person = ont_persons::table
            .filter(ont_persons::id.eq(person_id))
            .filter(ont_persons::user_id.eq(user_id))
            .first::<OntPerson>(&mut conn)?;

        let channels = ont_channels::table
            .filter(ont_channels::person_id.eq(person_id))
            .load::<OntChannel>(&mut conn)?;

        let edits = ont_person_edits::table
            .filter(ont_person_edits::person_id.eq(person_id))
            .load::<OntPersonEdit>(&mut conn)?;

        Ok(PersonWithChannels {
            person,
            channels,
            edits,
        })
    }

    /// Get edits for a Person.
    pub fn get_person_edits(
        &self,
        user_id: i32,
        person_id: i32,
    ) -> Result<Vec<OntPersonEdit>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_person_edits::table
            .filter(ont_person_edits::user_id.eq(user_id))
            .filter(ont_person_edits::person_id.eq(person_id))
            .load::<OntPersonEdit>(&mut conn)
    }

    /// Upsert a person edit (user override).
    pub fn set_person_edit(
        &self,
        user_id: i32,
        person_id: i32,
        property: &str,
        value: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        // Try update first
        let updated = diesel::update(ont_person_edits::table)
            .filter(ont_person_edits::user_id.eq(user_id))
            .filter(ont_person_edits::person_id.eq(person_id))
            .filter(ont_person_edits::property_name.eq(property))
            .set((
                ont_person_edits::value.eq(value),
                ont_person_edits::edited_at.eq(now),
            ))
            .execute(&mut conn)?;

        if updated == 0 {
            let new_edit = NewOntPersonEdit {
                user_id,
                person_id,
                property_name: property.to_string(),
                value: value.to_string(),
                edited_at: now,
            };
            diesel::insert_into(ont_person_edits::table)
                .values(&new_edit)
                .execute(&mut conn)?;
        }

        // Update person's updated_at
        diesel::update(ont_persons::table.filter(ont_persons::id.eq(person_id)))
            .set(ont_persons::updated_at.eq(now))
            .execute(&mut conn)?;

        Self::log_change(
            &mut conn,
            user_id,
            "person",
            person_id,
            "updated",
            Some(format!("{{\"property\":\"{}\"}}", property)),
            "user_action",
        );

        Ok(())
    }

    /// Find a Person by room_id on any of their channels.
    pub fn find_person_by_room_id(
        &self,
        user_id: i32,
        room_id: &str,
    ) -> Result<Option<PersonWithChannels>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let channel: Option<OntChannel> = ont_channels::table
            .filter(ont_channels::user_id.eq(user_id))
            .filter(ont_channels::room_id.eq(room_id))
            .first::<OntChannel>(&mut conn)
            .optional()?;

        match channel {
            Some(ch) => {
                let person = ont_persons::table
                    .filter(ont_persons::id.eq(ch.person_id))
                    .first::<OntPerson>(&mut conn)?;

                let channels = ont_channels::table
                    .filter(ont_channels::person_id.eq(person.id))
                    .load::<OntChannel>(&mut conn)?;

                let edits = ont_person_edits::table
                    .filter(ont_person_edits::person_id.eq(person.id))
                    .load::<OntPersonEdit>(&mut conn)?;

                Ok(Some(PersonWithChannels {
                    person,
                    channels,
                    edits,
                }))
            }
            None => Ok(None),
        }
    }

    /// Find a Person by name (case-insensitive, checks both base name and nickname edits).
    pub fn find_person_by_name(
        &self,
        user_id: i32,
        name: &str,
    ) -> Result<Option<PersonWithChannels>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let name_lower = name.to_lowercase();

        // First try nickname edits
        let nickname_edit: Option<OntPersonEdit> = ont_person_edits::table
            .filter(ont_person_edits::user_id.eq(user_id))
            .filter(ont_person_edits::property_name.eq("nickname"))
            .filter(diesel::dsl::sql::<diesel::sql_types::Bool>(&format!(
                "LOWER(value) = LOWER('{}')",
                name.replace('\'', "''")
            )))
            .first::<OntPersonEdit>(&mut conn)
            .optional()?;

        let person_id = if let Some(edit) = nickname_edit {
            edit.person_id
        } else {
            // Try base name
            let person: Option<OntPerson> = ont_persons::table
                .filter(ont_persons::user_id.eq(user_id))
                .filter(diesel::dsl::sql::<diesel::sql_types::Bool>(&format!(
                    "LOWER(name) = LOWER('{}')",
                    name_lower.replace('\'', "''")
                )))
                .first::<OntPerson>(&mut conn)
                .optional()?;

            match person {
                Some(p) => p.id,
                None => return Ok(None),
            }
        };

        let person = ont_persons::table
            .filter(ont_persons::id.eq(person_id))
            .first::<OntPerson>(&mut conn)?;

        let channels = ont_channels::table
            .filter(ont_channels::person_id.eq(person_id))
            .load::<OntChannel>(&mut conn)?;

        let edits = ont_person_edits::table
            .filter(ont_person_edits::person_id.eq(person_id))
            .load::<OntPersonEdit>(&mut conn)?;

        Ok(Some(PersonWithChannels {
            person,
            channels,
            edits,
        }))
    }

    /// Update a channel's room_id (auto-capture from bridge).
    pub fn update_channel_room_id(
        &self,
        channel_id: i32,
        room_id: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(ont_channels::table.filter(ont_channels::id.eq(channel_id)))
            .set(ont_channels::room_id.eq(Some(room_id)))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Update channel notification settings.
    pub fn update_channel_notification(
        &self,
        channel_id: i32,
        mode: &str,
        noti_type: &str,
        on_call: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(ont_channels::table.filter(ont_channels::id.eq(channel_id)))
            .set((
                ont_channels::notification_mode.eq(mode),
                ont_channels::notification_type.eq(noti_type),
                ont_channels::notify_on_call.eq(on_call),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Delete a Person and cascade to channels + edits.
    pub fn delete_person(&self, user_id: i32, person_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        Self::log_change(
            &mut conn,
            user_id,
            "person",
            person_id,
            "deleted",
            None,
            "user_action",
        );

        // CASCADE handles channels and edits
        diesel::delete(
            ont_persons::table
                .filter(ont_persons::id.eq(person_id))
                .filter(ont_persons::user_id.eq(user_id)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    /// Delete a specific channel.
    pub fn delete_channel(&self, user_id: i32, channel_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        Self::log_change(
            &mut conn,
            user_id,
            "channel",
            channel_id,
            "deleted",
            None,
            "user_action",
        );

        diesel::delete(
            ont_channels::table
                .filter(ont_channels::id.eq(channel_id))
                .filter(ont_channels::user_id.eq(user_id)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    /// Merge two Persons: move all channels from merge_id to keep_id, then delete merge_id.
    pub fn merge_persons(
        &self,
        user_id: i32,
        keep_id: i32,
        merge_id: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        // Move channels from merge_id to keep_id
        diesel::update(
            ont_channels::table
                .filter(ont_channels::person_id.eq(merge_id))
                .filter(ont_channels::user_id.eq(user_id)),
        )
        .set(ont_channels::person_id.eq(keep_id))
        .execute(&mut conn)?;

        // Update keep person's updated_at
        diesel::update(ont_persons::table.filter(ont_persons::id.eq(keep_id)))
            .set(ont_persons::updated_at.eq(now))
            .execute(&mut conn)?;

        Self::log_change(
            &mut conn,
            user_id,
            "person",
            keep_id,
            "merged",
            Some(format!("{{\"merged_from\":{}}}", merge_id)),
            "user_action",
        );

        // Delete the merged person (cascade removes its edits)
        diesel::delete(
            ont_persons::table
                .filter(ont_persons::id.eq(merge_id))
                .filter(ont_persons::user_id.eq(user_id)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    /// Find channels by room_ids. Returns map of room_id -> person display name.
    /// Used by search_chats to show which rooms are already assigned.
    pub fn find_channels_by_room_ids(
        &self,
        user_id: i32,
        room_ids: &[String],
        exclude_person_id: Option<i32>,
    ) -> Result<std::collections::HashMap<String, String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let mut query = ont_channels::table
            .inner_join(ont_persons::table)
            .filter(ont_channels::user_id.eq(user_id))
            .filter(ont_channels::room_id.is_not_null())
            .into_boxed();

        if let Some(excl_id) = exclude_person_id {
            query = query.filter(ont_channels::person_id.ne(excl_id));
        }

        let results: Vec<(OntChannel, OntPerson)> = query.load(&mut conn)?;

        let mut map = std::collections::HashMap::new();
        for (channel, person) in results {
            if let Some(ref rid) = channel.room_id {
                if room_ids.contains(rid) {
                    // Check for nickname edit
                    let display = ont_person_edits::table
                        .filter(ont_person_edits::person_id.eq(person.id))
                        .filter(ont_person_edits::property_name.eq("nickname"))
                        .select(ont_person_edits::value)
                        .first::<String>(&mut conn)
                        .optional()?
                        .unwrap_or(person.name.clone());
                    map.insert(rid.clone(), display);
                }
            }
        }

        Ok(map)
    }

    /// Search persons by name (partial, case-insensitive). Checks both base name and nickname edits.
    pub fn search_persons(
        &self,
        user_id: i32,
        query: &str,
    ) -> Result<Vec<PersonWithChannels>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let query_lower = query.to_lowercase();
        let like_pattern = format!("%{}%", query_lower);

        // Find persons whose name matches
        let by_name: Vec<OntPerson> = ont_persons::table
            .filter(ont_persons::user_id.eq(user_id))
            .filter(diesel::dsl::sql::<diesel::sql_types::Bool>(&format!(
                "LOWER(name) LIKE '{}'",
                like_pattern.replace('\'', "''")
            )))
            .load::<OntPerson>(&mut conn)?;

        // Find persons whose nickname edit matches
        let by_nickname: Vec<i32> = ont_person_edits::table
            .filter(ont_person_edits::user_id.eq(user_id))
            .filter(ont_person_edits::property_name.eq("nickname"))
            .filter(diesel::dsl::sql::<diesel::sql_types::Bool>(&format!(
                "LOWER(value) LIKE '{}'",
                like_pattern.replace('\'', "''")
            )))
            .select(ont_person_edits::person_id)
            .load::<i32>(&mut conn)?;

        // Combine unique person IDs
        let mut person_ids: Vec<i32> = by_name.iter().map(|p| p.id).collect();
        for pid in by_nickname {
            if !person_ids.contains(&pid) {
                person_ids.push(pid);
            }
        }

        if person_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Load full data for matched persons
        let persons = ont_persons::table
            .filter(ont_persons::id.eq_any(&person_ids))
            .load::<OntPerson>(&mut conn)?;

        let all_channels = ont_channels::table
            .filter(ont_channels::person_id.eq_any(&person_ids))
            .load::<OntChannel>(&mut conn)?;

        let all_edits = ont_person_edits::table
            .filter(ont_person_edits::person_id.eq_any(&person_ids))
            .load::<OntPersonEdit>(&mut conn)?;

        let result = persons
            .into_iter()
            .map(|p| {
                let pid = p.id;
                let channels = all_channels
                    .iter()
                    .filter(|c| c.person_id == pid)
                    .cloned()
                    .collect();
                let edits = all_edits
                    .iter()
                    .filter(|e| e.person_id == pid)
                    .cloned()
                    .collect();
                PersonWithChannels {
                    person: p,
                    channels,
                    edits,
                }
            })
            .collect();

        Ok(result)
    }

    /// Update Person's base name.
    pub fn update_person_name(
        &self,
        user_id: i32,
        person_id: i32,
        name: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        diesel::update(
            ont_persons::table
                .filter(ont_persons::id.eq(person_id))
                .filter(ont_persons::user_id.eq(user_id)),
        )
        .set((ont_persons::name.eq(name), ont_persons::updated_at.eq(now)))
        .execute(&mut conn)?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Links CRUD
    // -----------------------------------------------------------------------

    /// Create a link between two entities. Uses ON CONFLICT to avoid duplicates.
    #[allow(clippy::too_many_arguments)]
    pub fn create_link(
        &self,
        user_id: i32,
        source_type: &str,
        source_id: i32,
        target_type: &str,
        target_id: i32,
        link_type: &str,
        metadata: Option<&str>,
    ) -> Result<OntLink, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        let new_link = NewOntLink {
            user_id,
            source_type: source_type.to_string(),
            source_id,
            target_type: target_type.to_string(),
            target_id,
            link_type: link_type.to_string(),
            metadata: metadata.map(|m| m.to_string()),
            created_at: now,
        };

        let link: OntLink = diesel::insert_into(ont_links::table)
            .values(&new_link)
            .on_conflict_do_nothing()
            .get_result(&mut conn)?;

        Self::log_change(
            &mut conn,
            user_id,
            "link",
            link.id,
            "created",
            Some(format!(
                "{{\"source\":\"{}/{}\",\"target\":\"{}/{}\",\"type\":\"{}\"}}",
                source_type, source_id, target_type, target_id, link_type
            )),
            "pipeline",
        );

        Ok(link)
    }

    /// Delete a link by ID.
    pub fn delete_link(&self, user_id: i32, link_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(
            ont_links::table
                .filter(ont_links::id.eq(link_id))
                .filter(ont_links::user_id.eq(user_id)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    /// Get all links for an entity (both as source and target).
    pub fn get_links_for_entity(
        &self,
        user_id: i32,
        entity_type: &str,
        entity_id: i32,
    ) -> Result<Vec<OntLink>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let as_source: Vec<OntLink> = ont_links::table
            .filter(ont_links::user_id.eq(user_id))
            .filter(ont_links::source_type.eq(entity_type))
            .filter(ont_links::source_id.eq(entity_id))
            .load(&mut conn)?;

        let as_target: Vec<OntLink> = ont_links::table
            .filter(ont_links::user_id.eq(user_id))
            .filter(ont_links::target_type.eq(entity_type))
            .filter(ont_links::target_id.eq(entity_id))
            .load(&mut conn)?;

        let mut all = as_source;
        all.extend(as_target);
        Ok(all)
    }

    // -----------------------------------------------------------------------
    // Messages
    // -----------------------------------------------------------------------

    /// Insert a message into ont_messages, returning the created row.
    pub fn insert_message(&self, msg: &NewOntMessage) -> Result<OntMessage, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(ont_messages::table)
            .values(msg)
            .get_result(&mut conn)
    }

    /// Get messages for a specific room, ordered by created_at DESC.
    pub fn get_messages_for_room(
        &self,
        user_id: i32,
        room_id: &str,
        limit: i64,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::room_id.eq(room_id))
            .order(ont_messages::created_at.desc())
            .limit(limit)
            .load::<OntMessage>(&mut conn)
    }

    /// Get all messages linked to an event, ordered by message timestamp ASC.
    pub fn get_messages_for_event(
        &self,
        user_id: i32,
        event_id: i32,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let links = self.get_links_for_entity(user_id, "Event", event_id)?;
        let mut message_ids: Vec<i64> = links
            .into_iter()
            .filter_map(|link| {
                if link.source_type == "Message" && link.target_type == "Event" {
                    Some(link.source_id as i64)
                } else if link.source_type == "Event" && link.target_type == "Message" {
                    Some(link.target_id as i64)
                } else {
                    None
                }
            })
            .collect();

        message_ids.sort_unstable();
        message_ids.dedup();

        if message_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut messages = ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::id.eq_any(&message_ids))
            .load::<OntMessage>(&mut conn)?;

        messages.sort_by_key(|m| (m.created_at, m.id));
        Ok(messages)
    }

    /// Get recent messages for a platform since a timestamp.
    pub fn get_recent_messages(
        &self,
        user_id: i32,
        platform: &str,
        since_ts: i32,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::platform.eq(platform))
            .filter(ont_messages::created_at.gt(since_ts))
            .order(ont_messages::created_at.desc())
            .load::<OntMessage>(&mut conn)
    }

    /// Get recent messages with optional platform filter and limit.
    pub fn get_recent_messages_filtered(
        &self,
        user_id: i32,
        platform: Option<&str>,
        since_ts: i32,
        limit: i64,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let mut query = ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::created_at.gt(since_ts))
            .order(ont_messages::created_at.desc())
            .limit(limit)
            .into_boxed();

        if let Some(plat) = platform {
            query = query.filter(ont_messages::platform.eq(plat));
        }

        query.load::<OntMessage>(&mut conn)
    }

    /// Get recent messages across all platforms since a timestamp.
    pub fn get_recent_messages_all_platforms(
        &self,
        user_id: i32,
        since_ts: i32,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::created_at.gt(since_ts))
            .order(ont_messages::created_at.desc())
            .load::<OntMessage>(&mut conn)
    }

    /// Get suggestion candidates: senders with no person_id, grouped by name+room+platform.
    /// Returns (sender_name, room_id, platform, count) where count >= threshold.
    pub fn get_suggestion_candidates(
        &self,
        user_id: i32,
        threshold: i64,
    ) -> Result<Vec<(String, String, String, i64)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::sql_query(
            "SELECT sender_name, room_id, platform, COUNT(*) as msg_count \
             FROM ont_messages \
             WHERE user_id = $1 AND person_id IS NULL AND sender_name != 'You' \
             GROUP BY sender_name, room_id, platform \
             HAVING COUNT(*) >= $2 \
             ORDER BY msg_count DESC",
        )
        .bind::<diesel::sql_types::Int4, _>(user_id)
        .bind::<diesel::sql_types::Int8, _>(threshold)
        .load::<SuggestionCandidate>(&mut conn)
        .map(|rows| {
            rows.into_iter()
                .map(|r| (r.sender_name, r.room_id, r.platform, r.msg_count))
                .collect()
        })
    }

    /// Delete completed/expired rules older than max_age_secs.
    pub fn purge_old_rules(&self, max_age_secs: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cutoff = Self::now() - max_age_secs;
        diesel::delete(
            ont_rules::table
                .filter(ont_rules::status.eq_any(&["completed", "expired"]))
                .filter(ont_rules::updated_at.lt(cutoff)),
        )
        .execute(&mut conn)
    }

    /// Compute sender signals from message history for importance evaluation.
    /// Returns empty signals on any error - never fails.
    pub fn compute_sender_signals(
        &self,
        user_id: i32,
        room_id: &str,
        sender_name: &str,
        now: i32,
        tz_offset_secs: i32,
        person_id: Option<i32>,
    ) -> crate::models::ontology_models::SenderSignals {
        use crate::models::ontology_models::SenderSignals;

        let messages = match self.get_messages_for_room(user_id, room_id, 500) {
            Ok(msgs) => msgs,
            Err(_) => return SenderSignals::empty(),
        };

        let thirty_days_ago = now - 30 * 86400;

        // Messages are returned DESC, filter to last 30 days
        let recent: Vec<_> = messages
            .iter()
            .filter(|m| m.created_at >= thirty_days_ago)
            .collect();

        // Separate sender messages and user replies
        let sender_msgs: Vec<_> = recent
            .iter()
            .filter(|m| m.sender_name == sender_name)
            .collect();
        let user_msgs: Vec<_> = recent.iter().filter(|m| m.sender_name == "You").collect();

        let message_count_30d = sender_msgs.len() as i64;
        if message_count_30d == 0 {
            return SenderSignals::empty();
        }

        let user_message_count_30d = user_msgs.len() as i64;

        // Last contact: second-most-recent sender message (since newest is the current one)
        // Messages are DESC so index 1 is second-most-recent
        let last_contact_ago_secs = sender_msgs.get(1).map(|m| (now - m.created_at) as i64);

        // Reply analysis: for each sender message, check if user replied within 2 hours
        let mut replied_count = 0i64;
        let mut total_response_secs = 0i64;

        for sender_msg in &sender_msgs {
            // Find earliest "You" message after this sender message within 7200s
            if let Some(reply) = user_msgs.iter().find(|u| {
                u.created_at > sender_msg.created_at && u.created_at <= sender_msg.created_at + 7200
            }) {
                replied_count += 1;
                total_response_secs += (reply.created_at - sender_msg.created_at) as i64;
            }
        }

        let user_reply_rate = if message_count_30d > 0 {
            replied_count as f32 / message_count_30d as f32
        } else {
            0.0
        };

        let avg_response_secs = if replied_count > 0 {
            Some(total_response_secs / replied_count)
        } else {
            None
        };

        // Bidirectional ratio: how much does the user engage vs receive
        let bidirectional_ratio = if message_count_30d > 0 {
            user_message_count_30d as f32 / message_count_30d as f32
        } else {
            0.0
        };

        // Recency trend: compare last 7 days to 30-day average
        let seven_days_ago = now - 7 * 86400;
        let msgs_7d = sender_msgs
            .iter()
            .filter(|m| m.created_at >= seven_days_ago)
            .count() as f32;
        let expected_7d = (message_count_30d as f32 / 30.0) * 7.0;
        let recency_trend = if message_count_30d >= 5 && expected_7d > 0.5 {
            let ratio = msgs_7d / expected_7d;
            if ratio >= 2.0 {
                Some("Contact frequency from this person has increased significantly in the last week.".to_string())
            } else if ratio <= 0.3 && msgs_7d < 1.0 {
                Some("This person has been unusually quiet in the last week compared to their normal pattern.".to_string())
            } else {
                None
            }
        } else {
            None
        };

        // Multi-platform count: how many platforms does this person use to reach the user
        let platform_count = if let Some(pid) = person_id {
            self.get_person_platform_count(user_id, pid).unwrap_or(1)
        } else {
            1
        };

        // First contact detection
        let is_first_contact = message_count_30d <= 1 && last_contact_ago_secs.is_none();

        // Check if user has custom notification settings for this person
        let has_custom_settings = if let Some(pid) = person_id {
            self.person_has_custom_settings(user_id, pid)
        } else {
            false
        };

        // Temporal anomaly: Laplace-smoothed surprisal with confidence gating.
        let temporal_anomaly = {
            let timestamps: Vec<i32> = sender_msgs.iter().map(|m| m.created_at).collect();
            let hour_buckets = bucket_by_hour(&timestamps, tz_offset_secs);
            let current_hour = current_local_hour(now, tz_offset_secs);

            if current_hour < 24 {
                let score =
                    compute_temporal_score(&hour_buckets, message_count_30d as u32, current_hour);
                let range_ctx = active_hour_range(&hour_buckets, message_count_30d as u32)
                    .map(|(first, last)| {
                        format!(
                            " Their {} messages are typically between {}-{}.",
                            message_count_30d,
                            format_hour(first),
                            format_hour(last),
                        )
                    })
                    .unwrap_or_default();

                if score > 10.0 {
                    Some(format!(
                        "This person messaging at {} is highly unusual.{}",
                        format_hour(current_hour),
                        range_ctx,
                    ))
                } else if score > 7.0 {
                    Some(format!(
                        "This person messaging at {} is somewhat unusual.{}",
                        format_hour(current_hour),
                        range_ctx,
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        };

        SenderSignals {
            message_count_30d,
            last_contact_ago_secs,
            user_reply_rate,
            avg_response_secs,
            temporal_anomaly,
            user_message_count_30d,
            bidirectional_ratio,
            platform_count,
            recency_trend,
            is_first_contact,
            has_custom_settings,
        }
    }

    /// Count distinct platforms a person contacts the user on.
    fn get_person_platform_count(&self, user_id: i32, person_id: i32) -> Result<i32, DieselError> {
        use crate::pg_schema::ont_channels;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let channels: Vec<OntChannel> = ont_channels::table
            .filter(ont_channels::user_id.eq(user_id))
            .filter(ont_channels::person_id.eq(person_id))
            .load(&mut conn)?;
        let platforms: std::collections::HashSet<&str> =
            channels.iter().map(|c| c.platform.as_str()).collect();
        Ok(platforms.len() as i32)
    }

    /// Check if user has custom notification settings for a person.
    fn person_has_custom_settings(&self, user_id: i32, person_id: i32) -> bool {
        use crate::pg_schema::ont_person_edits;
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let count: i64 = ont_person_edits::table
            .filter(ont_person_edits::user_id.eq(user_id))
            .filter(ont_person_edits::person_id.eq(person_id))
            .filter(
                ont_person_edits::property_name
                    .eq("notification_mode")
                    .or(ont_person_edits::property_name.eq("notification_type"))
                    .or(ont_person_edits::property_name.eq("importance"))
                    .or(ont_person_edits::property_name.eq("nickname")),
            )
            .count()
            .get_result(&mut conn)
            .unwrap_or(0);
        count > 0
    }

    /// Detect if user is likely sleeping based on their sent-message activity patterns.
    /// Uses Laplace-smoothed surprisal with confidence gating - same math as sender
    /// temporal anomaly, applied to the user's own "You" messages across all rooms.
    pub fn compute_user_sleep_context(
        &self,
        user_id: i32,
        now: i32,
        tz_offset_secs: i32,
    ) -> Option<String> {
        use crate::pg_schema::ont_messages;

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let thirty_days_ago = now - 30 * 86400;

        let user_messages: Vec<OntMessage> = ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::sender_name.eq("You"))
            .filter(ont_messages::created_at.ge(thirty_days_ago))
            .select(OntMessage::as_select())
            .load(&mut conn)
            .unwrap_or_default();

        if user_messages.is_empty() {
            return None;
        }

        let timestamps: Vec<i32> = user_messages.iter().map(|m| m.created_at).collect();
        let total = timestamps.len() as u32;
        let hour_buckets = bucket_by_hour(&timestamps, tz_offset_secs);
        let current_hour = current_local_hour(now, tz_offset_secs);
        if current_hour >= 24 {
            return None;
        }

        let score = compute_temporal_score(&hour_buckets, total, current_hour);

        let range_ctx = active_hour_range(&hour_buckets, total)
            .map(|(first, last)| {
                format!(
                    "The user is typically active between {}-{}. ",
                    format_hour(first),
                    format_hour(last),
                )
            })
            .unwrap_or_default();

        if score > 10.0 {
            Some(format!(
                "{}Current time is {} - the user is almost certainly sleeping. \
                 Only notify for genuinely urgent messages where a delay until morning \
                 would cause real harm.",
                range_ctx,
                format_hour(current_hour),
            ))
        } else if score > 7.0 {
            Some(format!(
                "{}Current time is {} - the user is likely sleeping. \
                 Apply a higher bar for notification urgency.",
                range_ctx,
                format_hour(current_hour),
            ))
        } else {
            None
        }
    }

    /// Get recent messages from a known person on OTHER platforms (cross-platform escalation).
    /// Returns messages from the given person_id on platforms != exclude_platform, since since_ts.
    pub fn get_cross_platform_messages(
        &self,
        user_id: i32,
        person_id: i32,
        exclude_platform: &str,
        since_ts: i32,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::person_id.eq(person_id))
            .filter(ont_messages::platform.ne(exclude_platform))
            .filter(ont_messages::sender_name.ne("You"))
            .filter(ont_messages::created_at.gt(since_ts))
            .order(ont_messages::created_at.desc())
            .limit(5)
            .load::<OntMessage>(&mut conn)
    }

    /// Purge messages older than max_age_secs.
    pub fn purge_old_messages(&self, max_age_secs: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cutoff = Self::now() - max_age_secs;
        diesel::delete(ont_messages::table.filter(ont_messages::created_at.lt(cutoff)))
            .execute(&mut conn)
    }

    /// Update a message's classification (urgency, category, summary) after LLM evaluation.
    pub fn update_message_classification(
        &self,
        message_id: i64,
        urgency: &str,
        category: &str,
        summary: Option<&str>,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(ont_messages::table.filter(ont_messages::id.eq(message_id)))
            .set((
                ont_messages::urgency.eq(urgency),
                ont_messages::category.eq(category),
                ont_messages::summary.eq(summary),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Get pending digest items: medium-urgency messages not yet delivered, for a user.
    pub fn get_pending_digest_messages(
        &self,
        user_id: i32,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::urgency.eq("medium"))
            .filter(ont_messages::digest_delivered_at.is_null())
            .filter(ont_messages::sender_name.ne("You"))
            .order(ont_messages::created_at.desc())
            .limit(20)
            .load::<OntMessage>(&mut conn)
    }

    /// Mark digest messages as delivered.
    pub fn mark_digest_delivered(&self, message_ids: &[i64], now: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(ont_messages::table.filter(ont_messages::id.eq_any(message_ids)))
            .set(ont_messages::digest_delivered_at.eq(now))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Get all user IDs that have pending digest messages.
    pub fn get_users_with_pending_digests(&self) -> Result<Vec<i32>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::urgency.eq("medium"))
            .filter(ont_messages::digest_delivered_at.is_null())
            .filter(ont_messages::sender_name.ne("You"))
            .select(ont_messages::user_id)
            .distinct()
            .load::<i32>(&mut conn)
    }

    /// Compute the user's typical wake-up hour from their activity patterns.
    /// Returns the local hour (0-23) when the user typically starts being active.
    pub fn compute_user_wake_hour(&self, user_id: i32, tz_offset_secs: i32) -> Option<usize> {
        use crate::pg_schema::ont_messages;

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let thirty_days_ago = Self::now() - 30 * 86400;

        let user_messages: Vec<OntMessage> = ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::sender_name.eq("You"))
            .filter(ont_messages::created_at.ge(thirty_days_ago))
            .select(OntMessage::as_select())
            .load(&mut conn)
            .unwrap_or_default();

        if user_messages.len() < 10 {
            return None; // not enough data
        }

        let timestamps: Vec<i32> = user_messages.iter().map(|m| m.created_at).collect();
        let hour_buckets = bucket_by_hour(&timestamps, tz_offset_secs);

        // Find the first hour of activity: scan from hour 4 (earliest reasonable wake)
        // through the day, find the first hour with meaningful activity
        let total: u32 = hour_buckets.iter().sum();
        if total == 0 {
            return None;
        }
        let threshold = total as f64 * 0.02; // at least 2% of messages in this hour

        if let Some(h) = (4..24).find(|&h| hour_buckets[h] as f64 >= threshold) {
            return Some(h);
        }
        // Wrap around: check hours 0-3
        if let Some(h) = (0..4).find(|&h| hour_buckets[h] as f64 >= threshold) {
            return Some(h);
        }
        None
    }

    // -----------------------------------------------------------------------
    // Events (tracked items with lifecycle)
    // -----------------------------------------------------------------------

    pub fn create_event(&self, event: &NewOntEvent) -> Result<OntEvent, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let created: OntEvent = diesel::insert_into(ont_events::table)
            .values(event)
            .get_result(&mut conn)?;
        Self::log_change(
            &mut conn,
            event.user_id,
            "Event",
            created.id,
            "created",
            Some(format!("description={}", event.description)),
            "user_action",
        );
        Ok(created)
    }

    pub fn get_event(&self, user_id: i32, event_id: i32) -> Result<OntEvent, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_events::table
            .filter(ont_events::id.eq(event_id))
            .filter(ont_events::user_id.eq(user_id))
            .first(&mut conn)
    }

    pub fn get_active_events(&self, user_id: i32) -> Result<Vec<OntEvent>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_events::table
            .filter(ont_events::user_id.eq(user_id))
            .filter(ont_events::status.eq("active"))
            .order(ont_events::created_at.desc())
            .load(&mut conn)
    }

    pub fn get_events(
        &self,
        user_id: i32,
        status: Option<&str>,
    ) -> Result<Vec<OntEvent>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let mut query = ont_events::table
            .filter(ont_events::user_id.eq(user_id))
            .into_boxed();

        if let Some(status) = status {
            if status != "all" {
                query = query.filter(ont_events::status.eq(status));
            }
        }

        query.order(ont_events::created_at.desc()).load(&mut conn)
    }

    pub fn update_event_status(
        &self,
        user_id: i32,
        event_id: i32,
        status: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            ont_events::table
                .filter(ont_events::id.eq(event_id))
                .filter(ont_events::user_id.eq(user_id)),
        )
        .set((
            ont_events::status.eq(status),
            ont_events::updated_at.eq(Self::now()),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn dismiss_event(&self, user_id: i32, event_id: i32) -> Result<(), DieselError> {
        self.update_event_status(user_id, event_id, "dismissed")
    }

    pub fn update_event(
        &self,
        user_id: i32,
        event_id: i32,
        append_description: Option<&str>,
        status: Option<&str>,
        remind_at: Option<i32>,
        due_at: Option<i32>,
    ) -> Result<OntEvent, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        if let Some(append) = append_description {
            let current: OntEvent = ont_events::table
                .filter(ont_events::id.eq(event_id))
                .filter(ont_events::user_id.eq(user_id))
                .first(&mut conn)?;
            let merged = if current.description.is_empty() {
                append.to_string()
            } else {
                format!("{}\nUpdate: {}", current.description, append)
            };
            diesel::update(
                ont_events::table
                    .filter(ont_events::id.eq(event_id))
                    .filter(ont_events::user_id.eq(user_id)),
            )
            .set((
                ont_events::description.eq(merged),
                ont_events::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        }

        if let Some(s) = status {
            diesel::update(
                ont_events::table
                    .filter(ont_events::id.eq(event_id))
                    .filter(ont_events::user_id.eq(user_id)),
            )
            .set((ont_events::status.eq(s), ont_events::updated_at.eq(now)))
            .execute(&mut conn)?;
        }

        if remind_at.is_some() || due_at.is_some() {
            let current: OntEvent = ont_events::table
                .filter(ont_events::id.eq(event_id))
                .filter(ont_events::user_id.eq(user_id))
                .first(&mut conn)?;
            diesel::update(
                ont_events::table
                    .filter(ont_events::id.eq(event_id))
                    .filter(ont_events::user_id.eq(user_id)),
            )
            .set((
                ont_events::remind_at.eq(remind_at.or(current.remind_at)),
                ont_events::due_at.eq(due_at.or(current.due_at)),
                ont_events::status.eq("active"),
                ont_events::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        }

        ont_events::table
            .filter(ont_events::id.eq(event_id))
            .filter(ont_events::user_id.eq(user_id))
            .first(&mut conn)
    }

    /// Get active events with remind_at <= now (due for reminder).
    pub fn get_events_due_for_notification(&self, now: i32) -> Result<Vec<OntEvent>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_events::table
            .filter(ont_events::status.eq("active"))
            .filter(ont_events::remind_at.is_not_null())
            .filter(ont_events::remind_at.le(now))
            .load(&mut conn)
    }

    /// Get active or already-notified events with due_at <= now.
    pub fn get_expired_events(&self, now: i32) -> Result<Vec<OntEvent>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_events::table
            .filter(ont_events::status.eq_any(&["active", "notified"]))
            .filter(ont_events::due_at.is_not_null())
            .filter(ont_events::due_at.le(now))
            .load(&mut conn)
    }

    /// Purge completed/dismissed/expired/notified events older than max_age_secs.
    pub fn purge_old_events(&self, max_age_secs: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cutoff = Self::now() - max_age_secs;
        diesel::delete(
            ont_events::table
                .filter(ont_events::status.eq_any(&[
                    "completed",
                    "dismissed",
                    "expired",
                    "notified",
                ]))
                .filter(ont_events::updated_at.lt(cutoff)),
        )
        .execute(&mut conn)
    }

    // -----------------------------------------------------------------------
    // Rules (Automation -> Logic -> Action)
    // -----------------------------------------------------------------------

    pub fn create_rule(&self, rule: &NewOntRule) -> Result<OntRule, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let created: OntRule = diesel::insert_into(ont_rules::table)
            .values(rule)
            .get_result(&mut conn)?;
        Self::log_change(
            &mut conn,
            rule.user_id,
            "Rule",
            created.id,
            "created",
            Some(format!("name={}", rule.name)),
            "user_action",
        );
        Ok(created)
    }

    pub fn get_rules(&self, user_id: i32) -> Result<Vec<OntRule>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_rules::table
            .filter(ont_rules::user_id.eq(user_id))
            .order(ont_rules::created_at.desc())
            .load(&mut conn)
    }

    pub fn get_active_rules(&self, user_id: i32) -> Result<Vec<OntRule>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_rules::table
            .filter(ont_rules::user_id.eq(user_id))
            .filter(ont_rules::status.eq("active"))
            .load(&mut conn)
    }

    pub fn get_rule(&self, user_id: i32, rule_id: i32) -> Result<OntRule, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_rules::table
            .filter(ont_rules::id.eq(rule_id))
            .filter(ont_rules::user_id.eq(user_id))
            .first(&mut conn)
    }

    pub fn delete_rule(&self, user_id: i32, rule_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(
            ont_rules::table
                .filter(ont_rules::id.eq(rule_id))
                .filter(ont_rules::user_id.eq(user_id)),
        )
        .execute(&mut conn)?;
        Self::log_change(
            &mut conn,
            user_id,
            "Rule",
            rule_id,
            "deleted",
            None,
            "user_action",
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_rule(
        &self,
        user_id: i32,
        rule_id: i32,
        name: &str,
        trigger_type: &str,
        trigger_config: &str,
        logic_type: &str,
        logic_prompt: Option<&str>,
        logic_fetch: Option<&str>,
        action_type: &str,
        action_config: &str,
        next_fire_at: Option<i32>,
        flow_config: Option<&str>,
    ) -> Result<OntRule, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();

        diesel::update(
            ont_rules::table
                .filter(ont_rules::id.eq(rule_id))
                .filter(ont_rules::user_id.eq(user_id)),
        )
        .set((
            ont_rules::name.eq(name),
            ont_rules::trigger_type.eq(trigger_type),
            ont_rules::trigger_config.eq(trigger_config),
            ont_rules::logic_type.eq(logic_type),
            ont_rules::logic_prompt.eq(logic_prompt),
            ont_rules::logic_fetch.eq(logic_fetch),
            ont_rules::action_type.eq(action_type),
            ont_rules::action_config.eq(action_config),
            ont_rules::next_fire_at.eq(next_fire_at),
            ont_rules::flow_config.eq(flow_config),
            ont_rules::updated_at.eq(now),
        ))
        .get_result(&mut conn)
    }

    pub fn update_rule_status(&self, rule_id: i32, status: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(ont_rules::table.filter(ont_rules::id.eq(rule_id)))
            .set((
                ont_rules::status.eq(status),
                ont_rules::updated_at.eq(Self::now()),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_rule_last_triggered(&self, rule_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(ont_rules::table.filter(ont_rules::id.eq(rule_id)))
            .set((
                ont_rules::last_triggered_at.eq(Self::now()),
                ont_rules::updated_at.eq(Self::now()),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_rule_next_fire_at(
        &self,
        rule_id: i32,
        next_fire_at: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(ont_rules::table.filter(ont_rules::id.eq(rule_id)))
            .set((
                ont_rules::next_fire_at.eq(next_fire_at),
                ont_rules::updated_at.eq(Self::now()),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Get all schedule-based rules that are due to fire.
    pub fn get_due_schedule_rules(&self, now: i32) -> Result<Vec<OntRule>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_rules::table
            .filter(ont_rules::trigger_type.eq("schedule"))
            .filter(ont_rules::status.eq("active"))
            .filter(ont_rules::next_fire_at.le(now))
            .load(&mut conn)
    }

    /// Get all active ontology_change rules for a given user.
    /// Caller is responsible for matching trigger_config filters against the entity data.
    pub fn get_ontology_change_rules(&self, user_id: i32) -> Result<Vec<OntRule>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_rules::table
            .filter(ont_rules::user_id.eq(user_id))
            .filter(ont_rules::trigger_type.eq("ontology_change"))
            .filter(ont_rules::status.eq("active"))
            .load(&mut conn)
    }

    /// Count messages since a timestamp for a user.
    pub fn count_messages_since(&self, user_id: i32, since_ts: i32) -> Result<i64, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::created_at.gt(since_ts))
            .count()
            .get_result(&mut conn)
    }

    /// Get recent messages from known persons (person_id IS NOT NULL) since a timestamp.
    pub fn get_notifiable_messages(
        &self,
        user_id: i32,
        since_ts: i32,
        limit: i64,
    ) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::person_id.is_not_null())
            .filter(ont_messages::created_at.gt(since_ts))
            .order(ont_messages::created_at.desc())
            .limit(limit)
            .load::<OntMessage>(&mut conn)
    }

    /// Get distinct senders from ont_messages (for rule builder autocomplete).
    pub fn get_distinct_senders(
        &self,
        user_id: i32,
    ) -> Result<Vec<(String, String, i64)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        // Reuse SuggestionCandidate shape (room_id will be empty placeholder)
        diesel::sql_query(
            "SELECT sender_name, platform, '' as room_id, COUNT(*) as msg_count \
             FROM ont_messages \
             WHERE user_id = $1 \
             GROUP BY sender_name, platform \
             ORDER BY msg_count DESC \
             LIMIT 200",
        )
        .bind::<diesel::sql_types::Int4, _>(user_id)
        .load::<SuggestionCandidate>(&mut conn)
        .map(|rows| {
            rows.into_iter()
                .map(|r| (r.sender_name, r.platform, r.msg_count))
                .collect()
        })
    }

    // -----------------------------------------------------------------------
    // Changelog (for activity feed)
    // -----------------------------------------------------------------------

    /// Get recent changelog entries for a user.
    pub fn get_recent_changelog(
        &self,
        user_id: i32,
        since_ts: i32,
        limit: i64,
    ) -> Result<Vec<OntChangelog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_changelog::table
            .filter(ont_changelog::user_id.eq(user_id))
            .filter(ont_changelog::created_at.gt(since_ts))
            .order(ont_changelog::created_at.desc())
            .limit(limit)
            .load::<OntChangelog>(&mut conn)
    }

    /// Expire rules past their expires_at timestamp.
    pub fn expire_old_rules(&self, now: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            ont_rules::table
                .filter(ont_rules::status.eq("active"))
                .filter(ont_rules::expires_at.is_not_null())
                .filter(ont_rules::expires_at.le(now)),
        )
        .set((
            ont_rules::status.eq("expired"),
            ont_rules::updated_at.eq(now),
        ))
        .execute(&mut conn)
    }
}
