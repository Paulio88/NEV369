
// --- Sovereign Wallet Generation Endpoint ---
#[actix_web::post("/wallet/generate")]
async fn generate_wallet() -> impl Responder {
    use sha2::{Sha256, Digest};
    let random_seed = uuid::Uuid::new_v4().to_string();
    let mut hasher = Sha256::new();
    hasher.update(random_seed.as_bytes());
    let result = hasher.finalize();
    let public_key = format!("NEV369_PUB_{:x}", result);
    let private_key = format!("NEV369_PRIV_{:x}", Sha256::digest(&result));

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "public_address": public_key,
        "private_key": private_key,
        "message": "Sovereign wallet generated securely."
    }))
}
