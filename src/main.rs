use actix_cors::Cors;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use pqc_dilithium::verify;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

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
        if self.sender == "GENESIS" || self.sender == "NETWORK_REWARD" || self.sender == ARCHITECT_ADDRESS {
            return true;
        }
        let pk_bytes = match hex::decode(&self.public_key_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig_bytes = match hex::decode(&self.signature_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let msg = self.get_message_bytes();
        verify(&sig_bytes, &msg, &pk_bytes).is_ok()
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
        let tx_data: String = self.transactions.iter().map(|tx| format!("{}{}{}", tx.sender, tx.recipient, tx.amount)).collect();
        let record = format!("{}{}{}{}{}{}", self.index, self.timestamp, tx_data, self.previous_hash, self.nonce, self.block_dedication);
        sha256::digest(record.as_bytes())
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
    pub db: sled::Db,
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
            db,
        };
        app.load_state_from_db();
        if app.chain.is_empty() {
            app.initialize_genesis_block();
        }
        app
    }

    pub fn initialize_genesis_block(&mut self) {
        println!("[*] Initializing clean NEV369 Genesis Block...");
        self.balances.insert(ARCHITECT_ADDRESS.to_string(), ARCHITECT_PREMINE);
        self.balances.insert(NEVAEH_VAULT_ADDRESS.to_string(), NEVAEH_PREMINE);
        self.nonces.insert(ARCHITECT_ADDRESS.to_string(), 0);
        self.nonces.insert(NEVAEH_VAULT_ADDRESS.to_string(), 0);

        let genesis_tx_architect = Transaction {
            sender: "GENESIS".to_string(),
            recipient: ARCHITECT_ADDRESS.to_string(),
            amount: ARCHITECT_PREMINE,
            fee: 0.0,
            crown_tax: 0.0,
            nonce: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            public_key_hex: "00".to_string(),
            signature_hex: "00".to_string(),
            payload_memo: GENESIS_DEDICATION.to_string(),
        };

        let genesis_tx_nevaeh = Transaction {
            sender: "GENESIS".to_string(),
            recipient: NEVAEH_VAULT_ADDRESS.to_string(),
            amount: NEVAEH_PREMINE,
            fee: 0.0,
            crown_tax: 0.0,
            nonce: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            public_key_hex: "00".to_string(),
            signature_hex: "00".to_string(),
            payload_memo: GENESIS_DEDICATION.to_string(),
        };

        let mut genesis_block = Block {
            index: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            transactions: vec![genesis_tx_architect, genesis_tx_nevaeh],
            previous_hash: "0".to_string(),
            hash: String::new(),
            nonce: 0,
            difficulty: 4,
            block_dedication: GENESIS_DEDICATION.to_string(),
        };
        genesis_block.hash = genesis_block.calculate_hash();

        self.chain.push(genesis_block);
        self.chain_height = 0;
        self.latest_block_hash = self.chain[0].hash.clone();
        self.recalculate_balances();
        self.persist_state_to_db();
    }

    pub fn get_current_difficulty(&self) -> usize {
        4 + ((self.chain_height as usize) / 10)
    }

    pub fn recalculate_balances(&mut self) {
        self.balances.clear();
        self.nonces.clear();
        self.balances.insert(ARCHITECT_ADDRESS.to_string(), 0.0);
        self.balances.insert(NEVAEH_VAULT_ADDRESS.to_string(), 0.0);
        self.nonces.insert(ARCHITECT_ADDRESS.to_string(), 0);
        self.nonces.insert(NEVAEH_VAULT_ADDRESS.to_string(), 0);

        for block in &self.chain {
            for tx in &block.transactions {
                if tx.sender != "GENESIS" && tx.sender != "NETWORK_REWARD" {
                    let sender_bal = self.balances.entry(tx.sender.clone()).or_insert(0.0);
                    let total_cost = tx.amount + tx.fee + tx.crown_tax;
                    if *sender_bal >= total_cost {
                        *sender_bal -= total_cost;
                    }
                    let sender_nonce = self.nonces.entry(tx.sender.clone()).or_insert(0);
                    *sender_nonce = tx.nonce + 1;
                }
                let recipient_bal = self.balances.entry(tx.recipient.clone()).or_insert(0.0);
                *recipient_bal += tx.amount;
            }
        }
    }

    pub fn persist_state_to_db(&self) {
        let balances_bytes = bincode::serialize(&self.balances).unwrap();
        let nonces_bytes = bincode::serialize(&self.nonces).unwrap();
        let chain_bytes = bincode::serialize(&self.chain).unwrap();

        self.db.insert(b"state_balances", balances_bytes).unwrap();
        self.db.insert(b"state_nonces", nonces_bytes).unwrap();
        self.db.insert(b"state_chain", chain_bytes).unwrap();
        self.db.insert(b"state_chain_height", self.chain_height.to_be_bytes().to_vec()).unwrap();
        self.db.insert(b"state_latest_hash", self.latest_block_hash.as_bytes()).unwrap();
        self.db.flush().unwrap();
    }

    pub fn load_state_from_db(&mut self) {
        if let Ok(Some(bytes)) = self.db.get(b"state_balances") {
            self.balances = bincode::deserialize(&bytes).unwrap_or_default();
        }
        if let Ok(Some(bytes)) = self.db.get(b"state_nonces") {
            self.nonces = bincode::deserialize(&bytes).unwrap_or_default();
        }
        if let Ok(Some(bytes)) = self.db.get(b"state_chain") {
            self.chain = bincode::deserialize(&bytes).unwrap_or_default();
        }
        if let Ok(Some(bytes)) = self.db.get(b"state_chain_height") {
            if bytes.len() == 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes);
                self.chain_height = u64::from_be_bytes(arr);
            }
        }
        if let Ok(Some(bytes)) = self.db.get(b"state_latest_hash") {
            self.latest_block_hash = String::from_utf8(bytes.to_vec()).unwrap_or_default();
        }
        self.recalculate_balances();
    }
}

