use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::models::ontology_models::{
    NewOntChangelog, NewOntChannel, NewOntPerson, NewOntPersonEdit, OntChannel, OntPerson,
    OntPersonEdit, PersonWithChannels,
};
use crate::pg_schema::{ont_changelog, ont_channels, ont_person_edits, ont_persons};
use crate::PgDbPool;

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
            .filter(diesel::dsl::sql::<diesel::sql_types::Bool>(&format!(
                "LOWER(name) = LOWER('{}')",
                name.replace('\'', "''")
            )))
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
    ) -> Result<Vec<PersonWithChannels>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let persons = ont_persons::table
            .filter(ont_persons::user_id.eq(user_id))
            .order(ont_persons::name.asc())
            .load::<OntPerson>(&mut conn)?;

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
}
