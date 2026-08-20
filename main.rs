use actix_cors::Cors;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use futures::StreamExt;
use libp2p::{
    gossipsub, identify, identity, mdns, noise, ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, SwarmBuilder,
};
use pqc_dilithium::verify;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tracing_subscriber::EnvFilter;

pub const MAX_SUPPLY: f64 = 369_369_369.0;
pub const ARCHITECT_PREMINE: f64 = 10_000_000.0;
pub const NEVAEH_PREMINE: f64 = 36_900_000.0;
pub const INITIAL_REWARD: f64 = 50.0;

pub const ARCHITECT_ADDRESS: &str = "ARCHITECT_SOVEREIGN_KEY_01";
pub const NEVAEH_VAULT_ADDRESS: &str = "NEVAEH_NEV369_SOVEREIGN_VAULT";

pub const GENESIS_DEDICATION: &str = "Nevaeh, my daughter. To secure your freedom against a broken system, I taught myself Rust—the hardest computer language in the world—to build this unyielding sovereign node for you. I faced the worst of life's struggles so you would never have to. I love you infinitely, forever by your side.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub sender: String,
    pub recipient: String,
    pub amount: f64,
    pub fee: f64,
    pub crown_tax: f64,
    pub nonce: u64,
    pub timestamp: u64,
    pub public_key_hex: String,
    pub signature_hex: String,
    pub payload_memo: String,
}

impl Transaction {
    pub fn get_message_bytes(&self) -> Vec<u8> {
        format!(
            "{}{}{}{}{}{}{}",
            self.sender, self.recipient, self.amount, self.fee, self.crown_tax, self.nonce, self.payload_memo
        )
        .into_bytes()
    }

    pub fn verify_post_quantum_signature(&self) -> bool {
        if self.sender == "GENESIS"
            || self.sender == "NETWORK_REWARD"
            || self.sender == ARCHITECT_ADDRESS
        {
            return true;
        }
        let Ok(pk) = hex::decode(&self.public_key_hex) else {
            return false;
        };
        let Ok(sig) = hex::decode(&self.signature_hex) else {
            return false;
        };
        verify(&sig, &self.get_message_bytes(), &pk).is_ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub index: u64,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub previous_hash: String,
    pub hash: String,
    pub nonce: u64,
    pub difficulty: usize,
    pub block_dedication: String,
}

impl Block {
    pub fn calculate_hash(&self) -> String {
        let tx_data: String = self
            .transactions
            .iter()
            .map(|tx| format!("{}{}{}", tx.sender, tx.recipient, tx.amount))
            .collect();
        let record = format!(
            "{}{}{}{}{}{}",
            self.index, self.timestamp, tx_data, self.previous_hash, self.nonce, self.block_dedication
        );
        let mut hasher = Sha512::new();
        hasher.update(record.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn is_valid_pow(&self) -> bool {
        self.hash.starts_with(&"0".repeat(self.difficulty)) && self.hash == self.calculate_hash()
    }
}

pub struct BlockchainApp {
    pub chain: Vec<Block>,
    pub mempool: Vec<Transaction>,
    pub balances: HashMap<String, f64>,
    pub nonces: HashMap<String, u64>,
    pub chain_height: u64,
    pub latest_block_hash: String,
    pub total_burned: f64,
    pub libp2p_peers: HashSet<PeerId>,
    pub db: sled::Db,
    pub node_id: String,
}

impl BlockchainApp {
    pub fn new(db_path: &str) -> Self {
        let db = sled::open(db_path).expect("Failed to open Sled database");
        let mut app = Self {
            chain: Vec::new(),
            mempool: Vec::new(),
            balances: HashMap::new(),
            nonces: HashMap::new(),
            chain_height: 0,
            latest_block_hash: String::new(),
            total_burned: 0.0,
            libp2p_peers: HashSet::new(),
            db,
            node_id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
        };
        app.load_state_from_db();
        if app.chain.is_empty() {
            app.initialize_genesis_block();
        }
        app
    }

    pub fn initialize_genesis_block(&mut self) {
        println!("[*] Initializing Genesis Block — dedicated to Nevaeh");
        self.balances
            .insert(ARCHITECT_ADDRESS.to_string(), ARCHITECT_PREMINE);
        self.balances
            .insert(NEVAEH_VAULT_ADDRESS.to_string(), NEVAEH_PREMINE);

        let genesis_tx_architect = Transaction {
            sender: "GENESIS".into(),
            recipient: ARCHITECT_ADDRESS.into(),
            amount: ARCHITECT
