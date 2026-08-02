use actix_cors::Cors;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use pqc_dilithium::{verify, Keypair};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

pub const MAX_SUPPLY: f64 = 369_369_369.0;
pub const ARCHITECT_PREMINE: f64 = 10_000_000.0;
pub const NEVAEH_PREMINE: f64 = 36_900_000.0;
pub const INITIAL_REWARD: f64 = 50.0;
pub const CROWN_TAX_RATE: f64 = 0.0369;

pub const ARCHITECT_ADDRESS: &str = "ARCHITECT_SOVEREIGN_KEY_01";
pub const NEVAEH_VAULT_ADDRESS: &str = "NEVAEH_NEV369_SOVEREIGN_VAULT";
pub const BURN_ADDRESS: &str = "NEV369_PROOF_OF_BURN_INCINERATOR";

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
        format!("{:x}", sha256::digest(record.as_bytes()))
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
        println!("[*] Initializing NEV369 Genesis Block with Sovereign Family Inscription...");
        self.balances.insert(ARCHITECT_ADDRESS.to_string(), ARCHITECT_PREMINE);
        self.balances.insert(NEVAEH_VAULT_ADDRESS.to_string(), NEVAEH_PREMINE);
        self.nonces.insert(ARCHITECT_ADDRESS.to_string(), 0);
        self.nonces.insert(NEVAEH_VAULT_ADDRESS.to_string(), 0);

        let genesis_tx = Transaction {
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

        let mut genesis_block = Block {
            index: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            transactions: vec![genesis_tx],
            previous_hash: "0".to_string(),
            hash: String::new(),
            nonce: 0,
            difficulty: 2,
            block_dedication: GENESIS_DEDICATION.to_string(),
        };
        genesis_block.hash = genesis_block.calculate_hash();

        self.chain.push(genesis_block);
        self.chain_height = 0;
        self.latest_block_hash = self.chain[0].hash.clone();
        self.persist_state_to_db();
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
    }
}

pub type SharedState = Arc<RwLock<BlockchainApp>>;

