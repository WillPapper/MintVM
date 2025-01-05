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

fn main() -> Result<()> {
    let conn = initialize_db()?;

    Ok(())
}

fn initialize_db() -> Result<Connection, rusqlite::Error> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main() {
        assert!(main().is_ok());
    }
}