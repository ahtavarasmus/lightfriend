use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::{
    models::user_models::{
        DailyCheckin, NewDailyCheckin, NewWellbeingPointEvent, NewWellbeingPoints,
        WellbeingPointEvent, WellbeingPoints,
    },
    schema::{daily_checkins, user_settings, wellbeing_point_events, wellbeing_points},
    DbPool,
};

pub struct WellbeingRepository {
    pub pool: DbPool,
}

impl WellbeingRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // --- Dumbphone Mode ---

    pub fn get_dumbphone_mode(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let on: i32 = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::dumbphone_mode_on)
            .first(&mut conn)?;
        Ok(on != 0)
    }

    pub fn set_dumbphone_mode(&self, user_id: i32, on: bool) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::dumbphone_mode_on.eq(if on { 1 } else { 0 }))
            .execute(&mut conn)?;
        Ok(())
    }

    // --- Notification Calmer ---

    pub fn get_notification_calmer(
        &self,
        user_id: i32,
    ) -> Result<(bool, Option<String>), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let (on, schedule): (i32, Option<String>) = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select((
                user_settings::notification_calmer_on,
                user_settings::notification_calmer_schedule,
            ))
            .first(&mut conn)?;
        Ok((on != 0, schedule))
    }

    pub fn set_notification_calmer(
        &self,
        user_id: i32,
        on: bool,
        schedule: Option<String>,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set((
                user_settings::notification_calmer_on.eq(if on { 1 } else { 0 }),
                user_settings::notification_calmer_schedule.eq(schedule),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    // --- Daily Check-in ---

    pub fn get_checkin_for_date(
        &self,
        user_id: i32,
        date: &str,
    ) -> Result<Option<DailyCheckin>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        daily_checkins::table
            .filter(daily_checkins::user_id.eq(user_id))
            .filter(daily_checkins::checkin_date.eq(date))
            .first::<DailyCheckin>(&mut conn)
            .optional()
    }

    pub fn upsert_checkin(&self, new: &NewDailyCheckin) -> Result<DailyCheckin, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        // Try to update existing
        let updated = diesel::update(
            daily_checkins::table
                .filter(daily_checkins::user_id.eq(new.user_id))
                .filter(daily_checkins::checkin_date.eq(&new.checkin_date)),
        )
        .set((
            daily_checkins::mood.eq(new.mood),
            daily_checkins::energy.eq(new.energy),
            daily_checkins::sleep_quality.eq(new.sleep_quality),
        ))
        .execute(&mut conn)?;

        if updated == 0 {
            diesel::insert_into(daily_checkins::table)
                .values(new)
                .execute(&mut conn)?;
        }

        daily_checkins::table
            .filter(daily_checkins::user_id.eq(new.user_id))
            .filter(daily_checkins::checkin_date.eq(&new.checkin_date))
            .first::<DailyCheckin>(&mut conn)
    }

    pub fn get_checkin_history(
        &self,
        user_id: i32,
        limit: i64,
    ) -> Result<Vec<DailyCheckin>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        daily_checkins::table
            .filter(daily_checkins::user_id.eq(user_id))
            .order(daily_checkins::checkin_date.desc())
            .limit(limit)
            .load::<DailyCheckin>(&mut conn)
    }

    // --- Wellbeing Points ---

    pub fn get_or_create_points(&self, user_id: i32) -> Result<WellbeingPoints, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let existing = wellbeing_points::table
            .filter(wellbeing_points::user_id.eq(user_id))
            .first::<WellbeingPoints>(&mut conn)
            .optional()?;

        match existing {
            Some(p) => Ok(p),
            None => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;
                let new = NewWellbeingPoints {
                    user_id,
                    points: 0,
                    current_streak: 0,
                    longest_streak: 0,
                    last_activity_date: None,
                    created_at: now,
                };
                diesel::insert_into(wellbeing_points::table)
                    .values(&new)
                    .execute(&mut conn)?;
                wellbeing_points::table
                    .filter(wellbeing_points::user_id.eq(user_id))
                    .first::<WellbeingPoints>(&mut conn)
            }
        }
    }

    /// Award points for an event, once per day per event_type.
    /// Updates streak automatically.
    pub fn award_points(
        &self,
        user_id: i32,
        event_type: &str,
        points: i32,
        today: &str,
    ) -> Result<WellbeingPoints, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Check if already awarded this event type today
        let existing_event: Option<WellbeingPointEvent> = wellbeing_point_events::table
            .filter(wellbeing_point_events::user_id.eq(user_id))
            .filter(wellbeing_point_events::event_type.eq(event_type))
            .filter(wellbeing_point_events::event_date.eq(today))
            .first::<WellbeingPointEvent>(&mut conn)
            .optional()?;

        if existing_event.is_some() {
            // Already awarded today, just return current points
            return self.get_or_create_points(user_id);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Record the event
        let event = NewWellbeingPointEvent {
            user_id,
            event_type: event_type.to_string(),
            points_earned: points,
            event_date: today.to_string(),
            created_at: now,
        };
        diesel::insert_into(wellbeing_point_events::table)
            .values(&event)
            .execute(&mut conn)?;

        // Get or create points record
        let current = self.get_or_create_points(user_id)?;

        // Calculate streak
        let yesterday = calculate_yesterday(today);
        let (new_streak, new_longest) = if current.last_activity_date.as_deref() == Some(today) {
            // Already active today
            (current.current_streak, current.longest_streak)
        } else if current.last_activity_date.as_deref() == Some(&yesterday) {
            // Consecutive day
            let s = current.current_streak + 1;
            (s, s.max(current.longest_streak))
        } else {
            // Streak broken or first day
            (1, current.longest_streak.max(1))
        };

        // Update points
        diesel::update(wellbeing_points::table.filter(wellbeing_points::user_id.eq(user_id)))
            .set((
                wellbeing_points::points.eq(current.points + points),
                wellbeing_points::current_streak.eq(new_streak),
                wellbeing_points::longest_streak.eq(new_longest),
                wellbeing_points::last_activity_date.eq(today),
            ))
            .execute(&mut conn)?;

        wellbeing_points::table
            .filter(wellbeing_points::user_id.eq(user_id))
            .first::<WellbeingPoints>(&mut conn)
    }

    pub fn get_recent_events(
        &self,
        user_id: i32,
        limit: i64,
    ) -> Result<Vec<WellbeingPointEvent>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        wellbeing_point_events::table
            .filter(wellbeing_point_events::user_id.eq(user_id))
            .order(wellbeing_point_events::created_at.desc())
            .limit(limit)
            .load::<WellbeingPointEvent>(&mut conn)
    }

    // --- Wellbeing Stats ---

    pub fn get_wellbeing_signup_timestamp(&self, user_id: i32) -> Result<Option<i32>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::wellbeing_signup_timestamp)
            .first(&mut conn)
    }

    pub fn ensure_wellbeing_signup_timestamp(
        &self,
        user_id: i32,
        now: i32,
    ) -> Result<i32, DieselError> {
        let existing = self.get_wellbeing_signup_timestamp(user_id)?;
        match existing {
            Some(ts) => Ok(ts),
            None => {
                let mut conn = self.pool.get().expect("Failed to get DB connection");
                diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
                    .set(user_settings::wellbeing_signup_timestamp.eq(now))
                    .execute(&mut conn)?;
                Ok(now)
            }
        }
    }
}

/// Calculate yesterday's date string (YYYY-MM-DD) from today's date string.
fn calculate_yesterday(today: &str) -> String {
    use chrono::NaiveDate;
    if let Ok(date) = NaiveDate::parse_from_str(today, "%Y-%m-%d") {
        (date - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string()
    } else {
        String::new()
    }
}
