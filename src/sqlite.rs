#![allow(unused)]
#![allow(dead_code)]

use rusqlite::{Connection, Result, ToSql};
use rusqlite::types::ToSqlOutput;
use serde::{Serialize, Deserialize};

#[derive(Debug)]
struct Transactions {
    id: i32,
    transaction_type: TransactionType,
    // Signed TX data as bytes
    data: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize, strum::Display, strum::EnumString)]
enum TransactionType {
    CreateToken,
    AddTokenSigner,
    RemoveTokenSigner,
    SetDefaultTokenURI,
    SetTokenURIPerId,
    Mint,
    Transfer,
    Burn,
    Approve,
    SetApprovalForAll,
}

impl ToSql for TransactionType {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

#[derive(Debug)]
pub enum DatabaseError {
    SqliteError(rusqlite::Error),
    InvalidTransactionType(String),
    InvalidTransactionData(String),
}

impl std::error::Error for DatabaseError {}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DatabaseError::SqliteError(e) => write!(f, "Database error: {}", e),
            DatabaseError::InvalidTransactionType(msg) => write!(f, "Invalid transaction type: {}", msg),
            DatabaseError::InvalidTransactionData(msg) => write!(f, "Invalid transaction data: {}", msg),
        }
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(err: rusqlite::Error) -> DatabaseError {
        DatabaseError::SqliteError(err)
    }
}

fn main() -> Result<(), DatabaseError> {
    let conn = initialize_db()?;
    Ok(())
}

fn initialize_db() -> Result<Connection, DatabaseError> {
    let conn = Connection::open_in_memory()?;

    // Change ID to use the ID from the smart contract once written
    // For now we'll auto-increment for testing purposes, but later on we'll use
    // the ID from the smart contract
    conn.execute(
        "CREATE TABLE transactions(
            id    INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_type TEXT NOT NULL,
            data  BLOB
        )",
        (), // empty list of parameters.
    )?;

    Ok(conn)
}

fn insert_transaction(conn: &Connection, transaction: &Transactions) -> Result<(), DatabaseError> {
    // Rust enums are checked at compile time, so we don't need to check that the transaction type is valid

    // Error if data is null
    // TODO: Error if data is not a valid signed Ethereum transaction
    if transaction.data.is_none() {
        return Err(DatabaseError::InvalidTransactionData(
            "Transaction data cannot be null - all transactions must contain signed data".to_string()
        ));
    }

    conn.execute(
        "INSERT INTO transactions (transaction_type, data) VALUES (?1, ?2)",
        (&transaction.transaction_type, &transaction.data),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main() {
        assert!(main().is_ok());
    }

    #[test]
    fn test_insert_transaction() {
        let conn = initialize_db().unwrap();
        let transaction = Transactions {
            id: 0,
            transaction_type: TransactionType::CreateToken,
            data: Some("0x".as_bytes().to_vec()),
        };
        insert_transaction(&conn, &transaction).unwrap();
    }
}