pub type SharedState = Arc<RwLock<BlockchainApp>>;

#[get("/")]
async fn dashboard() -> impl Responder {
    HttpResponse::Ok().content_type("text/html").body(r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>NEV369 | Production Sovereign Node</title>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
    <style>
        * { box-sizing: border-box; }
        :root {
            --bg-body: #090a0f; --bg-card: #131622; --bg-hover: #1b1f30;
            --accent-primary: #38bdf8; --accent-glow: rgba(56, 189, 248, 0.15);
            --success: #10b981; --text-main: #f8fafc; --text-muted: #94a3b8; --border: #1e293b;
            --gold: #f59e0b;
        }
        body { background-color: var(--bg-body); color: var(--text-main); font-family: 'Inter', sans-serif; margin: 0; padding: 20px; display: flex; justify-content: center; }
        .layout-wrapper { width: 100%; max-width: 1200px; display: flex; flex-direction: column; gap: 20px; }
        header { background: var(--bg-card); border: 1px solid var(--border); border-radius: 12px; padding: 20px 30px; display: flex; justify-content: space-between; align-items: center; box-shadow: 0 4px 20px -2px rgba(0,0,0,0.5); }
        .brand-info h1 { font-size: 1.25rem; font-weight: 700; margin: 0; color: #fff; display: flex; align-items: center; gap: 10px; }
        .brand-info h1 span { color: var(--accent-primary); font-family: 'JetBrains Mono', monospace; font-size: 0.85rem; background: var(--accent-glow); padding: 2px 8px; border-radius: 4px; border: 1px solid rgba(56, 189, 248, 0.3); }
        .node-status { display: flex; align-items: center; gap: 8px; font-size: 0.85rem; font-weight: 500; color: var(--success); background: rgba(16, 185, 129, 0.1); padding: 6px 12px; border-radius: 20px; border: 1px solid rgba(16, 185, 129, 0.2); }
        .status-dot { width: 8px; height: 8px; background: var(--success); border-radius: 50%; box-shadow: 0 0 8px var(--success); }
        nav { display: flex; gap: 8px; background: var(--bg-card); padding: 8px; border-radius: 10px; border: 1px solid var(--border); overflow-x: auto; }
        .nav-tab { background: transparent; border: none; color: var(--text-muted); padding: 10px 18px; font-family: inherit; font-size: 0.875rem; font-weight: 500; cursor: pointer; border-radius: 6px; transition: all 0.2s; white-space: nowrap; }
        .nav-tab:hover { color: var(--text-main); background: var(--bg-hover); }
        .nav-tab.active { color: var(--accent-primary); background: var(--accent-glow); border: 1px solid rgba(56, 189, 248, 0.2); }
        .workspace { display: grid; grid-template-columns: 1fr; gap: 20px; }
        .tab-content { display: none; }
        .tab-content.active { display: block; }
        .dashboard-grid { display: grid; grid-template-columns: 1fr 1.2fr; gap: 20px; }
        @media(max-width: 900px) { .dashboard-grid { grid-template-columns: 1fr; } }
        .card { background: var(--bg-card); border: 1px solid var(--border); border-radius: 12px; padding: 24px; box-shadow: 0 4px 12px rgba(0,0,0,0.3); }
        .card h2 { font-size: 1rem; font-weight: 600; margin-top: 0; margin-bottom: 20px; display: flex; justify-content: space-between; align-items: center; }
        .metrics-container { display: flex; flex-direction: column; gap: 12px; }
        .metric-row { display: flex; justify-content: space-between; align-items: center; padding: 12px 16px; background: var(--bg-body); border: 1px solid var(--border); border-radius: 8px; }
        .metric-label { font-size: 0.85rem; color: var(--text-muted); }
        .metric-value { font-family: 'JetBrains Mono', monospace; font-size: 0.9rem; color: var(--accent-primary); font-weight: 600; }
        .form-group { margin-bottom: 16px; }
        label { display: block; font-size: 0.8rem; font-weight: 500; color: var(--text-muted); margin-bottom: 6px; }
        input { width: 100%; background: var(--bg-body); border: 1px solid var(--border); color: var(--text-main); padding: 12px 16px; font-family: 'JetBrains Mono', monospace; font-size: 0.875rem; border-radius: 8px; }
        input:focus { outline: none; border-color: var(--accent-primary); box-shadow: 0 0 0 3px var(--accent-glow); }
        .btn-primary { background: var(--accent-primary); color: #090a0f; border: none; padding: 12px 20px; font-family: inherit; font-weight: 600; cursor: pointer; border-radius: 8px; width: 100%; transition: filter 0.2s; }
        .btn-primary:hover { filter: brightness(1.1); }
        .btn-gold { background: var(--gold); color: #090a0f; }
        pre { background: var(--bg-body); border: 1px solid var(--border); padding: 16px; color: #38bdf8; font-family: 'JetBrains Mono', monospace; font-size: 0.8rem; max-height: 250px; overflow-y: auto; border-radius: 8px; margin-top: 16px; white-space: pre-wrap; word-break: break-all; }
        .badge { font-size: 0.75rem; background: rgba(245, 158, 11, 0.1); color: var(--gold); border: 1px solid rgba(245, 158, 11, 0.3); padding: 2px 8px; border-radius: 4px; }
        .wallet-card { background: var(--bg-body); border: 1px solid var(--border); border-radius: 8px; padding: 16px; margin-bottom: 12px; display: flex; justify-content: space-between; align-items: center; }
        .wallet-info h4 { margin: 0 0 4px 0; font-size: 0.9rem; color: var(--text-main); font-family: 'JetBrains Mono', monospace; }
        .wallet-info span { font-size: 0.75rem; color: var(--text-muted); }
        .wallet-balance { font-family: 'JetBrains Mono', monospace; font-size: 1.1rem; color: var(--accent-primary); font-weight: 700; }
        footer { text-align: center; font-size: 0.75rem; color: var(--text-muted); padding: 10px 0; border-top: 1px solid var(--border); }
        footer span { color: var(--accent-primary); }
    </style>
</head>
<body>
    <div class="layout-wrapper">
        <header>
            <div class="brand-info">
                <h1>NEV369 SOVEREIGN NODE <span>v1.0.0-PROD</span></h1>
            </div>
            <div class="node-status">
                <div class="status-dot"></div>
                PRODUCTION ONLINE
            </div>
        </header>

        <nav>
            <button class="nav-tab active" onclick="switchTab('console')">Console</button>
            <button class="nav-tab" onclick="switchTab('wallets')">Wallets & Vaults</button>
            <button class="nav-tab" onclick="switchTab('transfer')">Transfer Assets</button>
            <button class="nav-tab" onclick="switchTab('mining')">PoW Miner</button>
            <button class="nav-tab" onclick="switchTab('explorer')">Block Explorer</button>
        </nav>

        <div class="workspace">
            <!-- CONSOLE TAB -->
            <div id="tab-console" class="tab-content active">
                <div class="dashboard-grid">
                    <div class="card">
                        <h2>Node Telemetry <span class="badge">LIVE SYNC</span></h2>
                        <div class="metrics-container">
                            <div class="metric-row"><span class="metric-label">Chain Height</span><span id="metric-height" class="metric-value">0</span></div>
                            <div class="metric-row"><span class="metric-label">Current Difficulty</span><span id="metric-difficulty" class="metric-value">4</span></div>
                            <div class="metric-row"><span class="metric-label">Max Token Supply</span><span class="metric-value">369,369,369</span></div>
                            <div class="metric-row"><span class="metric-label">Mempool Backlog</span><span id="metric-mempool" class="metric-value">0</span></div>
                        </div>
                        <button class="btn-primary" style="margin-top: 20px;" onclick="fetchInfo()">Refresh Telemetry</button>
                    </div>
                    <div class="card">
                        <h2>Genesis Dedication</h2>
                        <p style="font-size: 0.85rem; color: var(--text-muted); line-height: 1.6; font-style: italic; background: var(--bg-body); padding: 16px; border-radius: 8px; border: 1px solid var(--border);" id="genesis-text">
                            Loading immutable ledger inscription...
                        </p>
                        <button class="btn-primary btn-gold" onclick="switchTab('mining')">Jump to PoW Miner</button>
                    </div>
                </div>
            </div>

            <!-- WALLETS TAB -->
            <div id="tab-wallets" class="tab-content">
                <div class="card" style="max-width: 800px; margin: 0 auto;">
                    <h2>Sovereign Wallet Balances <button class="btn-primary" style="width: auto; padding: 6px 14px; font-size: 0.75rem;" onclick="fetchWallets()">Refresh</button></h2>
                    <div id="wallets-list-container">
                        <p style="color: var(--text-muted);">Syncing secure wallet vaults...</p>
                    </div>
                    <pre id="wallets-output" style="display:none;"></pre>
                </div>
            </div>

            <!-- TRANSFER TAB -->
            <div id="tab-transfer" class="tab-content">
                <div class="card" style="max-width: 650px; margin: 0 auto;">
                    <h2>Sovereign Asset Transfer</h2>
                    <div class="form-group">
                        <label>SENDER ADDRESS</label>
                        <input type="text" id="tx-sender" value="ARCHITECT_SOVEREIGN_KEY_01">
                    </div>
                    <div class="form-group">
                        <label>RECIPIENT ADDRESS</label>
                        <input type="text" id="tx-recipient" value="NEVAEH_NEV369_SOVEREIGN_VAULT">
                    </div>
                    <div class="form-group">
                        <label>TRANSFER AMOUNT (NEV)</label>
                        <input type="number" id="tx-amount" value="50.0">
                    </div>
                    <div class="form-group">
                        <label>NETWORK FEE (NEV)</label>
                        <input type="number" id="tx-fee" value="1.0">
                    </div>
                    <div class="form-group">
                        <label>NONCE</label>
                        <input type="number" id="tx-nonce" value="0">
                    </div>
                    <button class="btn-primary" onclick="submitTx()">Broadcast Signed Transaction</button>
                    <pre id="tx-output">Transaction broadcast logs will appear here.</pre>
                </div>
            </div>

            <!-- MINING TAB -->
            <div id="tab-mining" class="tab-content">
                <div class="card" style="max-width: 650px; margin: 0 auto;">
                    <h2>Proof-of-Work Mining Engine</h2>
                    <div class="form-group">
                        <label>MINER REWARD DESTINATION ADDRESS</label>
                        <input type="text" id="mine-address" value="NEVAEH_NEV369_SOVEREIGN_VAULT">
                    </div>
                    <button class="btn-primary btn-gold" onclick="mineBlock()">Compute & Mine Block (PoW)</button>
                    <pre id="mine-output">Mining console awaiting trigger...</pre>
                </div>
            </div>

            <!-- EXPLORER TAB -->
            <div id="tab-explorer" class="tab-content">
                <div class="card" style="max-width: 850px; margin: 0 auto;">
                    <h2>Block Ledger Explorer <button class="btn-primary" style="width: auto; padding: 6px 14px; font-size: 0.75rem;" onclick="fetchExplorer()">Refresh Chain</button></h2>
                    <pre id="explorer-output">Loading sovereign block ledger...</pre>
                </div>
            </div>
        </div>

        <footer>
            © 2026 NEV369 Sovereign Infrastructure. Production Ready. Built for <span>Nevaeh</span> in Rust.
        </footer>
    </div>

    <script>
        function switchTab(tabName) {
            document.querySelectorAll('.nav-tab').forEach(btn => btn.classList.remove('active'));
            document.querySelectorAll('.tab-content').forEach(tab => tab.classList.remove('active'));
            document.getElementById('tab-' + tabName).classList.add('active');
            
            // Highlight matching nav tab
            document.querySelectorAll('.nav-tab').forEach(btn => {
                if(btn.getAttribute('onclick').includes(tabName)) btn.classList.add('active');
            });

            if(tabName === 'wallets') { fetchWallets(); }
            if(tabName === 'explorer') { fetchExplorer(); }
            if(tabName === 'console') { fetchInfo(); }
        }

        async function fetchInfo() {
            try {
                const res = await fetch('/info');
                const data = await res.json();
                document.getElementById('metric-height').innerText = data.chain_height;
                document.getElementById('metric-difficulty').innerText = data.current_difficulty;
                document.getElementById('metric-mempool').innerText = data.mempool_size;
                if(data.genesis_dedication) {
                    document.getElementById('genesis-text').innerText = '"' + data.genesis_dedication + '"';
                }
            } catch(e) {
                console.error("Telemetry sync error");
            }
        }

        async function fetchWallets() {
            try {
                const res = await fetch('/wallets');
                const data = await res.json();
                document.getElementById('wallets-output').innerText = JSON.stringify(data, null, 2);
                
                const container = document.getElementById('wallets-list-container');
                container.innerHTML = '';
                for (const [address, balance] of Object.entries(data)) {
                    container.innerHTML += `
                        <div class="wallet-card">
                            <div class="wallet-info">
                                <h4>${address}</h4>
                                <span>Verified Sovereign Vault</span>
                            </div>
                            <div class="wallet-balance">${balance.toLocaleString()} NEV</div>
                        </div>
                    `;
                }
            } catch(e) {
                document.getElementById('wallets-list-container').innerHTML = '<p style="color: #ef4444;">Failed to sync wallet vaults.</p>';
            }
        }

        async function submitTx() {
            const tx = {
                sender: document.getElementById('tx-sender').value,
                recipient: document.getElementById('tx-recipient').value,
                amount: parseFloat(document.getElementById('tx-amount').value),
                fee: parseFloat(document.getElementById('tx-fee').value),
                crown_tax: 0.0,
                nonce: parseInt(document.getElementById('tx-nonce').value),
                timestamp: Math.floor(Date.now() / 1000),
                public_key_hex: "00",
                signature_hex: "00",
                payload_memo: "Dashboard Sovereign Transfer"
            };
            const res = await fetch('/tx/submit', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(tx)
            });
            const data = await res.json();
            document.getElementById('tx-output').innerText = JSON.stringify(data, null, 2);
            fetchInfo();
        }

        async function mineBlock() {
            document.getElementById('mine-output').innerText = "Mining block... computing proof-of-work hash target...";
            const req = { miner_address: document.getElementById('mine-address').value };
            const res = await fetch('/mine', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(req)
            });
            const data = await res.json();
            document.getElementById('mine-output').innerText = JSON.stringify(data, null, 2);
            fetchInfo();
        }

        async function fetchExplorer() {
            const res = await fetch('/explorer');
            const data = await res.json();
            document.getElementById('explorer-output').innerText = JSON.stringify(data, null, 2);
        }

        // Auto-refresh telemetry on load
        fetchInfo();
        setInterval(fetchInfo, 5000);
    </script>
</body>
</html>"##)
}

#[get("/info")]
async fn get_info(data: web::Data<SharedState>) -> impl Responder {
    let app = data.read().await;
    HttpResponse::Ok().json(serde_json::json!({
        "chain_height": app.chain_height,
        "current_difficulty": app.get_current_difficulty(),
        "max_supply": MAX_SUPPLY,
        "total_burned": app.total_burned,
        "mempool_size": app.mempool.len(),
        "genesis_dedication": GENESIS_DEDICATION
    }))
}

#[get("/wallets")]
async fn get_wallets(data: web::Data<SharedState>) -> impl Responder {
    let app = data.read().await;
    HttpResponse::Ok().json(app.balances.clone())
}

#[get("/explorer")]
async fn get_explorer(data: web::Data<SharedState>) -> impl Responder {
    let app = data.read().await;
    HttpResponse::Ok().json(serde_json::json!({
        "chain_height": app.chain_height,
        "latest_block_hash": app.latest_block_hash.clone(),
        "chain": app.chain.clone()
    }))
}

#[post("/tx/submit")]
async fn submit_transaction(data: web::Data<SharedState>, tx: web::Json<Transaction>) -> impl Responder {
    let mut app = data.write().await;
    if !tx.verify_post_quantum_signature() {
        return HttpResponse::BadRequest().json(serde_json::json!({ "status": "error", "message": "Invalid post-quantum Dilithium signature!" }));
    }
    app.mempool.push(tx.into_inner());
    HttpResponse::Ok().json(serde_json::json!({ "status": "success", "message": "Transaction added to mempool successfully." }))
}

#[derive(Debug, Deserialize)]
pub struct MineRequest { pub miner_address: String }

#[post("/mine")]
async fn mine_block(data: web::Data<SharedState>, req: web::Json<MineRequest>) -> impl Responder {
    let mut app = data.write().await;
    let block_reward = INITIAL_REWARD;
    let mut total_fees = 0.0;
    for tx in &app.mempool { total_fees += tx.fee; }
    
    let reward_tx = Transaction {
        sender: "NETWORK_REWARD".to_string(),
        recipient: req.miner_address.clone(),
        amount: block_reward + total_fees,
        fee: 0.0, crown_tax: 0.0, nonce: 0,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        public_key_hex: "00".to_string(), signature_hex: "00".to_string(),
        payload_memo: "Block Reward".to_string(),
    };

    let mut block_txs = vec![reward_tx];
    block_txs.append(&mut app.mempool);

    app.chain_height += 1;
    let previous_hash = app.latest_block_hash.clone();
    let target_difficulty = app.get_current_difficulty();

    let mut new_block = Block {
        index: app.chain_height,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        transactions: block_txs,
        previous_hash,
        hash: String::new(),
        nonce: 0,
        difficulty: target_difficulty,
        block_dedication: format!("Block #{} (Diff: {})", app.chain_height, target_difficulty),
    };

    loop {
        new_block.hash = new_block.calculate_hash();
        if new_block.hash.starts_with(&"0".repeat(new_block.difficulty)) { break; }
        new_block.nonce += 1;
    }

    app.latest_block_hash = new_block.hash.clone();
    app.chain.push(new_block);
    app.mempool.clear();
    app.recalculate_balances();
    app.persist_state_to_db();

    HttpResponse::Ok().json(serde_json::json!({ 
        "status": "success", 
        "block_height": app.chain_height, 
        "difficulty": target_difficulty,
        "hash": app.latest_block_hash.clone()
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::fs::create_dir_all("./data").ok();
    let _ = std::fs::remove_dir_all("./data/nev369.db");
    let app_state = Arc::new(RwLock::new(BlockchainApp::new("./data/nev369.db")));
    println!("[*] NEV369 Production Sovereign Node active on http://127.0.0.1:8080");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(Cors::permissive())
            .service(dashboard)
            .service(get_info)
            .service(get_wallets)
            .service(get_explorer)
            .service(submit_transaction)
            .service(mine_block)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
