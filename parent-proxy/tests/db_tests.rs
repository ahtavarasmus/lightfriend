use parent_proxy::db::{Database, EnclaveTarget};
use rusqlite::Connection;
use tempfile::NamedTempFile;

fn setup_test_db() -> (NamedTempFile, Database) {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    // Create database with schema
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            phone_number TEXT NOT NULL,
            active_enclave TEXT
        )",
        [],
    )
    .unwrap();

    // Insert test data
    conn.execute(
        "INSERT INTO users (id, phone_number, active_enclave) VALUES (1, '+1234567890', NULL)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO users (id, phone_number, active_enclave) VALUES (2, '+1987654321', 'old')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO users (id, phone_number, active_enclave) VALUES (3, '+1555555555', 'new')",
        [],
    )
    .unwrap();

    drop(conn);

    let db = Database::new(path).unwrap();
    (temp_file, db)
}

#[test]
fn test_get_enclave_by_phone_null() {
    let (_temp, db) = setup_test_db();
    let target = db.get_enclave_by_phone("+1234567890").unwrap();
    assert_eq!(target, EnclaveTarget::New);
}

#[test]
fn test_get_enclave_by_phone_old() {
    let (_temp, db) = setup_test_db();
    let target = db.get_enclave_by_phone("+1987654321").unwrap();
    assert_eq!(target, EnclaveTarget::Old);
}

#[test]
fn test_get_enclave_by_phone_new() {
    let (_temp, db) = setup_test_db();
    let target = db.get_enclave_by_phone("+1555555555").unwrap();
    assert_eq!(target, EnclaveTarget::New);
}

#[test]
fn test_get_enclave_by_phone_not_found() {
    let (_temp, db) = setup_test_db();
    let target = db.get_enclave_by_phone("+9999999999").unwrap();
    assert_eq!(target, EnclaveTarget::New);
}

#[test]
fn test_get_enclave_by_user_id_null() {
    let (_temp, db) = setup_test_db();
    let target = db.get_enclave_by_user_id(1).unwrap();
    assert_eq!(target, EnclaveTarget::New);
}

#[test]
fn test_get_enclave_by_user_id_new() {
    let (_temp, db) = setup_test_db();
    let target = db.get_enclave_by_user_id(3).unwrap();
    assert_eq!(target, EnclaveTarget::New);
}

#[test]
fn test_get_enclave_by_user_id_not_found() {
    let (_temp, db) = setup_test_db();
    let target = db.get_enclave_by_user_id(999).unwrap();
    assert_eq!(target, EnclaveTarget::New);
}
