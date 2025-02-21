use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::error::Error;
use chrono::Local;
use crate::{
    models::user_models::{User, Call, NewCall},
    schema::calls,
    DbPool,
};

pub struct UserCalls {
    pool: DbPool
}

impl UserCalls {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_call(&self, new_call: NewCall) -> Result<(), diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(calls::table)
            .values(&new_call)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_call(
        &self,
        call_id: i32,
        status: String,
        call_duration_secs: i32,
    ) -> Result<(), diesel::result::Error> {
        use crate::schema::calls::dsl::*;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(calls.find(call_id))
            .set((
                status.eq(status),
                call_duration_secs.eq(call_duration_secs),
            ))
            .execute(&mut conn)?;
        
        Ok(())
    }
}


