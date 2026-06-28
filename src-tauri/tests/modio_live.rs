//! Live mod.io OAuth diagnostics. Run manually:
//!   cargo test -p modkistmkii --test modio_live -- --ignored --nocapture
use std::path::PathBuf;

use modkistmkii_lib::modio_api::ApiClient;

async fn build_session() -> (ApiClient, u64, String) {
    dotenvy::from_filename("../.env").ok();
    dotenvy::dotenv().ok();

    let api_key = std::env::var("MODIO_API_KEY").expect("MODIO_API_KEY");
    let game_id: u64 = std::env::var("MODIO_GAME_ID")
        .expect("MODIO_GAME_ID")
        .parse()
        .expect("MODIO_GAME_ID parse");

    let store = PathBuf::from(std::env::var("HOME").unwrap())
        .join("Library/Application Support/net.tnrd.modkistmkii/modio-auth.json");
    let store: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&store).expect("read auth store")).unwrap();
    let token = store["accessToken"].as_str().expect("accessToken").to_string();

    let client = ApiClient::new(api_key, Some(game_id), None, false).expect("client");

    (client, game_id, token)
}

async fn fetch_subscribed_paged(client: &ApiClient, token: &str, game_id: u64) -> usize {
    use modkistmkii_lib::modio_api::ModQuery;

    const LIMIT: u32 = 100;
    let mut count = 0usize;
    let mut pages = 0usize;
    let mut offset = 0u32;
    loop {
        let query = ModQuery {
            limit: LIMIT,
            offset,
            ..ModQuery::default()
        };
        let list = client
            .get_user_subscriptions(token, game_id, &query)
            .await
            .expect("subscriptions page");
        pages += 1;
        let page = list.data.len() as u32;
        count += page as usize;
        offset += page;
        if page < LIMIT || page == 0 || offset >= list.result_total {
            break;
        }
    }
    eprintln!("paged subscriptions: {count} mod(s) in {pages} HTTP request(s)");
    count
}

async fn try_subscribe(client: &ApiClient, token: &str, game_id: u64, mod_id: u64) -> bool {
    match client.subscribe(token, game_id, mod_id).await {
        Ok(()) => {
            eprintln!("subscribe {mod_id}: OK");
            true
        }
        Err(err) => {
            eprintln!(
                "subscribe {mod_id}: {} (rate_limited={}, error_ref={:?})",
                err,
                err.is_rate_limited(),
                err.error_ref
            );
            false
        }
    }
}

#[tokio::test]
#[ignore = "live mod.io API — requires .env and modio-auth.json"]
async fn diagnose_oauth_subscribe_flow() {
    use modkistmkii_lib::modio_api::ModQuery;

    let (client, game_id, token) = build_session().await;
    let query = ModQuery {
        limit: 100,
        offset: 0,
        ..ModQuery::default()
    };
    let list = client
        .get_user_subscriptions(&token, game_id, &query)
        .await
        .expect("get_user_subscriptions");
    eprintln!("subscriptions (single request): {} mod(s)", list.data.len());
    try_subscribe(&client, &token, game_id, 3_454_919).await;
}

#[tokio::test]
#[ignore = "live mod.io API — requires .env and modio-auth.json"]
async fn diagnose_startup_burst_then_subscribe() {
    use modkistmkii_lib::modio_api::ModQuery;

    let (client, game_id, token) = build_session().await;

    // Game-key burst similar to startup: list subs + metadata for each.
    let query = ModQuery {
        limit: 100,
        offset: 0,
        ..ModQuery::default()
    };
    let subs = client
        .get_user_subscriptions(&token, game_id, &query)
        .await
        .expect("subs");
    let mod_ids: Vec<u64> = subs.data.iter().map(|m| m.id).collect();
    eprintln!("loaded {} subscription id(s)", mod_ids.len());

    for mod_id in &mod_ids {
        let _ = client
            .get_mod(game_id, *mod_id, Some(&token))
            .await
            .expect("get_mod");
        let _ = client
            .get_mod_dependencies(game_id, *mod_id, Some(&token))
            .await
            .expect("deps");
    }
    eprintln!("game-key burst: {} get_mod + {} deps", mod_ids.len(), mod_ids.len());

    fetch_subscribed_paged(&client, &token, game_id).await;
    try_subscribe(&client, &token, game_id, 3_454_919).await;
}
