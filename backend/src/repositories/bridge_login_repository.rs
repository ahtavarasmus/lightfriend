use anyhow::{anyhow, Context, Result};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::{Array, BigInt, Text};

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

pub struct BridgeLoginRepository;

impl BridgeLoginRepository {
    /// Count bridge-side platform logins for Matrix users without modifying the
    /// bridge-owned database. Any query uncertainty is returned as an error so
    /// callers retain Tuwunel rooms.
    pub fn login_count(bridge_type: &str, matrix_user_ids: &[String]) -> Result<i64> {
        if matrix_user_ids.is_empty() {
            return Ok(0);
        }

        let env_name = bridge_database_env_name(bridge_type)
            .ok_or_else(|| anyhow!("unsupported bridge type {bridge_type}"))?;
        let database_url =
            std::env::var(env_name).with_context(|| format!("{env_name} is not configured"))?;
        let mut conn = PgConnection::establish(&database_url)
            .with_context(|| format!("failed to connect using {env_name}"))?;
        conn.batch_execute("SET default_transaction_read_only = on")
            .context("failed to make bridge login probe read-only")?;

        match bridge_type {
            "whatsapp" | "signal" => bridgev2_login_count(&mut conn, matrix_user_ids, bridge_type),
            "telegram" => telegram_login_count(&mut conn, matrix_user_ids),
            _ => Err(anyhow!("unsupported bridge type {bridge_type}")),
        }
    }
}

pub fn bridge_database_env_name(bridge_type: &str) -> Option<&'static str> {
    match bridge_type {
        "whatsapp" => Some("WHATSAPP_BRIDGE_DATABASE_URL"),
        "signal" => Some("SIGNAL_BRIDGE_DATABASE_URL"),
        "telegram" => Some("TELEGRAM_BRIDGE_DATABASE_URL"),
        _ => None,
    }
}

fn bridgev2_login_count(
    conn: &mut PgConnection,
    matrix_user_ids: &[String],
    bridge_type: &str,
) -> Result<i64> {
    let mut count = 0_i64;
    let mut schemas_found = 0_u8;

    for table in ["user_logins", "user_login"] {
        let query =
            format!("SELECT count(*)::BIGINT AS count FROM {table} WHERE user_mxid = ANY($1)");
        match diesel::sql_query(query)
            .bind::<Array<Text>, _>(matrix_user_ids)
            .get_result::<CountRow>(conn)
        {
            Ok(row) => {
                schemas_found = schemas_found.saturating_add(1);
                count = count.saturating_add(row.count);
            }
            Err(error) if relation_missing(&error) => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("{bridge_type} login query failed for {table}"));
            }
        }
    }

    if schemas_found == 0 {
        return Err(anyhow!(
            "{bridge_type} bridge database has neither user_logins nor user_login"
        ));
    }
    Ok(count)
}

fn telegram_login_count(conn: &mut PgConnection, matrix_user_ids: &[String]) -> Result<i64> {
    Ok(diesel::sql_query(
        r#"SELECT count(*)::BIGINT AS count
             FROM "user"
            WHERE mxid = ANY($1)
              AND tgid IS NOT NULL"#,
    )
    .bind::<Array<Text>, _>(matrix_user_ids)
    .get_result::<CountRow>(conn)
    .context("telegram login query failed")?
    .count)
}

fn relation_missing(error: &DieselError) -> bool {
    matches!(
        error,
        DieselError::DatabaseError(_, info)
            if info.message().contains("does not exist")
                && info.message().contains("relation")
    )
}
