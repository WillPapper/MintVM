#![allow(unused)]
#![allow(dead_code)]

use rusqlite::{Connection, Result, ToSql};
use rusqlite::types::ToSqlOutput;
use serde::{Serialize, Deserialize};
use thiserror::Error;

#[derive(Debug)]
struct Transactions {
    id: i32,
    transaction_type: TransactionType,
    // Signed TX data as bytes
    data: Option<Vec<u8>>,
    // Fetched from the metabased chain. Used to derive the block number
    timestamp: i64,
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

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Database error: {0}")]
    SqliteError(#[from] rusqlite::Error),
    #[error("Invalid transaction type: {0}")]
    InvalidTransactionType(String),
    #[error("Invalid transaction data: {0}")]
    InvalidTransactionData(String),
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
            data  BLOB,
            timestamp INTEGER NOT NULL
        )",
        (), // empty list of parameters.
    )?;

    Ok(conn)
}

// Connection must be mutable because commitments mutate the connection
fn insert_transaction(conn: &mut Connection, transaction: &Transactions) -> Result<(), DatabaseError> {
    // Start a new transaction
    let tx = conn.transaction()?;

    // Rust enums are checked at compile time, so we don't need to check that
    // the transaction type is valid
    // Error if data is null
    if transaction.data.is_none() {
        return Err(DatabaseError::InvalidTransactionData(
            "Transaction data cannot be null - all transactions must contain signed data".to_string()
        ));
    }

    tx.execute(
        "INSERT INTO transactions (transaction_type, data, timestamp) VALUES (?1, ?2, ?3)",
        (&transaction.transaction_type, &transaction.data, &transaction.timestamp),
    )?;

    // Commit the transaction
    tx.commit()?;

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
        let mut conn = initialize_db().unwrap();
        let transaction = Transactions {
            id: 0,
            transaction_type: TransactionType::CreateToken,
            data: Some("0x".as_bytes().to_vec()),
            timestamp: 1715136000,
        };
        insert_transaction(&mut conn, &transaction).unwrap();
    }
}
