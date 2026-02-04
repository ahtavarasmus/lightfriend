#!/usr/bin/env python3
"""
SQLite to PostgreSQL Data Migration Script

This script migrates all data from the SQLite database to PostgreSQL.
It assumes the PostgreSQL schema has already been created via diesel migrations.

Usage:
    python3 migrate_sqlite_to_postgres.py <sqlite_db_path> <postgres_url>

Example:
    python3 migrate_sqlite_to_postgres.py ./database.db "postgres://user:pass@localhost/lightfriend"

Requirements:
    pip install psycopg2-binary
"""

import sqlite3
import psycopg2
import sys
from typing import List, Tuple, Any

# Tables in dependency order (tables with foreign keys come after their dependencies)
TABLES_IN_ORDER = [
    "users",
    "admin_alerts",
    "bridge_disconnection_events",
    "bridges",
    "calendar_notifications",
    "contact_profiles",
    "contact_profile_exceptions",
    "conversations",
    "country_availability",
    "critical_categories",
    "digests",
    "disabled_alert_types",
    "email_judgments",
    "google_calendar",
    "imap_connection",
    "keywords",
    "message_history",
    "message_status_log",
    "priority_senders",
    "processed_emails",
    "refund_info",
    "site_metrics",
    "subaccounts",
    "tasks",
    "tesla",
    "totp_backup_codes",
    "totp_secrets",
    "uber",
    "usage_logs",
    "user_info",
    "user_settings",
    "waitlist",
    "webauthn_challenges",
    "webauthn_credentials",
    "youtube",
]


def get_table_columns(sqlite_cursor, table_name: str) -> List[str]:
    """Get column names for a table from SQLite."""
    sqlite_cursor.execute(f"PRAGMA table_info({table_name})")
    return [row[1] for row in sqlite_cursor.fetchall()]


def get_table_data(sqlite_cursor, table_name: str) -> Tuple[List[str], List[Tuple[Any, ...]]]:
    """Get all data from a SQLite table."""
    columns = get_table_columns(sqlite_cursor, table_name)
    sqlite_cursor.execute(f"SELECT * FROM {table_name}")
    rows = sqlite_cursor.fetchall()
    return columns, rows


def insert_data(pg_cursor, table_name: str, columns: List[str], rows: List[Tuple[Any, ...]]):
    """Insert data into PostgreSQL table."""
    if not rows:
        print(f"  No data to migrate for {table_name}")
        return 0

    # Build INSERT statement with placeholders
    col_list = ", ".join(columns)
    placeholders = ", ".join(["%s"] * len(columns))

    # For tables with SERIAL PRIMARY KEY, we need to handle id specially
    insert_sql = f"INSERT INTO {table_name} ({col_list}) VALUES ({placeholders})"

    count = 0
    for row in rows:
        try:
            pg_cursor.execute(insert_sql, row)
            count += 1
        except Exception as e:
            print(f"  Error inserting row into {table_name}: {e}")
            print(f"  Row data: {row[:3]}...")  # Print first 3 columns for debugging
            raise

    return count


def reset_sequence(pg_cursor, table_name: str):
    """Reset PostgreSQL sequence to max(id) + 1."""
    try:
        # Check if table has an id column with a sequence
        pg_cursor.execute(f"""
            SELECT column_default
            FROM information_schema.columns
            WHERE table_name = %s AND column_name = 'id'
        """, (table_name,))
        result = pg_cursor.fetchone()

        if result and result[0] and 'nextval' in str(result[0]):
            # Extract sequence name from default value like "nextval('users_id_seq'::regclass)"
            seq_name = f"{table_name}_id_seq"

            # Get max id
            pg_cursor.execute(f"SELECT COALESCE(MAX(id), 0) FROM {table_name}")
            max_id = pg_cursor.fetchone()[0]

            # Reset sequence
            pg_cursor.execute(f"SELECT setval('{seq_name}', %s, true)", (max_id,))
            print(f"  Reset sequence {seq_name} to {max_id}")
    except Exception as e:
        print(f"  Warning: Could not reset sequence for {table_name}: {e}")


def migrate_table(sqlite_cursor, pg_cursor, table_name: str) -> int:
    """Migrate a single table from SQLite to PostgreSQL."""
    print(f"Migrating {table_name}...")

    columns, rows = get_table_data(sqlite_cursor, table_name)
    count = insert_data(pg_cursor, table_name, columns, rows)

    if count > 0:
        reset_sequence(pg_cursor, table_name)

    print(f"  Migrated {count} rows")
    return count


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(1)

    sqlite_path = sys.argv[1]
    postgres_url = sys.argv[2]

    print(f"Source SQLite: {sqlite_path}")
    print(f"Target PostgreSQL: {postgres_url.split('@')[1] if '@' in postgres_url else postgres_url}")
    print()

    # Connect to SQLite
    sqlite_conn = sqlite3.connect(sqlite_path)
    sqlite_cursor = sqlite_conn.cursor()

    # Connect to PostgreSQL
    pg_conn = psycopg2.connect(postgres_url)
    pg_cursor = pg_conn.cursor()

    try:
        # Disable foreign key checks during migration
        pg_cursor.execute("SET session_replication_role = 'replica'")

        total_rows = 0
        for table in TABLES_IN_ORDER:
            try:
                count = migrate_table(sqlite_cursor, pg_cursor, table)
                total_rows += count
            except Exception as e:
                print(f"Error migrating {table}: {e}")
                pg_conn.rollback()
                raise

        # Re-enable foreign key checks
        pg_cursor.execute("SET session_replication_role = 'origin'")

        # Commit all changes
        pg_conn.commit()

        print()
        print(f"Migration complete! Total rows migrated: {total_rows}")

    except Exception as e:
        pg_conn.rollback()
        print(f"Migration failed: {e}")
        sys.exit(1)
    finally:
        sqlite_cursor.close()
        sqlite_conn.close()
        pg_cursor.close()
        pg_conn.close()


if __name__ == "__main__":
    main()
