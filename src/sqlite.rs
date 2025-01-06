#![allow(unused)]
#![allow(dead_code)]

use rusqlite::{Connection, Result, ToSql};
use rusqlite::types::ToSqlOutput;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use alloy::primitives::{Address, keccak256};
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
struct AddressSqlite(Address);

impl ToSql for AddressSqlite {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_slice()))
    }
}

impl From<Address> for AddressSqlite {
    fn from(addr: Address) -> Self {
        AddressSqlite(addr)
    }
}

impl From<AddressSqlite> for Address {
    fn from(addr: AddressSqlite) -> Self {
        addr.0
    }
}

#[derive(Debug)]
struct Transactions {
    id: i32,
    sender: AddressSqlite,
    transaction_type: TransactionType,
    data: Vec<u8>,
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

#[derive(Debug)]
struct Contracts {
    id: i32,
    address: AddressSqlite,
    signers: Vec<AddressSqlite>,
    transaction_id: i32,
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
    
    // Register custom functions first
    conn.create_scalar_function(
        "derive_contract_address",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let transaction_id: i64 = ctx.get::<i64>(0)?;
            
            // CREATE2 address derivation
            // address = keccak256(0xff ++ deployerAddress ++ salt ++ keccak256(initCode))[12:]
            
            // Using a fixed deployer address and init code for this example
            // In production, these should be parameters or configured constants
            // TODO: Change to sender of bridge address
            let deployer = AddressSqlite::from(
                Address::from_str("0x4000000000000000000000000000000000000000").unwrap()
            );
            
            // This should be your actual contract init code
            // TODO: Change to ERC-721/20/1155 init code
            let init_code = hex::decode("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
            
            // Calculate keccak256(initCode)
            let init_code_hash = keccak256(&init_code);
            
            // Prepare the CREATE2 input buffer
            let mut buffer = Vec::with_capacity(85); // 1 + 20 + 32 + 32
            buffer.push(0xff);
            buffer.extend_from_slice(deployer.0.as_slice());
            
            // Use transaction_id as salt, padded to 32 bytes
            let mut salt = [0u8; 32];
            // We want to pad the address to the right so that transaction ID comes at the end
            salt[24..32].copy_from_slice(&transaction_id.to_be_bytes());
            buffer.extend_from_slice(&salt);
            
            buffer.extend_from_slice(init_code_hash.as_slice());
            
            // Calculate final hash and take last 20 bytes for the address
            let address_bytes = &keccak256(&buffer)[12..];
            Ok(address_bytes.to_vec())
        }
    )?;

    // Change ID to use the ID from the smart contract once written
    // For now we'll auto-increment for testing purposes, but later on we'll use
    // the ID from the smart contract
    conn.execute(
        "CREATE TABLE transactions(
            id    INTEGER PRIMARY KEY AUTOINCREMENT,
            sender BLOB NOT NULL,
            transaction_type TEXT NOT NULL,
            data  BLOB,
            timestamp INTEGER NOT NULL
        )",
        (), // empty list of parameters.
    )?;

    // Create a table for contract addresses
    // Contract addresses are unique. Transactions and contracts are 1:1 and also unique
    conn.execute(
        "CREATE TABLE contracts(
            id    INTEGER PRIMARY KEY AUTOINCREMENT,
            address BLOB NOT NULL UNIQUE,
            signers BLOB,
            transaction_id INTEGER NOT NULL UNIQUE
        )",
        (),
    )?;

    // Create a trigger to automatically create a new contract when a
    // TransactionType of CreateToken is inserted. Uses a custom function to
    // derive the contract address from the transaction ID
    // Down the road, this can be updated with a salt so that the contract is
    // synced with CREATE2
    conn.execute(
        "CREATE TRIGGER create_contract_trigger AFTER INSERT ON transactions
        WHEN NEW.transaction_type = 'CreateToken'
        BEGIN
            INSERT INTO contracts (address, signers, transaction_id) 
            VALUES (derive_contract_address(NEW.id), NEW.sender, NEW.id);
        END",
        (),
    )?;

    Ok(conn)
}

// Connection must be mutable because commitments mutate the connection
fn insert_transaction(conn: &mut Connection, transaction: &Transactions) -> Result<(), DatabaseError> {
    // Start a new transaction
    let tx = conn.transaction()?;

    // Rust enums are checked at compile time, so we don't need to check that
    // the transaction type is valid

    tx.execute(
        "INSERT INTO transactions (sender, transaction_type, data, timestamp) VALUES (?1, ?2, ?3, ?4)",
        (&transaction.sender, &transaction.transaction_type, &transaction.data, &transaction.timestamp),
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
    fn test_insert_transaction() -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = initialize_db()?;
        let transaction = Transactions {
            id: 0,
            sender: AddressSqlite::from(Address::from_str("0x0000000000000000000000000000000000000001").unwrap()),
            transaction_type: TransactionType::CreateToken,
            data: "0x".as_bytes().to_vec(),
            timestamp: 1715136000,
        };
        insert_transaction(&mut conn, &transaction)?;

        // Run queries to confirm that the transaction was inserted
        let transaction_id = conn.query_row("SELECT * FROM transactions", [], |row| {
            row.get::<usize, i32>(0)
        })?;
        println!("Transaction inserted: {}", transaction_id);

        // Run queries to confirm that the contract was created
        let contract_id = conn.query_row("SELECT * FROM contracts", [], |row| {
            row.get::<usize, i32>(0)
        })?;
        println!("Contract created: {}", contract_id);

        Ok(())
    }
}