#[get("/")]
async fn dashboard() -> impl Responder {
    let html = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>NEV369 | Sovereign Node Console</title>
    <link href="https://fonts.googleapis.com/css2?family=Cinzel:wght@600;700&family=JetBrains+Mono:wght@400;700&display=swap" rel="stylesheet">
    <style>
        :root {
            --bg-color: #050505;
            --panel-bg: #0f0f0f;
            --border-gold: #c5a059;
            --border-subtle: #26221d;
            --text-main: #f3f4f6;
            --text-muted: #9ca3af;
            --gold: #d4af37;
            --gold-hover: #f3e5ab;
            --accent-green: #10b981;
        }
        body {
            background-color: var(--bg-color);
            color: var(--text-main);
            font-family: 'JetBrains Mono', monospace;
            margin: 0;
            padding: 0;
            display: flex;
            flex-direction: column;
            min-height: 100vh;
        }
        nav {
            background: rgba(15, 15, 15, 0.95);
            border-bottom: 1px solid var(--border-gold);
            padding: 15px 40px;
            display: flex;
            justify-content: space-between;
            align-items: center;
            position: sticky;
            top: 0;
            z-index: 1000;
            backdrop-filter: blur(10px);
        }
        .nav-brand {
            font-family: 'Cinzel', serif;
            color: var(--gold);
            font-size: 1.4rem;
            font-weight: bold;
            letter-spacing: 2px;
            text-decoration: none;
        }
        .nav-links {
            display: flex;
            gap: 25px;
            list-style: none;
            margin: 0;
            padding: 0;
        }
        .nav-links a {
            color: var(--text-muted);
            text-decoration: none;
            font-size: 0.85rem;
            text-transform: uppercase;
            letter-spacing: 1px;
            transition: color 0.3s ease;
            cursor: pointer;
        }
        .nav-links a:hover, .nav-links a.active {
            color: var(--gold);
        }
        .main-container {
            flex: 1;
            padding: 30px;
            max-width: 1400px;
            margin: 0 auto;
            width: 100%;
            box-sizing: border-box;
        }
        header {
            text-align: center;
            margin-bottom: 25px;
            border-bottom: 1px solid var(--border-subtle);
            padding-bottom: 20px;
        }
        h1 {
            font-family: 'Cinzel', serif;
            color: var(--gold);
            font-size: 2.5rem;
            margin: 0;
            letter-spacing: 2px;
        }
        p.subtitle {
            color: var(--text-muted);
            font-size: 0.9rem;
            margin-top: 5px;
            text-transform: uppercase;
            letter-spacing: 4px;
        }
        .dedication-banner {
            background: linear-gradient(135deg, rgba(20,20,20,0.95), rgba(35,28,15,0.95));
            border: 1px solid var(--border-gold);
            border-radius: 6px;
            padding: 20px 30px;
            margin-bottom: 30px;
            text-align: center;
            box-shadow: 0 4px 25px rgba(212, 175, 55, 0.15);
        }
        .dedication-banner h3 {
            font-family: 'Cinzel', serif;
            color: var(--gold);
            margin-top: 0;
            margin-bottom: 10px;
            font-size: 1.1rem;
            letter-spacing: 1px;
        }
        .dedication-banner p {
            color: var(--text-main);
            font-style: italic;
            font-size: 0.95rem;
            line-height: 1.6;
            margin: 0;
        }
        .tab-content {
            display: none;
        }
        .tab-content.active {
            display: block;
        }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
            gap: 20px;
        }
        .card {
            background: var(--panel-bg);
            border: 1px solid var(--border-gold);
            border-radius: 6px;
            padding: 25px;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.8);
            position: relative;
        }
        .card::before {
            content: '';
            position: absolute;
            top: 0; left: 0; width: 100%; height: 2px;
            background: linear-gradient(90deg, transparent, var(--gold), transparent);
        }
        h2 {
            font-family: 'Cinzel', serif;
            color: var(--gold);
            font-size: 1.2rem;
            margin-top: 0;
            border-bottom: 1px solid var(--border-subtle);
            padding-bottom: 10px;
        }
        label {
            display: block;
            color: var(--text-muted);
            font-size: 0.8rem;
            text-transform: uppercase;
            letter-spacing: 1px;
            margin-top: 12px;
            margin-bottom: 5px;
        }
        input, textarea, select {
            width: 100%;
            background: #000;
            border: 1px solid var(--border-subtle);
            color: var(--text-main);
            padding: 10px;
            border-radius: 4px;
            font-family: 'JetBrains Mono', monospace;
            box-sizing: border-box;
            font-size: 0.9rem;
        }
        input:focus, textarea:focus {
            border-color: var(--gold);
            outline: none;
        }
        button {
            background: linear-gradient(135deg, #b8860b, #d4af37);
            color: #000;
            border: none;
            padding: 12px 20px;
            cursor: pointer;
            border-radius: 4px;
            font-weight: bold;
            font-family: 'JetBrains Mono', monospace;
            width: 100%;
            text-transform: uppercase;
            letter-spacing: 1px;
            transition: all 0.3s ease;
            margin-top: 15px;
        }
        button:hover {
            background: var(--gold-hover);
            box-shadow: 0 0 15px rgba(212, 175, 55, 0.4);
        }
        pre {
            background: #000;
            border: 1px solid var(--border-subtle);
            padding: 15px;
            border-radius: 4px;
            overflow-x: auto;
            color: var(--gold);
            font-size: 0.85rem;
            max-height: 250px;
            margin-top: 15px;
        }
        .stat-row {
            display: flex;
            justify-content: space-between;
            margin: 10px 0;
            border-bottom: 1px dashed var(--border-subtle);
            padding-bottom: 8px;
        }
        .stat-label { color: var(--text-muted); }
        .stat-value { color: var(--gold); font-weight: bold; }
        .status-badge {
            display: inline-block;
            padding: 4px 10px;
            border-radius: 4px;
            font-size: 0.75rem;
            font-weight: bold;
            background: rgba(16, 185, 129, 0.15);
            color: var(--accent-green);
            border: 1px solid var(--accent-green);
        }
        footer {
            background: #080808;
            border-top: 1px solid var(--border-subtle);
            text-align: center;
            padding: 20px;
            color: var(--text-muted);
            font-size: 0.8rem;
            letter-spacing: 1px;
            margin-top: auto;
        }
        footer span {
            color: var(--gold);
        }
    </style>
</head>
<body>
    <nav>
        <a href="#" class="nav-brand" onclick="switchTab('console')">NEV369 NODE</a>
        <ul class="nav-links">
            <li><a onclick="switchTab('console')" id="nav-console" class="active">Console</a></li>
            <li><a onclick="switchTab('wallet')" id="nav-wallet">Wallet Gen</a></li>
            <li><a onclick="switchTab('transfer')" id="nav-transfer">Transfer</a></li>
            <li><a onclick="switchTab('mining')" id="nav-mining">Mining Console</a></li>
            <li><a onclick="switchTab('explorer')" id="nav-explorer">Explorer</a></li>
        </ul>
    </nav>
    <div class="main-container">
        <header>
            <h1>NEV369 Sovereign Node</h1>
            <p class="subtitle">Post-Quantum Layer-1 Infrastructure</p>
        </header>
        <div class="dedication-banner">
            <h3>Genesis Block Dedication &mdash; Built Against The System</h3>
            <p>"Nevaeh, my daughter. To secure your freedom against a broken system, I taught myself Rust&mdash;the hardest computer language in the world&mdash;to build this unyielding sovereign node for you. I faced the worst of life's struggles so you would never have to. I love you infinitely, forever by your side."</p>
        </div>
        <div id="tab-console" class="tab-content active">
            <div class="grid">
                <div class="card">
                    <h2>Node Metrics</h2>
                    <div class="stat-row"><span class="stat-label">Node Status</span><span class="stat-value"><span class="status-badge">ONLINE & SECURE</span></span></div>
                    <div class="stat-row"><span class="stat-label">Chain Height</span><span class="stat-value" id="metric-height">-</span></div>
                    <div class="stat-row"><span class="stat-label">Max Supply</span><span class="stat-value">369,369,369.00</span></div>
                    <div class="stat-row"><span class="stat-label">Mempool Txs</span><span class="stat-value" id="metric-mempool">-</span></div>
                    <div class="stat-row"><span class="stat-label">Total Burned</span><span class="stat-value" id="metric-burned">-</span></div>
                    <button onclick="fetchInfo()">Refresh Metrics</button>
                </div>
            </div>
        </div>
        <div id="tab-wallet" class="tab-content">
            <div class="card" style="max-width: 800px; margin: 0 auto;">
                <h2>Post-Quantum Wallet Generator</h2>
                <button onclick="generateKeys()">Generate Sovereign Keypair</button>
                <pre id="keys-box">Awaiting sovereign credential generation request...</pre>
            </div>
        </div>
        <div id="tab-transfer" class="tab-content">
            <div class="card" style="max-width: 800px; margin: 0 auto;">
                <h2>Sovereign Transaction Broadcast</h2>
                <label>Sender Address</label><input type="text" id="tx-sender" value="ARCHITECT_SOVEREIGN_KEY_01">
                <label>Recipient Address</label><input type="text" id="tx-recipient" placeholder="Recipient Sovereign Address">
                <label>Amount</label><input type="number" id="tx-amount" value="10.0">
                <label>Nonce</label><input type="number" id="tx-nonce" value="1">
                <label>Public Key (Hex)</label><textarea id="tx-pubkey" rows="2"></textarea>
                <label>Signature (Hex)</label><textarea id="tx-sig" rows="2"></textarea>
                <button onclick="submitTx()">Broadcast Transaction</button>
                <pre id="tx-response-box">Awaiting transaction response...</pre>
            </div>
        </div>
        <div id="tab-mining" class="tab-content">
            <div class="card" style="max-width: 800px; margin: 0 auto;">
                <h2>Sovereign Block Mining Console</h2>
                <label>Miner Sovereign Address</label><input type="text" id="mine-address" value="ARCHITECT_SOVEREIGN_KEY_01">
                <button onclick="mineBlock()">Mine New Block</button>
                <pre id="mine-response-box">Awaiting block mining...</pre>
            </div>
        </div>
        <div id="tab-explorer" class="tab-content">
            <div class="card">
                <h2>Blockchain Explorer</h2>
                <button onclick="fetchExplorer()">Refresh Chain Data</button>
                <div id="explorer-content"><pre>Loading chain ledger...</pre></div>
            </div>
        </div>
    </div>
    <footer>&copy; 2026 NEV369 Sovereign Infrastructure. Built for <span>Nevaeh</span> in Rust.</footer>
    <script>
        function switchTab(id) {
            document.querySelectorAll('.tab-content').forEach(e => e.classList.remove('active'));
            document.querySelectorAll('.nav-links a').forEach(e => e.classList.remove('active'));
            document.getElementById('tab-' + id).classList.add('active');
            if(id=='explorer') fetchExplorer();
        }
        async function fetchInfo() {
            const res = await fetch('/info'); const data = await res.json();
            document.getElementById('metric-height').innerText = data.chain_height;
            document.getElementById('metric-mempool').innerText = data.mempool_size;
            document.getElementById('metric-burned').innerText = data.total_burned.toFixed(2);
        }
        async function generateKeys() {
            const res = await fetch('/keys/generate', { method: 'POST' }); const data = await res.json();
            document.getElementById('keys-box').innerText = JSON.stringify(data, null, 2);
            document.getElementById('tx-pubkey').value = data.public_key_hex;
        }
        async function submitTx() {
            const body = {
                sender: document.getElementById('tx-sender').value,
                recipient: document.getElementById('tx-recipient').value,
                amount: parseFloat(document.getElementById('tx-amount').value),
                fee: 0.369, crown_tax: 0.369,
                nonce: parseInt(document.getElementById('tx-nonce').value),
                timestamp: Math.floor(Date.now() / 1000),
                public_key_hex: document.getElementById('tx-pubkey').value,
                signature_hex: document.getElementById('tx-sig').value,
                payload_memo: "Sovereign Transfer"
            };
            const res = await fetch('/tx/submit', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body) });
            document.getElementById('tx-response-box').innerText = JSON.stringify(await res.json(), null, 2);
        }
        async function mineBlock() {
            const res = await fetch('/mine', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ miner_address: document.getElementById('mine-address').value }) });
            document.getElementById('mine-response-box').innerText = JSON.stringify(await res.json(), null, 2);
            fetchInfo();
        }
        async function fetchExplorer() {
            const res = await fetch('/info'); const data = await res.json();
            document.getElementById('explorer-content').innerHTML = `<pre>${JSON.stringify(data, null, 2)}</pre>`;
        }
        fetchInfo();
    </script>
