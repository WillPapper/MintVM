#![allow(unused)]
#![allow(dead_code)]

use rusqlite::{Connection, Result, ToSql};
use rusqlite::types::{ToSqlOutput, FromSql};
use serde::{Serialize, Deserialize};
use thiserror::Error;
use alloy::primitives::{Address, keccak256};
use derive_more::{From, Display, FromStr};
use rusqlite::Row;
use rusqlite::named_params;
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, From, Display, FromStr, PartialEq)]
#[display("{}", _0)]
struct AddressSqlite(Address);

impl ToSql for AddressSqlite {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_slice()))
    }
}

impl rusqlite::types::FromSql for AddressSqlite {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            rusqlite::types::ValueRef::Blob(bytes) => {
                if bytes.len() != 20 {
                    return Err(rusqlite::types::FromSqlError::InvalidType);
                }
                let mut array = [0u8; 20];
                array.copy_from_slice(bytes);
                Ok(AddressSqlite(Address::from_slice(&array)))
            }
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
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

#[derive(Debug, Serialize, Deserialize, strum::Display, strum::EnumString, PartialEq)]
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

impl FromSql for TransactionType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let text = value.as_str()?;
        text.parse()
            .map_err(|_| rusqlite::types::FromSqlError::InvalidType)
    }
}

struct AddressSqliteList(Vec<AddressSqlite>);

// Show AddressSqliteList as a comma-separated list of addresses
impl std::fmt::Debug for AddressSqliteList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        for (i, addr) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", addr)?;
        }
        f.write_str("]")
    }
}

impl ToSql for AddressSqliteList {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        let mut bytes = Vec::with_capacity(self.0.len() * 20);
        for addr in &self.0 {
            bytes.extend_from_slice(addr.0.as_slice());
        }
        Ok(ToSqlOutput::from(bytes))
    }
}

impl FromSql for AddressSqliteList {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            rusqlite::types::ValueRef::Blob(bytes) => {
                if bytes.len() % 20 != 0 {
                    return Err(rusqlite::types::FromSqlError::InvalidType);
                }
                let addresses = bytes.chunks_exact(20)
                    .map(|chunk| {
                        let mut array = [0u8; 20];
                        array.copy_from_slice(chunk);
                        AddressSqlite(Address::from_slice(&array))
                    })
                    .collect();
                Ok(AddressSqliteList(addresses))
            }
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}

#[derive(Debug)]
struct Contracts {
    id: i32,
    address: AddressSqlite,
    signers: AddressSqliteList,
    transaction_id: i32,
}

impl TryFrom<&Row<'_>> for Contracts {
    type Error = rusqlite::Error;

    fn try_from(row: &Row) -> Result<Self, Self::Error> {
        Ok(Contracts {
            id: row.get(0)?,
            address: row.get(1)?,
            signers: row.get(2)?,
            transaction_id: row.get(3)?,
        })
    }
}

impl Contracts {
    // These getters are guaranteed to be unique based on the table constraints
    fn get_by_id(conn: &Connection, id: i32) -> Result<Self, rusqlite::Error> {
        conn.query_row(
            "SELECT * FROM contracts WHERE id = ?",
            [id],
            |row| Ok(Self::try_from(row)?)
        )
    }

    fn get_by_address(conn: &Connection, address: AddressSqlite) -> Result<Self, rusqlite::Error> {
        conn.query_row(
            "SELECT * FROM contracts WHERE address = ?",
            [address],
            |row| Ok(Self::try_from(row)?)
        )
    }

    fn get_by_transaction_id(conn: &Connection, tx_id: i32) -> Result<Self, rusqlite::Error> {
        conn.query_row(
            "SELECT * FROM contracts WHERE transaction_id = ?",
            [tx_id],
            |row| Ok(Self::try_from(row)?)
        )
    }
}

impl TryFrom<&Row<'_>> for Transactions {
    type Error = rusqlite::Error;

    fn try_from(row: &Row) -> Result<Self, Self::Error> {
        Ok(Transactions {
            id: row.get(0)?,
            sender: row.get(1)?,
            transaction_type: row.get(2)?,
            data: row.get(3)?,
            timestamp: row.get(4)?,
        })
    }
}

impl Transactions {
    fn get_by_id(conn: &Connection, id: i32) -> Result<Self, rusqlite::Error> {
        conn.query_row(
            "SELECT * FROM transactions WHERE id = ?",
            [id],
            |row| Ok(Self::try_from(row)?)
        )
    }

    fn get_by_sender(conn: &Connection, sender: AddressSqlite) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT * FROM transactions WHERE sender = ?")?;
        let transactions_iter = stmt.query_map([sender], |row| Ok(Self::try_from(row)?))?;
        
