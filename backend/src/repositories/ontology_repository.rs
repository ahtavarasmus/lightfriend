use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::models::ontology_models::{
    NewOntChangelog, NewOntChannel, NewOntLink, NewOntMessage, NewOntPerson, NewOntPersonEdit,
    NewOntRule, OntChangelog, OntChannel, OntLink, OntMessage, OntPerson, OntPersonEdit, OntRule,
    PersonWithChannels,
};
use crate::pg_schema::{
    ont_changelog, ont_channels, ont_links, ont_messages, ont_person_edits, ont_persons, ont_rules,
};
use crate::PgDbPool;

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
             WHERE user_id = $1 AND person_id IS NULL \
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

    /// Purge messages older than max_age_secs (but never purge pinned messages).
    pub fn purge_old_messages(&self, max_age_secs: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cutoff = Self::now() - max_age_secs;
        diesel::delete(
            ont_messages::table
                .filter(ont_messages::created_at.lt(cutoff))
                .filter(ont_messages::pinned.eq(false)),
        )
        .execute(&mut conn)
    }

    /// Pin a message (set pinned=true).
    pub fn pin_message(&self, user_id: i32, message_id: i64) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            ont_messages::table
                .filter(ont_messages::id.eq(message_id))
                .filter(ont_messages::user_id.eq(user_id)),
        )
        .set(ont_messages::pinned.eq(true))
        .execute(&mut conn)?;
        Ok(())
    }

    /// Pin a message and set its review_after deadline.
    pub fn pin_message_with_deadline(
        &self,
        user_id: i32,
        message_id: i64,
        review_after: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            ont_messages::table
                .filter(ont_messages::id.eq(message_id))
                .filter(ont_messages::user_id.eq(user_id)),
        )
        .set((
            ont_messages::pinned.eq(true),
            ont_messages::review_after.eq(Some(review_after)),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    /// Unpin a message (set pinned=false).
    pub fn unpin_message(&self, user_id: i32, message_id: i64) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            ont_messages::table
                .filter(ont_messages::id.eq(message_id))
                .filter(ont_messages::user_id.eq(user_id)),
        )
        .set(ont_messages::pinned.eq(false))
        .execute(&mut conn)?;
        Ok(())
    }

    /// Update a pinned message's status. If status == "completed", also unpin.
    /// If status == "extend_deadline", push review_after forward by 7 days.
    pub fn update_message_status(
        &self,
        user_id: i32,
        message_id: i64,
        status: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        if status == "completed" {
            diesel::update(
                ont_messages::table
                    .filter(ont_messages::id.eq(message_id))
                    .filter(ont_messages::user_id.eq(user_id)),
            )
            .set((
                ont_messages::status.eq(Some(status)),
                ont_messages::pinned.eq(false),
            ))
            .execute(&mut conn)?;
        } else if status == "extend_deadline" {
            let new_review_after = Self::now() + 7 * 86400;
            diesel::update(
                ont_messages::table
                    .filter(ont_messages::id.eq(message_id))
                    .filter(ont_messages::user_id.eq(user_id)),
            )
            .set((
                ont_messages::status.eq(Some("active")),
                ont_messages::review_after.eq(Some(new_review_after)),
            ))
            .execute(&mut conn)?;
        } else {
            diesel::update(
                ont_messages::table
                    .filter(ont_messages::id.eq(message_id))
                    .filter(ont_messages::user_id.eq(user_id)),
            )
            .set(ont_messages::status.eq(Some(status)))
            .execute(&mut conn)?;
        }
        Ok(())
    }

    /// Get all pinned messages for a user.
    pub fn get_pinned_messages(&self, user_id: i32) -> Result<Vec<OntMessage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        ont_messages::table
            .filter(ont_messages::user_id.eq(user_id))
            .filter(ont_messages::pinned.eq(true))
            .order(ont_messages::created_at.desc())
            .load(&mut conn)
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