</body>
</html>"##;
    HttpResponse::Ok().content_type("text/html").body(html)
}

#[get("/info")]
async fn get_info(data: web::Data<SharedState>) -> impl Responder {
    let app = data.read().await;
    HttpResponse::Ok().json(serde_json::json!({
        "chain_height": app.chain_height,
        "max_supply": MAX_SUPPLY,
        "total_burned": app.total_burned,
        "mempool_size": app.mempool.len(),
        "genesis_dedication": GENESIS_DEDICATION
    }))
}

#[post("/keys/generate")]
async fn generate_keys() -> impl Responder {
    let kp = Keypair::generate();
    HttpResponse::Ok().json(serde_json::json!({
        "public_key_hex": hex::encode(&kp.public),
        "secret_key_hex": hex::encode(&kp.secret)
    }))
}

#[post("/tx/submit")]
async fn submit_transaction(data: web::Data<SharedState>, tx: web::Json<Transaction>) -> impl Responder {
    let mut app = data.write().await;
    if !tx.verify_post_quantum_signature() {
        return HttpResponse::BadRequest().json(serde_json::json!({ "status": "error", "message": "Invalid post-quantum Dilithium signature!" }));
    }
    app.mempool.push(tx.into_inner());
    HttpResponse::Ok().json(serde_json::json!({ "status": "success", "message": "Transaction added to mempool." }))
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
    let mut new_block = Block {
        index: app.chain_height,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        transactions: block_txs,
        previous_hash,
        hash: String::new(),
        nonce: 0,
        difficulty: 2,
        block_dedication: format!("Block #{}", app.chain_height),
    };

    loop {
        new_block.hash = new_block.calculate_hash();
        if new_block.hash.starts_with(&"0".repeat(new_block.difficulty)) { break; }
        new_block.nonce += 1;
    }

    app.latest_block_hash = new_block.hash.clone();
    app.chain.push(new_block);
    app.mempool.clear();
    app.persist_state_to_db();

    HttpResponse::Ok().json(serde_json::json!({ "status": "success", "block_height": app.chain_height }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::fs::create_dir_all("./data").ok();
    let app_state = Arc::new(RwLock::new(BlockchainApp::new("./data/nev369.db")));
    println!("[*] NEV369 Sovereign Node running on http://127.0.0.1:8080");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(Cors::permissive())
            .service(dashboard)
            .service(get_info)
            .service(generate_keys)
            .service(submit_transaction)
            .service(mine_block)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