        // Collect and handle potential errors in the iterator
        transactions_iter.collect::<Result<Vec<_>, _>>()
    }

    fn get_by_type(conn: &Connection, tx_type: TransactionType) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT * FROM transactions WHERE transaction_type = ?")?;
        let transactions_iter = stmt.query_map([tx_type], |row| Ok(Self::try_from(row)?))?;
        
        // Collect and handle potential errors in the iterator
        transactions_iter.collect::<Result<Vec<_>, _>>()
    }

    fn get_by_type_and_sender(
        conn: &Connection, 
        tx_type: TransactionType,
        sender: AddressSqlite
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT * FROM transactions WHERE transaction_type = :type AND sender = :sender"
        )?;
        let transactions_iter = stmt.query_map(
            named_params! {":type": tx_type, ":sender": sender},
            |row| Ok(Self::try_from(row)?)
        )?;
        
        transactions_iter.collect::<Result<Vec<_>, _>>()
    }

    fn get_by_type_after_timestamp(
        conn: &Connection,
        tx_type: TransactionType,
        timestamp: i64
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT * FROM transactions WHERE transaction_type = :type AND timestamp > :ts"
        )?;
        let transactions_iter = stmt.query_map(
            named_params! {":type": tx_type, ":ts": timestamp},
            |row| Ok(Self::try_from(row)?)
        )?;
        
        transactions_iter.collect::<Result<Vec<_>, _>>()
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
        let sender = AddressSqlite::from(Address::from_str("0x0000000000000000000000000000000000000001").unwrap());
        let test_data = "0x".as_bytes().to_vec();
        let test_timestamp = 1715136000;

        let transaction = Transactions {
            id: 0,
            sender,
            transaction_type: TransactionType::CreateToken,
            data: test_data.clone(),
            timestamp: test_timestamp,
        };
        insert_transaction(&mut conn, &transaction)?;

        // Use getter instead of direct row access
        let saved_transaction = Transactions::get_by_id(&conn, 1)?;
        
        assert_eq!(saved_transaction.id, 1); // First record should have ID 1
        assert_eq!(saved_transaction.sender, sender);
        assert_eq!(saved_transaction.transaction_type, TransactionType::CreateToken);
        assert_eq!(saved_transaction.data, test_data);
        assert_eq!(saved_transaction.timestamp, test_timestamp);

        // Verify contract was created
        let contract = Contracts::get_by_id(&conn, 1)?;
        assert_eq!(contract.transaction_id, 1);
        assert_eq!(contract.signers.0, vec![sender]); // Verify sender is set as initial signer

        Ok(())
    }

    #[test]
    fn test_get_contract() -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = initialize_db()?;
        let sender = AddressSqlite::from(Address::from_str("0x0000000000000000000000000000000000000001").unwrap());
        
        // First insert a transaction that will create a contract
        let transaction = Transactions {
            id: 0,
            sender,
            transaction_type: TransactionType::CreateToken,
            data: "0x".as_bytes().to_vec(),
            timestamp: 1715136000,
        };
        insert_transaction(&mut conn, &transaction)?;

        // Get contract by ID
        let contract = Contracts::get_by_id(&conn, 1)?;
        assert_eq!(contract.id, 1);
        assert_eq!(contract.transaction_id, 1);
        assert_eq!(contract.signers.0, vec![sender]);

        // Get contract by address and verify it matches
        let contract_by_addr = Contracts::get_by_address(&conn, contract.address)?;
        assert_eq!(contract_by_addr.id, contract.id);
        assert_eq!(contract_by_addr.address, contract.address);
        assert_eq!(contract_by_addr.signers.0, contract.signers.0);
        assert_eq!(contract_by_addr.transaction_id, contract.transaction_id);

        // Get contract by transaction ID and verify it matches
        let contract_by_tx = Contracts::get_by_transaction_id(&conn, 1)?;
        assert_eq!(contract_by_tx.id, contract.id);
        assert_eq!(contract_by_tx.address, contract.address);
        assert_eq!(contract_by_tx.signers.0, contract.signers.0);
        assert_eq!(contract_by_tx.transaction_id, contract.transaction_id);

        Ok(())
    }

    #[test]
    fn test_multiple_transactions() -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = initialize_db()?;
        
        // Create test data
        let sender1 = AddressSqlite::from(Address::from_str("0x0000000000000000000000000000000000000001").unwrap());
        let sender2 = AddressSqlite::from(Address::from_str("0x0000000000000000000000000000000000000002").unwrap());
        
        let test_transactions = vec![
            Transactions {
                id: 0,
                sender: sender1,
                transaction_type: TransactionType::CreateToken,
                data: b"token1".to_vec(),
                timestamp: 1000,
            },
            Transactions {
                id: 0,
                sender: sender1,
                transaction_type: TransactionType::Mint,
                data: b"mint1".to_vec(),
                timestamp: 1001,
            },
            Transactions {
                id: 0,
                sender: sender2,
                transaction_type: TransactionType::CreateToken,
                data: b"token2".to_vec(),
                timestamp: 1002,
            },
            Transactions {
                id: 0,
                sender: sender2,
                transaction_type: TransactionType::Transfer,
                data: b"transfer1".to_vec(),
                timestamp: 1003,
            },
        ];

        // Insert all transactions
        for tx in &test_transactions {
            insert_transaction(&mut conn, tx)?;
        }

        // Test different query methods
        
        // 1. Get all CreateToken transactions
        let create_txs = Transactions::get_by_type(&conn, TransactionType::CreateToken)?;
        assert_eq!(create_txs.len(), 2);
        assert!(create_txs.iter().all(|tx| tx.transaction_type == TransactionType::CreateToken));

        // 2. Get all transactions from sender1
        let sender1_txs = Transactions::get_by_sender(&conn, sender1)?;
        assert_eq!(sender1_txs.len(), 2);
        assert!(sender1_txs.iter().all(|tx| tx.sender == sender1));

        // 3. Get CreateToken transactions from sender2
        let sender2_create_txs = Transactions::get_by_type_and_sender(
            &conn,
            TransactionType::CreateToken,
            sender2
        )?;
        assert_eq!(sender2_create_txs.len(), 1);
        assert_eq!(sender2_create_txs[0].data, b"token2");

        // 4. Get transactions after timestamp 1001
        let recent_txs = Transactions::get_by_type_after_timestamp(
            &conn,
            TransactionType::CreateToken,
            1001
        )?;
        assert_eq!(recent_txs.len(), 1);
        assert_eq!(recent_txs[0].sender, sender2);

        // 5. Verify we can get each transaction by ID
        for i in 1..=4 {
            let tx = Transactions::get_by_id(&conn, i)?;
            println!("Transaction {}: {:?}", i, tx);
        }

        Ok(())
    }
}
