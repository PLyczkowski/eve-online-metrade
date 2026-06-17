use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, Utc};
use flate2::read::GzDecoder;
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::Duration as StdDuration;
use tauri::{Manager, State};

struct AppState {
    db_path: Mutex<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Filters {
    search: String,
    status: String,
    direction: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Product {
    type_id: i64,
    name: String,
    enabled: bool,
    notes: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TradeHub {
    id: i64,
    name: String,
    region_id: i64,
    station_id: i64,
    enabled: bool,
    priority: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Setting {
    key: String,
    value: String,
    notes: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Opportunity {
    status: String,
    direction: String,
    type_id: i64,
    item_name: String,
    buy_hub: String,
    sell_hub: String,
    buy_price: Option<f64>,
    sell_reference: Option<f64>,
    destination_lowest_sell: Option<f64>,
    profit_per_unit: Option<f64>,
    spread: Option<f64>,
    source_available: Option<f64>,
    estimated_profit: Option<f64>,
    cargo_used_percent: Option<f64>,
    suggested_buy_quantity: Option<f64>,
    my_destination_sell_price_min: Option<f64>,
    my_destination_sell_price_max: Option<f64>,
    my_destination_sell_quantity: Option<i64>,
    my_destination_sell_order_count: Option<i64>,
    buy_region_volume: Option<f64>,
    sell_region_volume: Option<f64>,
    last_refresh: Option<String>,
    last_refresh_minutes: Option<i64>,
    notes: String,
    script_notes: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RefreshRun {
    refresh_time: String,
    items_scanned: i64,
    opportunities_written: i64,
    api_calls: i64,
    errors: String,
    skipped: String,
    duration_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RefreshJob {
    status: String,
    kind: String,
    current_item: String,
    scanned_count: i64,
    total_count: i64,
    api_calls: i64,
    last_error: String,
    queued_count: i64,
    started_at: String,
    finished_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DiscoverySummary {
    known_items: i64,
    market_rows: i64,
    candidates: i64,
    products: i64,
    enabled_products: i64,
    last_discovery: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DiscoveryRun {
    run_time: String,
    item_types_imported: i64,
    market_rows_imported: i64,
    candidates_found: i64,
    products_enabled: i64,
    errors: String,
    duration_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ApiLimitStatus {
    last_response_at: String,
    last_status: i64,
    error_limit_remain: Option<i64>,
    error_limit_reset: Option<i64>,
    retry_after: Option<i64>,
    rate_limit_limit: String,
    rate_limit_remaining: Option<i64>,
    rate_limit_used: Option<i64>,
    rate_limited: bool,
    last_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AuthCharacter {
    character_id: i64,
    character_name: String,
    scopes: String,
    expires_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AuthEvent {
    happened_at: String,
    status: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CharacterOrder {
    character_id: i64,
    order_id: i64,
    type_id: i64,
    region_id: i64,
    location_id: i64,
    is_buy_order: bool,
    price: f64,
    volume_remain: i64,
    volume_total: i64,
    issued: String,
    duration: i64,
    range: String,
    state: String,
    refreshed_at: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    sub: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    scp: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct EsiCharacterOrder {
    order_id: i64,
    type_id: i64,
    region_id: i64,
    location_id: i64,
    #[serde(default)]
    is_buy_order: bool,
    price: f64,
    volume_remain: i64,
    volume_total: i64,
    issued: String,
    duration: i64,
    range: String,
}

#[derive(Debug, Deserialize)]
struct EsiOrder {
    location_id: i64,
    price: f64,
    volume_remain: f64,
    #[serde(default)]
    is_buy_order: bool,
}

#[derive(Debug, Deserialize)]
struct HistoryRow {
    date: String,
    volume: i64,
}

#[derive(Debug, Deserialize)]
struct EsiType {
    name: String,
    #[serde(default)]
    volume: Option<f64>,
}

struct ProductMetadata {
    product: Product,
    volume_m3: Option<f64>,
}

struct HubPrices {
    lowest_sell: f64,
    reference_sell: f64,
    available_at_lowest: f64,
}

struct HubMarketData {
    hub: TradeHub,
    prices: HubPrices,
    volume: f64,
}

struct RouteCandidate<'a> {
    buy: &'a HubMarketData,
    sell: &'a HubMarketData,
    profit: f64,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join("eve-metrade.sqlite3");
            let conn = Connection::open(&db_path)?;
            migrate(&conn)?;
            seed(&conn)?;
            seed_initial_opportunities(&conn)?;
            run_initial_discovery_if_needed(&conn);
            app.manage(AppState {
                db_path: Mutex::new(db_path),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_opportunities,
            list_products,
            list_trade_hubs,
            list_settings,
            list_refresh_runs,
            list_discovery_summary,
            list_api_limit_status,
            list_auth_characters,
            list_auth_events,
            start_eve_login,
            refresh_character_orders,
            list_character_orders,
            get_refresh_status,
            discover_hot_products,
            start_refresh_next_batch,
            start_reset_and_refresh,
            start_refresh_product,
            refresh_next_batch,
            refresh_product,
            reset_and_refresh,
            update_product_notes,
            update_setting,
            set_trade_hub_enabled,
            set_product_enabled
        ])
        .run(tauri::generate_context!())
        .expect("error while running EVE Metrade");
}

#[tauri::command]
fn list_products(state: State<AppState>) -> Result<Vec<Product>, String> {
    let conn = open(&state)?;
    let mut stmt = conn
        .prepare("select type_id, name, enabled, notes from products order by type_id")
        .map_err(to_string)?;
    let result = rows(
        stmt.query_map([], |row| {
            Ok(Product {
                type_id: row.get(0)?,
                name: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
                notes: row.get(3)?,
            })
        })
        .map_err(to_string)?,
    )?;
    Ok(result)
}

#[tauri::command]
fn list_trade_hubs(state: State<AppState>) -> Result<Vec<TradeHub>, String> {
    let conn = open(&state)?;
    enabled_or_all_trade_hubs(&conn, false)
}

#[tauri::command]
fn list_settings(state: State<AppState>) -> Result<Vec<Setting>, String> {
    let conn = open(&state)?;
    let mut stmt = conn
        .prepare("select key, value, notes from settings order by rowid")
        .map_err(to_string)?;
    let result = rows(
        stmt.query_map([], |row| {
            Ok(Setting {
                key: row.get(0)?,
                value: row.get(1)?,
                notes: row.get(2)?,
            })
        })
        .map_err(to_string)?,
    )?;
    Ok(result)
}

#[tauri::command]
fn list_refresh_runs(state: State<AppState>) -> Result<Vec<RefreshRun>, String> {
    let conn = open(&state)?;
    let mut stmt = conn.prepare("select refresh_time, items_scanned, opportunities_written, api_calls, errors, skipped, duration_seconds from refresh_runs order by rowid desc limit 100").map_err(to_string)?;
    let result = rows(
        stmt.query_map([], refresh_run_from_row)
            .map_err(to_string)?,
    )?;
    Ok(result)
}

#[tauri::command]
fn list_discovery_summary(state: State<AppState>) -> Result<DiscoverySummary, String> {
    let conn = open(&state)?;
    discovery_summary(&conn)
}

#[tauri::command]
fn list_api_limit_status(state: State<AppState>) -> Result<ApiLimitStatus, String> {
    let conn = open(&state)?;
    api_limit_status(&conn)
}

#[tauri::command]
fn list_auth_characters(state: State<AppState>) -> Result<Vec<AuthCharacter>, String> {
    let conn = open(&state)?;
    let mut stmt = conn
        .prepare("select character_id, character_name, scopes, expires_at, updated_at from auth_characters order by character_name")
        .map_err(to_string)?;
    let result = rows(
        stmt.query_map([], auth_character_from_row)
            .map_err(to_string)?,
    )?;
    Ok(result)
}

#[tauri::command]
fn list_auth_events(state: State<AppState>) -> Result<Vec<AuthEvent>, String> {
    let conn = open(&state)?;
    let mut stmt = conn
        .prepare(
            "select happened_at, status, message from auth_events order by rowid desc limit 20",
        )
        .map_err(to_string)?;
    let result = rows(
        stmt.query_map([], |row| {
            Ok(AuthEvent {
                happened_at: row.get(0)?,
                status: row.get(1)?,
                message: row.get(2)?,
            })
        })
        .map_err(to_string)?,
    )?;
    Ok(result)
}

#[tauri::command]
fn start_eve_login(state: State<AppState>) -> Result<AuthCharacter, String> {
    let conn = open(&state)?;
    start_eve_login_inner(&conn)
}

#[tauri::command]
fn refresh_character_orders(
    state: State<AppState>,
    character_id: i64,
) -> Result<Vec<CharacterOrder>, String> {
    let conn = open(&state)?;
    refresh_character_orders_inner(&conn, character_id)
}

#[tauri::command]
fn list_character_orders(
    state: State<AppState>,
    character_id: Option<i64>,
) -> Result<Vec<CharacterOrder>, String> {
    let conn = open(&state)?;
    list_character_orders_inner(&conn, character_id)
}

#[tauri::command]
fn get_refresh_status(state: State<AppState>) -> Result<RefreshJob, String> {
    let conn = open(&state)?;
    recover_stale_refresh_job(&conn)?;
    get_refresh_job(&conn)
}

#[tauri::command]
fn list_opportunities(
    state: State<AppState>,
    filters: Filters,
) -> Result<Vec<Opportunity>, String> {
    let conn = open(&state)?;
    let cargo_m3 = setting(&conn, "Ship cargo capacity m3", "7900")
        .parse::<f64>()
        .unwrap_or(7900.0)
        .max(0.0);
    let suggested_destination_volume_percent = setting(
        &conn,
        "Suggested buy max destination 30d volume percent",
        "0.3",
    )
    .parse::<f64>()
    .unwrap_or(0.3)
    .clamp(0.0, 1.0);
    let mut stmt = conn
        .prepare(
            "select coalesce(o.status, 'PENDING'),
                coalesce(o.direction, ''),
                p.type_id,
                coalesce(nullif(o.item_name, ''), p.name),
                coalesce(o.buy_hub, ''),
                coalesce(o.sell_hub, ''),
                o.buy_price,
                o.sell_reference,
                o.destination_lowest_sell,
                o.profit_per_unit,
                o.spread,
                o.source_available,
                case
                    when o.profit_per_unit is not null and coalesce(
                        o.suggested_buy_quantity,
                        case
                            when o.source_available is not null then min(
                                o.source_available,
                                coalesce(case when m.volume_m3 is not null and ?1 > 0 and m.volume_m3 > 0 then floor(?1 / m.volume_m3) end, o.source_available),
                                coalesce(case when o.sell_region_volume is not null and ?2 > 0 then floor(o.sell_region_volume * ?2) end, o.source_available)
                            )
                        end
                    ) is not null
                    then max(0.0, o.profit_per_unit * coalesce(
                        o.suggested_buy_quantity,
                        case
                            when o.source_available is not null then min(
                                o.source_available,
                                coalesce(case when m.volume_m3 is not null and ?1 > 0 and m.volume_m3 > 0 then floor(?1 / m.volume_m3) end, o.source_available),
                                coalesce(case when o.sell_region_volume is not null and ?2 > 0 then floor(o.sell_region_volume * ?2) end, o.source_available)
                            )
                        end
                    ))
                    else o.estimated_profit
                end,
                coalesce(
                    o.cargo_used_percent,
                    case
                        when o.source_available is not null and m.volume_m3 is not null and ?1 > 0 and m.volume_m3 > 0
                        then min(1.0, max(0.0, (coalesce(
                            o.suggested_buy_quantity,
                            min(
                                o.source_available,
                                coalesce(floor(?1 / m.volume_m3), o.source_available),
                                coalesce(case when o.sell_region_volume is not null and ?2 > 0 then floor(o.sell_region_volume * ?2) end, o.source_available)
                            )
                        ) * m.volume_m3) / ?1))
                    end
                ),
                coalesce(
                    o.suggested_buy_quantity,
                    case
                        when o.source_available is not null then min(
                            o.source_available,
                            coalesce(case when m.volume_m3 is not null and ?1 > 0 and m.volume_m3 > 0 then floor(?1 / m.volume_m3) end, o.source_available),
                            coalesce(case when o.sell_region_volume is not null and ?2 > 0 then floor(o.sell_region_volume * ?2) end, o.source_available)
                        )
                    end
                ),
                my_orders.price_min,
                my_orders.price_max,
                my_orders.quantity,
                my_orders.order_count,
                o.buy_region_volume,
                o.sell_region_volume,
                o.last_refresh,
                coalesce(o.notes, p.notes),
                coalesce(o.script_notes, 'Awaiting ESI validation')
         from products p
         left join opportunities o on o.type_id = p.type_id
         left join item_metadata m on m.type_id = p.type_id
         left join trade_hubs sell_hub on sell_hub.name = o.sell_hub
         left join (
             select type_id,
                    location_id,
                    min(price) as price_min,
                    max(price) as price_max,
                    sum(volume_remain) as quantity,
                    count(*) as order_count
             from character_orders
             where is_buy_order = 0 and state = 'open'
             group by type_id, location_id
         ) my_orders on my_orders.type_id = p.type_id and my_orders.location_id = sell_hub.station_id
         where p.enabled = 1",
        )
        .map_err(to_string)?;
    let mut result = rows(
        stmt.query_map(
            params![cargo_m3, suggested_destination_volume_percent],
            opportunity_from_row,
        )
        .map_err(to_string)?,
    )?;
    let search = filters.search.trim().to_lowercase();
    result.retain(|row| {
        (filters.status == "ALL" || row.status == filters.status)
            && (filters.direction == "ALL" || row.direction == filters.direction)
            && (search.is_empty()
                || format!(
                    "{} {} {} {}",
                    row.type_id, row.item_name, row.notes, row.script_notes
                )
                .to_lowercase()
                .contains(&search))
    });
    Ok(result)
}

#[tauri::command]
fn update_product_notes(
    state: State<AppState>,
    type_id: i64,
    notes: String,
) -> Result<Product, String> {
    let conn = open(&state)?;
    conn.execute(
        "update products set notes = ?1 where type_id = ?2",
        params![notes, type_id],
    )
    .map_err(to_string)?;
    conn.execute(
        "update opportunities set notes = ?1 where type_id = ?2",
        params![notes, type_id],
    )
    .map_err(to_string)?;
    get_product(&conn, type_id)
}

#[tauri::command]
fn update_setting(state: State<AppState>, key: String, value: String) -> Result<Setting, String> {
    let conn = open(&state)?;
    conn.execute(
        "insert into settings(key, value, notes) values (?1, ?2, '')
         on conflict(key) do update set value = excluded.value",
        params![key, value],
    )
    .map_err(to_string)?;
    conn.query_row(
        "select key, value, notes from settings where key = ?1",
        params![key],
        |row| {
            Ok(Setting {
                key: row.get(0)?,
                value: row.get(1)?,
                notes: row.get(2)?,
            })
        },
    )
    .map_err(to_string)
}

#[tauri::command]
fn set_product_enabled(
    state: State<AppState>,
    type_id: i64,
    enabled: bool,
) -> Result<Product, String> {
    let conn = open(&state)?;
    conn.execute(
        "update products set enabled = ?1 where type_id = ?2",
        params![if enabled { 1 } else { 0 }, type_id],
    )
    .map_err(to_string)?;
    get_product(&conn, type_id)
}

#[tauri::command]
fn set_trade_hub_enabled(
    state: State<AppState>,
    id: i64,
    enabled: bool,
) -> Result<TradeHub, String> {
    let conn = open(&state)?;
    conn.execute(
        "update trade_hubs set enabled = ?1 where id = ?2",
        params![if enabled { 1 } else { 0 }, id],
    )
    .map_err(to_string)?;
    conn.query_row(
        "select id, name, region_id, station_id, enabled, priority from trade_hubs where id = ?1",
        params![id],
        trade_hub_from_row,
    )
    .map_err(to_string)
}

#[tauri::command]
fn discover_hot_products(state: State<AppState>) -> Result<DiscoveryRun, String> {
    let conn = open(&state)?;
    discover_hot_products_inner(&conn)
}

#[tauri::command]
fn start_refresh_next_batch(state: State<AppState>) -> Result<RefreshJob, String> {
    start_refresh_job(state, "batch".to_string(), None, false)
}

#[tauri::command]
fn start_reset_and_refresh(state: State<AppState>) -> Result<RefreshJob, String> {
    start_refresh_job(state, "reset".to_string(), None, true)
}

#[tauri::command]
fn start_refresh_product(state: State<AppState>, type_id: i64) -> Result<RefreshJob, String> {
    start_refresh_job(state, "product".to_string(), Some(type_id), false)
}

#[tauri::command]
fn reset_and_refresh(state: State<AppState>) -> Result<RefreshRun, String> {
    let conn = open(&state)?;
    conn.execute("insert into app_state(key, value) values ('cursor', '0') on conflict(key) do update set value = '0'", []).map_err(to_string)?;
    refresh_next_batch_inner(&conn)
}

#[tauri::command]
fn refresh_next_batch(state: State<AppState>) -> Result<RefreshRun, String> {
    let conn = open(&state)?;
    refresh_next_batch_inner(&conn)
}

#[tauri::command]
fn refresh_product(state: State<AppState>, type_id: i64) -> Result<Opportunity, String> {
    let conn = open(&state)?;
    refresh_product_inner(&conn, type_id)
}

fn start_refresh_job(
    state: State<AppState>,
    kind: String,
    type_id: Option<i64>,
    reset: bool,
) -> Result<RefreshJob, String> {
    let db_path = db_path(&state)?;
    let conn = open_path(db_path.clone())?;
    recover_stale_refresh_job(&conn)?;
    let current = get_refresh_job(&conn)?;
    if current.status == "running" {
        if kind == "product" {
            let queued_type_id = type_id.ok_or_else(|| "Missing product type ID".to_string())?;
            enqueue_refresh_product(&conn, queued_type_id)?;
            return get_refresh_job(&conn);
        }
        return Ok(current);
    }
    set_refresh_job(
        &conn,
        &RefreshJob {
            status: "running".to_string(),
            kind: kind.clone(),
            current_item: String::new(),
            scanned_count: 0,
            total_count: 0,
            api_calls: 0,
            last_error: String::new(),
            queued_count: queued_refresh_count(&conn)?,
            started_at: Utc::now().to_rfc3339(),
            finished_at: String::new(),
        },
    )?;
    thread::spawn(move || {
        let _ = run_refresh_job(db_path, kind, type_id, reset);
    });
    get_refresh_job(&conn)
}

fn run_refresh_job(
    db_path: PathBuf,
    kind: String,
    type_id: Option<i64>,
    reset: bool,
) -> Result<(), String> {
    let conn = open_path(db_path.clone())?;
    let result: Result<RefreshRun, String> = if kind == "product" {
        refresh_product_inner(
            &conn,
            type_id.ok_or_else(|| "Missing product type ID".to_string())?,
        )
        .and_then(|_| latest_refresh_run(&conn))
    } else {
        refresh_next_batch_inner_with_job(&conn, reset)
    };
    match result {
        Ok(run) => {
            let mut job = get_refresh_job(&conn)?;
            job.status = "done".to_string();
            job.current_item = String::new();
            job.api_calls = run.api_calls;
            if !run.errors.is_empty() {
                job.last_error = run.errors;
            }
            job.queued_count = queued_refresh_count(&conn)?;
            job.finished_at = Utc::now().to_rfc3339();
            set_refresh_job(&conn, &job)?;
            run_queued_refresh_jobs(&conn)
        }
        Err(error) => {
            let mut job = get_refresh_job(&conn)?;
            job.status = "failed".to_string();
            job.last_error = error;
            job.queued_count = queued_refresh_count(&conn)?;
            job.finished_at = Utc::now().to_rfc3339();
            set_refresh_job(&conn, &job)
        }
    }
}

fn refresh_product_inner(conn: &Connection, type_id: i64) -> Result<Opportunity, String> {
    let product = get_product(conn, type_id)?;
    let start = Utc::now();
    let mut api_calls = 0;
    update_refresh_job_progress(conn, &product.name, 0, 1, api_calls, "")?;
    let opportunity = fetch_and_analyze(conn, &product, &mut api_calls)?;
    upsert_opportunity(conn, &opportunity)?;
    update_refresh_job_progress(conn, &product.name, 1, 1, api_calls, "")?;
    insert_run(
        conn,
        RefreshRun {
            refresh_time: start.to_rfc3339(),
            items_scanned: 1,
            opportunities_written: 1,
            api_calls,
            errors: String::new(),
            skipped: "Manual product refresh".to_string(),
            duration_seconds: (Utc::now() - start).num_seconds(),
        },
    )?;
    Ok(opportunity)
}

fn refresh_next_batch_inner(conn: &Connection) -> Result<RefreshRun, String> {
    refresh_next_batch_inner_with_job(conn, false)
}

fn refresh_next_batch_inner_with_job(conn: &Connection, reset: bool) -> Result<RefreshRun, String> {
    let start = Utc::now();
    if reset {
        conn.execute("insert into app_state(key, value) values ('cursor', '0') on conflict(key) do update set value = '0'", []).map_err(to_string)?;
    }
    let max_items = setting(conn, "Max items per refresh", "5")
        .parse::<usize>()
        .unwrap_or(5)
        .max(1);
    let delay_ms = setting(conn, "Delay between items ms", "300")
        .parse::<u64>()
        .unwrap_or(300);
    let min_target_volume = setting(conn, "Skip refresh if target 30d volume below", "0")
        .parse::<f64>()
        .unwrap_or(0.0);
    let auto_disable_cold = setting(conn, "Auto-disable cold items", "TRUE") != "FALSE";
    let products = enabled_products(conn)?;
    let selected: Vec<Product> = products.iter().take(max_items).cloned().collect();
    update_refresh_job_progress(conn, "", 0, selected.len() as i64, 0, "")?;
    let mut errors = Vec::new();
    let mut written = 0;
    let mut skipped_low_volume = 0;
    let mut api_calls = 0;

    for (index, product) in selected.iter().enumerate() {
        update_refresh_job_progress(
            conn,
            &product.name,
            index as i64,
            selected.len() as i64,
            api_calls,
            "",
        )?;
        if should_skip_low_target_volume(conn, product.type_id, min_target_volume)? {
            skipped_low_volume += 1;
            update_refresh_job_progress(
                conn,
                &product.name,
                (index + 1) as i64,
                selected.len() as i64,
                api_calls,
                "",
            )?;
            continue;
        }
        match fetch_and_analyze(conn, product, &mut api_calls) {
            Ok(opportunity) => {
                if auto_disable_cold && is_cold_opportunity(&opportunity, min_target_volume) {
                    mark_product_cold(conn, product.type_id)?;
                    skipped_low_volume += 1;
                    update_refresh_job_progress(
                        conn,
                        &product.name,
                        (index + 1) as i64,
                        selected.len() as i64,
                        api_calls,
                        "",
                    )?;
                    continue;
                }
                upsert_opportunity(conn, &opportunity)?;
                written += 1;
            }
            Err(error) => {
                let message = format!("{}: {}", product.type_id, error);
                update_refresh_job_progress(
                    conn,
                    &product.name,
                    (index + 1) as i64,
                    selected.len() as i64,
                    api_calls,
                    &message,
                )?;
                errors.push(message);
            }
        }
        update_refresh_job_progress(
            conn,
            &product.name,
            (index + 1) as i64,
            selected.len() as i64,
            api_calls,
            "",
        )?;
        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
    }

    set_app_state(conn, "cursor", "0".to_string())?;
    let skipped = format!(
        "Priority refresh selected {} of {}; high estimated profit and stale rows run first{}",
        selected.len(),
        products.len(),
        if skipped_low_volume > 0 {
            format!("; skipped low target volume: {}", skipped_low_volume)
        } else {
            String::new()
        }
    );
    let run = RefreshRun {
        refresh_time: start.to_rfc3339(),
        items_scanned: selected.len() as i64,
        opportunities_written: written,
        api_calls,
        errors: errors.join("\n"),
        skipped,
        duration_seconds: (Utc::now() - start).num_seconds(),
    };
    insert_run(conn, run.clone())?;
    Ok(run)
}

fn discover_hot_products_inner(conn: &Connection) -> Result<DiscoveryRun, String> {
    let start = Utc::now();
    let mut errors = Vec::new();
    let item_types_imported = match import_item_types(conn) {
        Ok(count) => count,
        Err(error) => {
            errors.push(format!("item types: {}", error));
            0
        }
    };
    let market_rows_imported = match import_market_aggregates(conn) {
        Ok(count) => count,
        Err(error) => {
            errors.push(format!("market snapshot: {}", error));
            0
        }
    };
    let (candidates_found, products_enabled) = match generate_candidates(conn) {
        Ok(counts) => counts,
        Err(error) => {
            errors.push(format!("candidates: {}", error));
            (0, 0)
        }
    };
    let run = DiscoveryRun {
        run_time: start.to_rfc3339(),
        item_types_imported,
        market_rows_imported,
        candidates_found,
        products_enabled,
        errors: errors.join("\n"),
        duration_seconds: (Utc::now() - start).num_seconds(),
    };
    conn.execute(
        "insert into discovery_runs(run_time, item_types_imported, market_rows_imported, candidates_found, products_enabled, errors, duration_seconds)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![run.run_time, run.item_types_imported, run.market_rows_imported, run.candidates_found, run.products_enabled, run.errors, run.duration_seconds],
    ).map_err(to_string)?;
    Ok(run)
}

fn run_initial_discovery_if_needed(conn: &Connection) {
    let known_items = count_table(conn, "item_types").unwrap_or(0);
    let candidates = count_table(conn, "candidate_products").unwrap_or(0);
    if known_items == 0 || candidates == 0 {
        let _ = discover_hot_products_inner(conn);
    }
}

fn import_item_types(conn: &Connection) -> Result<i64, String> {
    let url = setting(
        conn,
        "Item type CSV URL",
        "https://www.fuzzwork.co.uk/resources/typeids.csv",
    );
    let body = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "EVE Metrade local app")
        .send()
        .map_err(to_string)?
        .error_for_status()
        .map_err(to_string)?
        .bytes()
        .map_err(to_string)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(body.as_ref());
    let tx = conn.unchecked_transaction().map_err(to_string)?;
    let now = Utc::now().to_rfc3339();
    let mut imported = 0;
    for record in reader.records() {
        let record = record.map_err(to_string)?;
        let type_id = record
            .get(0)
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let name = record.get(1).unwrap_or("").trim();
        if type_id <= 0 || name.is_empty() || name.starts_with('#') {
            continue;
        }
        tx.execute(
            "insert into item_types(type_id, name, source, source_updated_at) values (?1, ?2, 'fuzzwork-typeids', ?3)
             on conflict(type_id) do update set name = excluded.name, source = excluded.source, source_updated_at = excluded.source_updated_at",
            params![type_id, name, now],
        ).map_err(to_string)?;
        imported += 1;
    }
    tx.commit().map_err(to_string)?;
    Ok(imported)
}

fn import_market_aggregates(conn: &Connection) -> Result<i64, String> {
    let url = setting(
        conn,
        "Market aggregate CSV URL",
        "https://market.fuzzwork.co.uk/aggregatecsv.csv.gz",
    );
    let forge_region = setting(conn, "The Forge region ID", "10000002")
        .parse::<i64>()
        .unwrap_or(10000002);
    let domain_region = setting(conn, "Domain region ID", "10000043")
        .parse::<i64>()
        .unwrap_or(10000043);
    let bytes = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "EVE Metrade local app")
        .send()
        .map_err(to_string)?
        .error_for_status()
        .map_err(to_string)?
        .bytes()
        .map_err(to_string)?;
    let mut decoded = String::new();
    GzDecoder::new(bytes.as_ref())
        .read_to_string(&mut decoded)
        .map_err(to_string)?;
    let mut reader = csv::Reader::from_reader(decoded.as_bytes());
    let tx = conn.unchecked_transaction().map_err(to_string)?;
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "delete from market_aggregates where region_id in (?1, ?2)",
        params![forge_region, domain_region],
    )
    .map_err(to_string)?;
    let mut imported = 0;
    for record in reader.records() {
        let record = record.map_err(to_string)?;
        let what = record.get(0).unwrap_or("");
        let mut parts = what.split('|');
        let region_id = parts
            .next()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let type_id = parts
            .next()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let is_buy = parts.next().map(|value| value == "true").unwrap_or(false);
        if (region_id != forge_region && region_id != domain_region) || is_buy || type_id <= 0 {
            continue;
        }
        tx.execute(
            "insert into market_aggregates(region_id, type_id, is_buy, weighted_average, max_price, min_price, stddev, median, volume, num_orders, five_percent, order_set, snapshot_at)
             values (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             on conflict(region_id, type_id, is_buy) do update set weighted_average=excluded.weighted_average, max_price=excluded.max_price, min_price=excluded.min_price,
             stddev=excluded.stddev, median=excluded.median, volume=excluded.volume, num_orders=excluded.num_orders, five_percent=excluded.five_percent, order_set=excluded.order_set, snapshot_at=excluded.snapshot_at",
            params![
                region_id,
                type_id,
                csv_f64(&record, 1),
                csv_f64(&record, 2),
                csv_f64(&record, 3),
                csv_f64(&record, 4),
                csv_f64(&record, 5),
                csv_f64(&record, 6),
                csv_i64(&record, 7),
                csv_f64(&record, 8),
                csv_i64(&record, 9),
                now
            ],
        ).map_err(to_string)?;
        imported += 1;
    }
    tx.commit().map_err(to_string)?;
    Ok(imported)
}

fn generate_candidates(conn: &Connection) -> Result<(i64, i64), String> {
    let forge_region = setting(conn, "The Forge region ID", "10000002")
        .parse::<i64>()
        .unwrap_or(10000002);
    let domain_region = setting(conn, "Domain region ID", "10000043")
        .parse::<i64>()
        .unwrap_or(10000043);
    let min_volume = setting(conn, "Candidate minimum sell volume per hub", "100")
        .parse::<f64>()
        .unwrap_or(100.0);
    let min_orders = setting(conn, "Candidate minimum sell orders per hub", "1")
        .parse::<i64>()
        .unwrap_or(1);
    let min_spread = setting(conn, "Candidate minimum rough spread", "0.03")
        .parse::<f64>()
        .unwrap_or(0.03);
    let max_candidates = setting(conn, "Candidate max enabled products", "500")
        .parse::<i64>()
        .unwrap_or(500)
        .max(1);
    let now = Utc::now().to_rfc3339();
    conn.execute("delete from candidate_products", [])
        .map_err(to_string)?;
    conn.execute(
        "insert into candidate_products(type_id, name, source, source_updated_at, forge_sell_volume, domain_sell_volume, forge_sell_price, domain_sell_price, rough_spread, score, enabled, reason)
         select f.type_id,
                coalesce(t.name, printf('Type %d', f.type_id)),
                'fuzzwork-aggregate',
                ?1,
                f.volume,
                d.volume,
                case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end,
                case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end,
                abs((case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end) -
                    (case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end)) /
                    min((case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end),
                        (case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end)),
                (min(f.volume, d.volume) *
                  abs((case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end) -
                      (case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end))) /
                  max(1.0, min((case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end),
                               (case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end))),
                1,
                'Aggregate snapshot passed volume/order/spread filters'
         from market_aggregates f
         join market_aggregates d on d.type_id = f.type_id and d.region_id = ?3 and d.is_buy = 0
         left join item_types t on t.type_id = f.type_id
         where f.region_id = ?2
           and f.is_buy = 0
           and f.volume >= ?4
           and d.volume >= ?4
           and f.num_orders >= ?5
           and d.num_orders >= ?5
           and (case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end) > 0
           and (case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end) > 0
           and abs((case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end) -
                   (case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end)) /
               min((case when f.five_percent > 0 then f.five_percent when f.min_price > 0 then f.min_price else f.median end),
                   (case when d.five_percent > 0 then d.five_percent when d.min_price > 0 then d.min_price else d.median end)) >= ?6
         order by 10 desc
         limit ?7",
        params![now, forge_region, domain_region, min_volume, min_orders, min_spread, max_candidates],
    ).map_err(to_string)?;
    let candidates: i64 = conn
        .query_row("select count(*) from candidate_products", [], |row| {
            row.get(0)
        })
        .map_err(to_string)?;
    conn.execute(
        "insert into products(type_id, name, enabled, notes)
         select type_id, name, 1, ''
         from candidate_products
         where enabled = 1
         on conflict(type_id) do update set name = case when products.name = '' then excluded.name else products.name end, enabled = 1",
        [],
    ).map_err(to_string)?;
    let enabled: i64 = conn
        .query_row(
            "select count(*) from products where enabled = 1",
            [],
            |row| row.get(0),
        )
        .map_err(to_string)?;
    set_app_state(conn, "cursor", "0".to_string())?;
    Ok((candidates, enabled))
}

fn fetch_and_analyze(
    conn: &Connection,
    product: &Product,
    api_calls: &mut i64,
) -> Result<Opportunity, String> {
    let base = setting(conn, "ESI base URL", "https://esi.evetech.net/latest");
    let error_limit_threshold = setting(conn, "ESI low error-limit threshold", "20")
        .parse::<u64>()
        .unwrap_or(20);
    let metadata = product_with_metadata(conn, product, &base, api_calls, error_limit_threshold);
    let product = metadata.product;
    let hubs = enabled_or_all_trade_hubs(conn, true)?;
    if hubs.len() < 2 {
        return Ok(empty_opportunity(
            "ERROR",
            &product,
            0.0,
            0.0,
            Utc::now().to_rfc3339(),
            "Enable at least two trade hubs.",
        ));
    }
    let mut markets = Vec::new();
    for hub in hubs {
        let orders: Vec<EsiOrder> = fetch_json(
            conn,
            &format!(
                "{}/markets/{}/orders/?datasource=tranquility&order_type=sell&type_id={}&page=1",
                base, hub.region_id, product.type_id
            ),
            api_calls,
            error_limit_threshold,
        )?;
        let history: Vec<HistoryRow> = fetch_json(
            conn,
            &format!(
                "{}/markets/{}/history/?datasource=tranquility&type_id={}",
                base, hub.region_id, product.type_id
            ),
            api_calls,
            error_limit_threshold,
        )?;
        markets.push((hub, orders, recent_volume(&history)));
    }
    Ok(analyze(conn, &product, metadata.volume_m3, markets))
}

fn product_with_metadata(
    conn: &Connection,
    product: &Product,
    base: &str,
    api_calls: &mut i64,
    error_limit_threshold: u64,
) -> ProductMetadata {
    let cached_volume = conn
        .query_row(
            "select volume_m3 from item_metadata where type_id = ?1",
            params![product.type_id],
            |row| row.get::<_, Option<f64>>(0),
        )
        .optional()
        .ok()
        .flatten()
        .flatten();
    if !product.name.trim().is_empty() && cached_volume.is_some() {
        return ProductMetadata {
            product: product.clone(),
            volume_m3: cached_volume,
        };
    }
    let mut named = product.clone();
    if let Ok(type_info) = fetch_json::<EsiType>(
        conn,
        &format!(
            "{}/universe/types/{}/?datasource=tranquility&language=en",
            base, product.type_id
        ),
        api_calls,
        error_limit_threshold,
    ) {
        if !type_info.name.trim().is_empty() {
            named.name = type_info.name.trim().to_string();
            let _ = conn.execute(
                "update products set name = ?1 where type_id = ?2",
                params![named.name, named.type_id],
            );
            let _ = conn.execute(
                "update opportunities set item_name = ?1 where type_id = ?2 and item_name = ''",
                params![named.name, named.type_id],
            );
        }
        if let Some(volume) = type_info.volume {
            let _ = conn.execute(
                "insert into item_metadata(type_id, volume_m3, source_updated_at) values (?1, ?2, ?3)
                 on conflict(type_id) do update set volume_m3 = excluded.volume_m3, source_updated_at = excluded.source_updated_at",
                params![named.type_id, volume, Utc::now().to_rfc3339()],
            );
            return ProductMetadata {
                product: named,
                volume_m3: Some(volume),
            };
        }
    }
    ProductMetadata {
        product: named,
        volume_m3: cached_volume,
    }
}

fn analyze(
    conn: &Connection,
    product: &Product,
    volume_m3: Option<f64>,
    markets: Vec<(TradeHub, Vec<EsiOrder>, f64)>,
) -> Opportunity {
    let min_spread = setting(conn, "Minimum spread", "0.2")
        .parse::<f64>()
        .unwrap_or(0.2);
    let min_profit = setting(conn, "Minimum estimated profit", "500000")
        .parse::<f64>()
        .unwrap_or(500000.0);
    let suggested_buy_destination_volume_percent = setting(
        conn,
        "Suggested buy max destination 30d volume percent",
        "0.3",
    )
    .parse::<f64>()
    .unwrap_or(0.3)
    .clamp(0.0, 1.0);
    let min_source_volume = setting(conn, "Minimum 30d source volume", "1")
        .parse::<f64>()
        .unwrap_or(1.0);
    let min_dest_volume = setting(conn, "Minimum 30d destination volume", "1")
        .parse::<f64>()
        .unwrap_or(1.0);
    let sell_ref_min_units = setting(conn, "Sell reference minimum units", "5")
        .parse::<f64>()
        .unwrap_or(5.0)
        .max(1.0);
    let sell_ref_min_isk = setting(conn, "Sell reference minimum ISK depth", "25000000")
        .parse::<f64>()
        .unwrap_or(25000000.0)
        .max(0.0);
    let cargo_m3 = setting(conn, "Ship cargo capacity m3", "7900")
        .parse::<f64>()
        .unwrap_or(7900.0)
        .max(0.0);
    let refreshed = Utc::now().to_rfc3339();
    let hub_markets: Vec<HubMarketData> = markets
        .iter()
        .map(|(hub, orders, volume)| {
            let mut sells: Vec<&EsiOrder> = orders
                .iter()
                .filter(|order| !order.is_buy_order && order.location_id == hub.station_id)
                .collect();
            sells.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
            HubMarketData {
                hub: hub.clone(),
                prices: hub_prices(&sells, sell_ref_min_units, sell_ref_min_isk),
                volume: *volume,
            }
        })
        .collect();
    let available_hubs: Vec<&HubMarketData> = hub_markets
        .iter()
        .filter(|hub| hub.prices.lowest_sell > 0.0)
        .collect();
    if available_hubs.is_empty() {
        return empty_opportunity(
            "NO SELL ORDERS",
            product,
            0.0,
            0.0,
            refreshed,
            "No sell orders at enabled hub stations",
        );
    }
    if available_hubs.len() < 2 {
        let volume = available_hubs.first().map(|hub| hub.volume).unwrap_or(0.0);
        return empty_opportunity(
            "NO SPREAD",
            product,
            volume,
            0.0,
            refreshed,
            "Only one enabled hub has sell orders.",
        );
    }
    let mut best: Option<RouteCandidate<'_>> = None;
    for buy in &available_hubs {
        for sell in &available_hubs {
            if buy.hub.id == sell.hub.id {
                continue;
            }
            let profit = sell.prices.reference_sell - buy.prices.lowest_sell;
            if best
                .as_ref()
                .map(|route| profit > route.profit)
                .unwrap_or(true)
            {
                best = Some(RouteCandidate { buy, sell, profit });
            }
        }
    }
    let route = best.expect("at least two available hubs produce route candidates");
    let buy_hub = route.buy.hub.name.as_str();
    let sell_hub = route.sell.hub.name.as_str();
    let buy_price = route.buy.prices.lowest_sell;
    let destination_lowest_sell = route.sell.prices.lowest_sell;
    let sell_reference = route.sell.prices.reference_sell;
    let source_available = route.buy.prices.available_at_lowest;
    let buy_volume = route.buy.volume;
    let sell_volume = route.sell.volume;
    let profit = sell_reference - buy_price;
    let spread = if buy_price > 0.0 {
        profit / buy_price
    } else {
        0.0
    };
    let cargo_units = cargo_unit_capacity(cargo_m3, volume_m3);
    let destination_volume_units = if suggested_buy_destination_volume_percent > 0.0 {
        Some((sell_volume * suggested_buy_destination_volume_percent).floor())
    } else {
        Some(0.0)
    };
    let suggested_buy_quantity =
        suggested_buy_quantity(source_available, cargo_units, destination_volume_units);
    let estimated_profit: f64 = (suggested_buy_quantity * profit).max(0.0);
    let cargo_used_percent = cargo_used_percent(cargo_m3, volume_m3, suggested_buy_quantity);
    let (status, script_notes) = if profit <= 0.0 {
        (
            "NO SPREAD",
            "Depth-adjusted sell reference is equal or inverted.",
        )
    } else if spread < min_spread {
        ("LOW SPREAD", "Below minimum spread.")
    } else if estimated_profit < min_profit {
        ("LOW PROFIT", "Below minimum estimated profit.")
    } else if buy_volume < min_source_volume || sell_volume < min_dest_volume {
        ("LOW TRAFFIC", "Below recent regional volume threshold.")
    } else {
        ("GOOD", "Sell reference uses market depth to reduce one-off order skew; profit is capped by source units and cargo space.")
    };
    Opportunity {
        status: status.to_string(),
        direction: format!("{} -> {}", buy_hub, sell_hub),
        type_id: product.type_id,
        item_name: product.name.clone(),
        buy_hub: buy_hub.to_string(),
        sell_hub: sell_hub.to_string(),
        buy_price: Some(buy_price),
        sell_reference: Some(sell_reference),
        destination_lowest_sell: Some(destination_lowest_sell),
        profit_per_unit: Some(profit),
        spread: Some(spread),
        source_available: Some(source_available),
        estimated_profit: Some(estimated_profit),
        cargo_used_percent,
        suggested_buy_quantity: Some(suggested_buy_quantity),
        my_destination_sell_price_min: None,
        my_destination_sell_price_max: None,
        my_destination_sell_quantity: None,
        my_destination_sell_order_count: None,
        buy_region_volume: Some(buy_volume),
        sell_region_volume: Some(sell_volume),
        last_refresh: Some(refreshed),
        last_refresh_minutes: Some(0),
        notes: product.notes.clone(),
        script_notes: script_notes.to_string(),
    }
}

fn hub_prices(orders: &[&EsiOrder], min_units: f64, min_isk_depth: f64) -> HubPrices {
    let lowest_sell = orders.first().map(|order| order.price).unwrap_or(0.0);
    if lowest_sell <= 0.0 {
        return HubPrices {
            lowest_sell: 0.0,
            reference_sell: 0.0,
            available_at_lowest: 0.0,
        };
    }
    let available_at_lowest = orders
        .iter()
        .filter(|order| order.price <= lowest_sell)
        .map(|order| order.volume_remain)
        .sum();
    let mut cumulative_units = 0.0;
    let mut cumulative_value = 0.0;
    for order in orders {
        cumulative_units += order.volume_remain.max(0.0);
        cumulative_value += order.volume_remain.max(0.0) * order.price.max(0.0);
        if cumulative_units >= min_units
            || (min_isk_depth > 0.0 && cumulative_value >= min_isk_depth)
        {
            return HubPrices {
                lowest_sell,
                reference_sell: order.price,
                available_at_lowest,
            };
        }
    }
    HubPrices {
        lowest_sell,
        reference_sell: orders
            .last()
            .map(|order| order.price)
            .unwrap_or(lowest_sell),
        available_at_lowest,
    }
}

fn cargo_unit_capacity(cargo_m3: f64, volume_m3: Option<f64>) -> Option<f64> {
    let volume = volume_m3?;
    if cargo_m3 <= 0.0 || volume <= 0.0 {
        return None;
    }
    Some((cargo_m3 / volume).floor().max(0.0))
}

fn suggested_buy_quantity(
    source_available: f64,
    cargo_units: Option<f64>,
    destination_volume_units: Option<f64>,
) -> f64 {
    let mut quantity = source_available.max(0.0);
    if let Some(units) = cargo_units {
        quantity = quantity.min(units.max(0.0));
    }
    if let Some(units) = destination_volume_units {
        quantity = quantity.min(units.max(0.0));
    }
    quantity.floor()
}

fn cargo_used_percent(cargo_m3: f64, volume_m3: Option<f64>, units_bought: f64) -> Option<f64> {
    let volume = volume_m3?;
    if cargo_m3 <= 0.0 || volume <= 0.0 {
        return None;
    }
    Some(((units_bought * volume) / cargo_m3).clamp(0.0, 1.0))
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        create table if not exists products(type_id integer primary key, name text not null, enabled integer not null, notes text not null);
        create table if not exists settings(key text primary key, value text not null, notes text not null);
        create table if not exists trade_hubs(
          id integer primary key,
          name text not null unique,
          region_id integer not null,
          station_id integer not null,
          enabled integer not null,
          priority integer not null
        );
        create table if not exists item_types(type_id integer primary key, name text not null, source text not null, source_updated_at text not null);
        create table if not exists item_metadata(type_id integer primary key, volume_m3 real, source_updated_at text not null);
        create table if not exists market_aggregates(
          region_id integer not null, type_id integer not null, is_buy integer not null,
          weighted_average real not null, max_price real not null, min_price real not null, stddev real not null,
          median real not null, volume real not null, num_orders integer not null, five_percent real not null,
          order_set integer not null, snapshot_at text not null,
          primary key(region_id, type_id, is_buy)
        );
        create table if not exists candidate_products(
          type_id integer primary key, name text not null, source text not null, source_updated_at text not null,
          forge_sell_volume real not null, domain_sell_volume real not null,
          forge_sell_price real not null, domain_sell_price real not null,
          rough_spread real not null, score real not null, enabled integer not null, reason text not null
        );
        create table if not exists opportunities(
          type_id integer primary key, status text not null, direction text not null, item_name text not null,
          buy_hub text not null, sell_hub text not null, buy_price real, sell_reference real, destination_lowest_sell real, profit_per_unit real,
          spread real, source_available real, estimated_profit real, cargo_used_percent real, suggested_buy_quantity real, buy_region_volume real, sell_region_volume real,
          last_refresh text, notes text not null, script_notes text not null
        );
        create table if not exists refresh_runs(refresh_time text not null, items_scanned integer not null, opportunities_written integer not null, api_calls integer not null, errors text not null, skipped text not null, duration_seconds integer not null);
        create table if not exists refresh_jobs(
          id integer primary key check(id = 1),
          status text not null, kind text not null, current_item text not null,
          scanned_count integer not null, total_count integer not null, api_calls integer not null,
          last_error text not null, queued_count integer not null default 0, started_at text not null, finished_at text not null
        );
        create table if not exists refresh_queue(
          id integer primary key autoincrement,
          kind text not null,
          type_id integer,
          created_at text not null
        );
        create table if not exists discovery_runs(run_time text not null, item_types_imported integer not null, market_rows_imported integer not null, candidates_found integer not null, products_enabled integer not null, errors text not null, duration_seconds integer not null);
        create table if not exists app_state(key text primary key, value text not null);
        create table if not exists api_limit_state(key text primary key, value text not null);
        create table if not exists auth_characters(
          character_id integer primary key,
          character_name text not null,
          scopes text not null,
          access_token text not null,
          refresh_token text not null,
          expires_at text not null,
          updated_at text not null
        );
        create table if not exists auth_events(
          happened_at text not null,
          status text not null,
          message text not null
        );
        create table if not exists character_orders(
          character_id integer not null,
          order_id integer not null,
          type_id integer not null,
          region_id integer not null,
          location_id integer not null,
          is_buy_order integer not null,
          price real not null,
          volume_remain integer not null,
          volume_total integer not null,
          issued text not null,
          duration integer not null,
          range text not null,
          state text not null,
          refreshed_at text not null,
          primary key(character_id, order_id)
        );
        "
    )?;
    let _ = conn.execute(
        "alter table refresh_jobs add column queued_count integer not null default 0",
        [],
    );
    let _ = conn.execute(
        "alter table opportunities add column cargo_used_percent real",
        [],
    );
    let _ = conn.execute(
        "alter table opportunities add column destination_lowest_sell real",
        [],
    );
    let _ = conn.execute(
        "alter table opportunities add column suggested_buy_quantity real",
        [],
    );
    conn.execute(
        "update settings set value = '7900' where key = 'Ship cargo capacity m3' and value = '60000'",
        [],
    )?;
    seed_trade_hubs(conn)?;
    conn.execute(
        "insert into settings(key, value, notes) values ('Automatic refresh interval seconds', '600', 'Runs one batch every 10 minutes; keep this at 60 or higher for ESI safety.')
         on conflict(key) do nothing",
        [],
    )?;
    conn.execute(
        "insert into settings(key, value, notes) values ('ESI low error-limit threshold', '20', 'When ESI reports this many or fewer errors left, the app waits for reset.')
         on conflict(key) do nothing",
        [],
    )?;
    let discovery_settings = [
        (
            "Item type CSV URL",
            "https://www.fuzzwork.co.uk/resources/typeids.csv",
            "Static type ID and name list. Does not use ESI.",
        ),
        (
            "Market aggregate CSV URL",
            "https://market.fuzzwork.co.uk/aggregatecsv.csv.gz",
            "Bulk market aggregate snapshot. Does not use ESI.",
        ),
        (
            "Candidate minimum sell volume per hub",
            "100",
            "Fuzzwork aggregate sell volume required in both regions.",
        ),
        (
            "Candidate minimum sell orders per hub",
            "1",
            "Fuzzwork aggregate sell order count required in both regions.",
        ),
        (
            "Candidate minimum rough spread",
            "0.03",
            "Minimum rough price gap before spending ESI calls.",
        ),
        (
            "Candidate max enabled products",
            "500",
            "Maximum discovered products to add to the ESI refresh list.",
        ),
        (
            "Estimated safe ESI calls per hour",
            "1200",
            "UI budget for the API burn-rate indicator.",
        ),
        (
            "Auto-disable cold items",
            "TRUE",
            "After ESI validation, cold/low-traffic items are disabled to avoid future calls.",
        ),
        (
            "Sell reference minimum units",
            "5",
            "Use the first sell price level that reaches this cumulative unit depth.",
        ),
        (
            "Sell reference minimum ISK depth",
            "25000000",
            "Use this cumulative ISK depth as an alternate sell-reference threshold.",
        ),
        (
            "Ship cargo capacity m3",
            "7900",
            "Maximum cargo volume used to cap estimated profit.",
        ),
        (
            "Refresh stale timeout seconds",
            "600",
            "Marks a refresh failed if it does not complete within this time.",
        ),
        (
            "Max enabled hubs for ESI refresh",
            "3",
            "Caps enabled trade hubs used per item to limit API calls.",
        ),
        (
            "Suggested buy max destination 30d volume percent",
            "0.3",
            "Suggested buy quantity will not exceed this share of destination 30-day volume.",
        ),
        (
            "EVE SSO client ID",
            "",
            "Client ID from developers.eveonline.com for native EVE login.",
        ),
        (
            "EVE SSO callback URL",
            "http://localhost:17890/callback",
            "Must match the callback URL in the EVE developer app.",
        ),
        (
            "EVE SSO scopes",
            "esi-markets.read_character_orders.v1",
            "Scopes requested when logging in with EVE.",
        ),
    ];
    for row in discovery_settings {
        conn.execute("insert into settings(key, value, notes) values (?1, ?2, ?3) on conflict(key) do nothing", params![row.0, row.1, row.2])?;
    }
    conn.execute("delete from settings where key like 'Watchdog%'", [])?;
    conn.execute("update products set notes = '' where notes = 'Discovered from Fuzzwork aggregate snapshot'", [])?;
    conn.execute("update opportunities set notes = '' where notes = 'Discovered from Fuzzwork aggregate snapshot'", [])?;
    Ok(())
}

fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row("select count(*) from settings", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }
    let settings = [
        (
            "Jita station ID",
            "60003760",
            "Jita IV - Moon 4 - Caldari Navy Assembly Plant",
        ),
        (
            "Amarr station ID",
            "60008494",
            "Amarr VIII (Oris) - Emperor Family Academy",
        ),
        ("The Forge region ID", "10000002", "Jita region"),
        ("Domain region ID", "10000043", "Amarr region"),
        ("Minimum spread", "0.2", "20% default"),
        ("Minimum estimated profit", "500000", "ISK"),
        (
            "Minimum 30d source volume",
            "1",
            "Regional history traffic check",
        ),
        (
            "Minimum 30d destination volume",
            "1",
            "Regional history traffic check",
        ),
        ("History days", "30", "Use recent market history"),
        (
            "Max items per refresh",
            "5",
            "Lowered after URL-fetch quota and ESI 429 errors.",
        ),
        (
            "Delay between items ms",
            "300",
            "Slower requests reduce ESI pressure.",
        ),
        (
            "Include weak rows",
            "TRUE",
            "TRUE keeps rejected rows with status notes",
        ),
        ("Raw order rows per item/route", "10", "Limits audit rows"),
        (
            "ESI base URL",
            "https://esi.evetech.net/latest",
            "Public ESI, no login",
        ),
        (
            "User agent",
            "EVE Metrade local app",
            "Public ESI user agent",
        ),
        (
            "Automatic refresh enabled",
            "TRUE",
            "Controls background refresh",
        ),
        (
            "Automatic refresh interval seconds",
            "600",
            "Runs one batch every 10 minutes; keep this at 60 or higher for ESI safety.",
        ),
        (
            "ESI low error-limit threshold",
            "20",
            "When ESI reports this many or fewer errors left, the app waits for reset.",
        ),
        (
            "Item type CSV URL",
            "https://www.fuzzwork.co.uk/resources/typeids.csv",
            "Static type ID and name list. Does not use ESI.",
        ),
        (
            "Market aggregate CSV URL",
            "https://market.fuzzwork.co.uk/aggregatecsv.csv.gz",
            "Bulk market aggregate snapshot. Does not use ESI.",
        ),
        (
            "Candidate minimum sell volume per hub",
            "100",
            "Fuzzwork aggregate sell volume required in both regions.",
        ),
        (
            "Candidate minimum sell orders per hub",
            "1",
            "Fuzzwork aggregate sell order count required in both regions.",
        ),
        (
            "Candidate minimum rough spread",
            "0.03",
            "Minimum rough price gap before spending ESI calls.",
        ),
        (
            "Candidate max enabled products",
            "500",
            "Maximum discovered products to add to the ESI refresh list.",
        ),
        (
            "Estimated safe ESI calls per hour",
            "1200",
            "UI budget for the API burn-rate indicator.",
        ),
        (
            "Auto-disable cold items",
            "TRUE",
            "After ESI validation, cold/low-traffic items are disabled to avoid future calls.",
        ),
        (
            "Sell reference minimum units",
            "5",
            "Use the first sell price level that reaches this cumulative unit depth.",
        ),
        (
            "Sell reference minimum ISK depth",
            "25000000",
            "Use this cumulative ISK depth as an alternate sell-reference threshold.",
        ),
        (
            "Ship cargo capacity m3",
            "7900",
            "Maximum cargo volume used to cap estimated profit.",
        ),
        (
            "Refresh stale timeout seconds",
            "600",
            "Marks a refresh failed if it does not complete within this time.",
        ),
        (
            "Max enabled hubs for ESI refresh",
            "3",
            "Caps enabled trade hubs used per item to limit API calls.",
        ),
        (
            "Skip refresh if target 30d volume below",
            "50",
            "Skips already-known dead destination markets",
        ),
    ];
    for setting_row in settings {
        conn.execute(
            "insert into settings(key, value, notes) values (?1, ?2, ?3)",
            params![setting_row.0, setting_row.1, setting_row.2],
        )?;
    }
    for type_id in [
        2180, 2403, 2549, 3266, 3456, 3777, 3995, 4435, 10244, 11443, 31270, 31274, 31312, 31412,
        31532, 31600, 31718, 31754, 31764, 31874, 31876, 32994, 33180, 33334, 33441, 33569, 33704,
    ] {
        conn.execute(
            "insert into products(type_id, name, enabled, notes) values (?1, '', 1, '')",
            params![type_id],
        )?;
    }
    conn.execute(
        "insert into app_state(key, value) values ('cursor', '0') on conflict(key) do update set value = '0'",
        [],
    )?;
    Ok(())
}

fn seed_initial_opportunities(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row("select count(*) from opportunities", [], |row| row.get(0))?;
    let names = [
        (2180, "Guristas Scourge XL Cruise Missile"),
        (2403, "Advanced Planetology"),
        (3266, "Zainou 'Gypsy' CPU Management EE-604"),
        (3456, "Jump Drive Operation"),
        (3995, "Large EMP Smartbomb II"),
        (10244, "Zainou 'Gypsy' Signature Analysis SA-703"),
        (11443, "Hydromagnetic Physics"),
        (31270, "Medium Inverted Signal Field Projector II"),
        (31274, "Small Ionic Field Projector I"),
        (31312, "Medium Signal Focusing Kit I"),
        (31600, "Medium Hydraulic Bay Thrusters I"),
        (31718, "Medium EM Shield Reinforcer I"),
        (31754, "Medium Thermal Shield Reinforcer I"),
        (31764, "Small Core Defense Capacitor Safeguard I"),
        (32994, "Barium Firework CXIV"),
        (31876, "Caldari Navy Wasp"),
        (33441, "Limos' Rapid Heavy Missile Launcher I"),
        (2549, "Lava Command Center"),
        (31874, "Caldari Navy Vespa"),
        (33569, "Melted Snowball"),
        (4435, "Eutectic Compact Cap Recharger"),
        (31532, "Small Hybrid Burst Aerator II"),
        (3777, "Long-limb Roes"),
        (33180, "Scan Rangefinding Array I"),
        (31412, "Small Semiconductor Memory Cell II"),
        (33334, "Navy Cap Booster 75"),
        (33704, "Medium Hull Maintenance Bot I"),
    ];
    for (type_id, name) in names {
        conn.execute(
            "update products set name = ?1 where type_id = ?2 and name = ''",
            params![name, type_id],
        )?;
        conn.execute(
            "update opportunities set item_name = ?1 where type_id = ?2 and item_name = ''",
            params![name, type_id],
        )?;
    }
    if count > 0 {
        return Ok(());
    }
    let rows = [
        ("GOOD", "Amarr -> Jita", 32994, "Barium Firework CXIV", "Amarr", "Jita", 14900.0, 18970.0, 4070.0, 0.2731543624, 1191.0, 4847370.0, 4999.0, 112820.0, ""),
        ("LOW SPREAD", "Amarr -> Jita", 31876, "Caldari Navy Wasp", "Amarr", "Jita", 2371000.0, 2385000.0, 14000.0, 0.0059046816, 2.0, 28000.0, 2929.0, 39711.0, ""),
        ("LOW SPREAD", "Amarr -> Jita", 33441, "Limos' Rapid Heavy Missile Launcher I", "Amarr", "Jita", 25110.0, 25690.0, 580.0, 0.0230983672, 1.0, 580.0, 624.0, 24597.0, "Below minimum spread."),
        ("LOW SPREAD", "Amarr -> Jita", 2549, "Lava Command Center", "Amarr", "Jita", 133300.0, 148700.0, 15400.0, 0.1155288822, 2.0, 30800.0, 4928.0, 21515.0, "Below minimum spread."),
        ("LOW SPREAD", "Jita -> Amarr", 31874, "Caldari Navy Vespa", "Jita", "Amarr", 1850000.0, 1989000.0, 139000.0, 0.0751351351, 340.0, 47260000.0, 100989.0, 9612.0, ""),
        ("LOW PROFIT", "Jita -> Amarr", 33569, "Melted Snowball", "Jita", "Amarr", 33.01, 399.9, 366.89, 11.1145107543, 800.0, 293512.0, 87131.0, 8365.0, "Below minimum estimated profit."),
        ("LOW SPREAD", "Jita -> Amarr", 4435, "Eutectic Compact Cap Recharger", "Jita", "Amarr", 13670.0, 14810.0, 1140.0, 0.0833942941, 12272.0, 13990080.0, 89872.0, 8320.0, "Below minimum spread."),
        ("LOW SPREAD", "Amarr -> Jita", 31532, "Small Hybrid Burst Aerator II", "Amarr", "Jita", 2244000.0, 2328000.0, 84000.0, 0.0374331551, 2.0, 168000.0, 267.0, 8209.0, "Below minimum spread."),
        ("LOW PROFIT", "Jita -> Amarr", 3777, "Long-limb Roes", "Jita", "Amarr", 2074.0, 4500.0, 2426.0, 1.1697203472, 30.0, 72780.0, 14901866.0, 7488.0, "Below minimum estimated profit."),
        ("GOOD", "Jita -> Amarr", 33180, "Scan Rangefinding Array I", "Jita", "Amarr", 113900.0, 157900.0, 44000.0, 0.3863037752, 93.0, 4092000.0, 42069.0, 5526.0, ""),
        ("GOOD", "Amarr -> Jita", 31412, "Small Semiconductor Memory Cell II", "Amarr", "Jita", 4790000.0, 20860000.0, 16070000.0, 3.3549060543, 1.0, 16070000.0, 480.0, 2077.0, "Both prices are sell orders; direction is chosen from lower sell price to higher sell price."),
    ];
    for row in rows {
        conn.execute(
            "insert into opportunities(type_id, status, direction, item_name, buy_hub, sell_hub, buy_price, sell_reference, destination_lowest_sell, profit_per_unit, spread, source_available, estimated_profit, cargo_used_percent, suggested_buy_quantity, buy_region_volume, sell_region_volume, last_refresh, notes, script_notes)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, ?10, ?11, ?12, null, null, ?13, ?14, ?15, '', ?16)",
            params![row.2, row.0, row.1, row.3, row.4, row.5, row.6, row.7, row.8, row.9, row.10, row.11, row.12, row.13, "2026-06-17T11:47:12+00:00", row.14],
        )?;
    }
    Ok(())
}

fn open(state: &State<AppState>) -> Result<Connection, String> {
    open_path(db_path(state)?)
}

fn db_path(state: &State<AppState>) -> Result<PathBuf, String> {
    state
        .db_path
        .lock()
        .map_err(|_| "Database lock poisoned".to_string())
        .map(|path| path.clone())
}

fn open_path(path: PathBuf) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(to_string)?;
    conn.busy_timeout(StdDuration::from_secs(5))
        .map_err(to_string)?;
    Ok(conn)
}

fn rows<T>(
    mapped: rusqlite::MappedRows<impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, String> {
    mapped
        .collect::<rusqlite::Result<Vec<T>>>()
        .map_err(to_string)
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn csv_f64(record: &csv::StringRecord, index: usize) -> f64 {
    record
        .get(index)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn csv_i64(record: &csv::StringRecord, index: usize) -> i64 {
    record
        .get(index)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
}

fn count_table(conn: &Connection, table: &str) -> Result<i64, String> {
    conn.query_row(&format!("select count(*) from {}", table), [], |row| {
        row.get(0)
    })
    .map_err(to_string)
}

fn discovery_summary(conn: &Connection) -> Result<DiscoverySummary, String> {
    let last = conn
        .query_row(
            "select run_time from discovery_runs order by rowid desc limit 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "Never".to_string());
    Ok(DiscoverySummary {
        known_items: count_table(conn, "item_types")?,
        market_rows: count_table(conn, "market_aggregates")?,
        candidates: count_table(conn, "candidate_products")?,
        products: count_table(conn, "products")?,
        enabled_products: conn
            .query_row(
                "select count(*) from products where enabled = 1",
                [],
                |row| row.get(0),
            )
            .map_err(to_string)?,
        last_discovery: last,
    })
}

fn api_limit_status(conn: &Connection) -> Result<ApiLimitStatus, String> {
    Ok(ApiLimitStatus {
        last_response_at: api_state(conn, "last_response_at", "Never"),
        last_status: api_state(conn, "last_status", "0")
            .parse::<i64>()
            .unwrap_or(0),
        error_limit_remain: optional_i64(api_state(conn, "error_limit_remain", "")),
        error_limit_reset: optional_i64(api_state(conn, "error_limit_reset", "")),
        retry_after: optional_i64(api_state(conn, "retry_after", "")),
        rate_limit_limit: api_state(conn, "rate_limit_limit", ""),
        rate_limit_remaining: optional_i64(api_state(conn, "rate_limit_remaining", "")),
        rate_limit_used: optional_i64(api_state(conn, "rate_limit_used", "")),
        rate_limited: api_state(conn, "rate_limited", "FALSE") == "TRUE",
        last_url: api_state(conn, "last_url", ""),
    })
}

fn auth_character_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuthCharacter> {
    Ok(AuthCharacter {
        character_id: row.get(0)?,
        character_name: row.get(1)?,
        scopes: row.get(2)?,
        expires_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn character_order_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CharacterOrder> {
    Ok(CharacterOrder {
        character_id: row.get(0)?,
        order_id: row.get(1)?,
        type_id: row.get(2)?,
        region_id: row.get(3)?,
        location_id: row.get(4)?,
        is_buy_order: row.get::<_, i64>(5)? != 0,
        price: row.get(6)?,
        volume_remain: row.get(7)?,
        volume_total: row.get(8)?,
        issued: row.get(9)?,
        duration: row.get(10)?,
        range: row.get(11)?,
        state: row.get(12)?,
        refreshed_at: row.get(13)?,
    })
}

fn log_auth_event(
    conn: &Connection,
    status: impl AsRef<str>,
    message: impl AsRef<str>,
) -> Result<(), String> {
    conn.execute(
        "insert into auth_events(happened_at, status, message) values (?1, ?2, ?3)",
        params![Utc::now().to_rfc3339(), status.as_ref(), message.as_ref()],
    )
    .map_err(to_string)?;
    Ok(())
}

fn start_eve_login_inner(conn: &Connection) -> Result<AuthCharacter, String> {
    let client_id = setting(conn, "EVE SSO client ID", "").trim().to_string();
    if client_id.is_empty() {
        let _ = log_auth_event(conn, "failed", "Set EVE SSO client ID in Settings first.");
        return Err("Set EVE SSO client ID in Settings first.".to_string());
    }
    let callback_url = setting(
        conn,
        "EVE SSO callback URL",
        "http://localhost:17890/callback",
    );
    let scopes = setting(
        conn,
        "EVE SSO scopes",
        "esi-markets.read_character_orders.v1",
    );
    let (callback_port, callback_path) = callback_parts(&callback_url)?;
    let listener = TcpListener::bind(("127.0.0.1", callback_port))
        .map_err(|error| format!("Could not listen for EVE login callback: {}", error))?;
    listener.set_nonblocking(true).map_err(to_string)?;

    let verifier = random_url_token(32);
    let state = random_url_token(24);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let auth_url = format!(
        "https://login.eveonline.com/v2/oauth/authorize/?response_type=code&redirect_uri={}&client_id={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        urlencoding::encode(&callback_url),
        urlencoding::encode(&client_id),
        urlencoding::encode(&scopes),
        urlencoding::encode(&challenge),
        urlencoding::encode(&state)
    );

    webbrowser::open(&auth_url)
        .map_err(|error| format!("Could not open EVE login page: {}", error))?;
    let _ = log_auth_event(conn, "started", "Opened EVE login in the browser.");
    let callback = wait_for_login_callback(&listener, &callback_path, StdDuration::from_secs(180))?;
    if callback
        .get("state")
        .map(|value| value.as_str())
        .unwrap_or("")
        != state
    {
        let _ = log_auth_event(conn, "failed", "EVE login state did not match.");
        return Err("EVE login state did not match. Please try again.".to_string());
    }
    if let Some(error) = callback.get("error") {
        let description = callback
            .get("error_description")
            .map(|value| format!(": {}", value))
            .unwrap_or_default();
        let _ = log_auth_event(
            conn,
            "failed",
            format!("EVE login failed: {}{}", error, description),
        );
        return Err(format!("EVE login failed: {}", error));
    }
    let code = callback
        .get("code")
        .ok_or_else(|| "EVE login did not return a code.".to_string())?;
    let token = match exchange_auth_code(&client_id, code, &verifier) {
        Ok(token) => token,
        Err(error) => {
            let _ = log_auth_event(conn, "failed", format!("Token exchange failed: {}", error));
            return Err(error);
        }
    };
    match store_auth_character(conn, token, &scopes) {
        Ok(character) => {
            let _ = log_auth_event(
                conn,
                "success",
                format!("Logged in as {}.", character.character_name),
            );
            Ok(character)
        }
        Err(error) => {
            let _ = log_auth_event(conn, "failed", format!("Could not store login: {}", error));
            Err(error)
        }
    }
}

fn callback_parts(callback_url: &str) -> Result<(u16, String), String> {
    let url = callback_url
        .strip_prefix("http://localhost:")
        .or_else(|| callback_url.strip_prefix("http://127.0.0.1:"))
        .ok_or_else(|| {
            "Callback URL must start with http://localhost: or http://127.0.0.1:".to_string()
        })?;
    let mut pieces = url.splitn(2, '/');
    let port = pieces
        .next()
        .ok_or_else(|| "Callback URL is missing a port.".to_string())?
        .parse::<u16>()
        .map_err(|_| "Callback URL port must be a number.".to_string())?;
    let path = format!("/{}", pieces.next().unwrap_or("callback"));
    Ok((port, path))
}

fn random_url_token(bytes: usize) -> String {
    let mut buffer = vec![0_u8; bytes];
    rand::thread_rng().fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

fn wait_for_login_callback(
    listener: &TcpListener,
    callback_path: &str,
    timeout: StdDuration,
) -> Result<HashMap<String, String>, String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).map_err(to_string)?;
                let request = String::from_utf8_lossy(&buffer[..read]);
                let first_line = request.lines().next().unwrap_or("");
                let target = first_line
                    .split_whitespace()
                    .nth(1)
                    .ok_or_else(|| "Invalid EVE login callback.".to_string())?;
                let (path, query) = target.split_once('?').unwrap_or((target, ""));
                let parsed = parse_query(query);
                let body = if path != callback_path {
                    "EVE Metrade received an unexpected callback path.".to_string()
                } else if let Some(error) = parsed.get("error") {
                    let description = parsed
                        .get("error_description")
                        .map(|value| format!(" {}", value))
                        .unwrap_or_default();
                    format!("EVE Metrade login failed: {}{}", error, description)
                } else {
                    "EVE Metrade received the login callback. You can close this tab and return to the app.".to_string()
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                if path != callback_path {
                    return Err(format!("Unexpected login callback path: {}", path));
                }
                return Ok(parsed);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(StdDuration::from_millis(200));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Timed out waiting for EVE login.".to_string())
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((
                urlencoding::decode(key).ok()?.to_string(),
                urlencoding::decode(value).ok()?.to_string(),
            ))
        })
        .collect()
}

fn exchange_auth_code(
    client_id: &str,
    code: &str,
    verifier: &str,
) -> Result<TokenResponse, String> {
    let mut form = HashMap::new();
    form.insert("grant_type", "authorization_code".to_string());
    form.insert("code", code.to_string());
    form.insert("client_id", client_id.to_string());
    form.insert("code_verifier", verifier.to_string());
    reqwest::blocking::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()
        .map_err(to_string)?
        .post("https://login.eveonline.com/v2/oauth/token")
        .form(&form)
        .send()
        .map_err(to_string)
        .and_then(|response| token_response(response, "EVE login token exchange"))
}

fn refresh_access_token(
    conn: &Connection,
    character_id: i64,
    client_id: &str,
    refresh_token: &str,
) -> Result<String, String> {
    let mut form = HashMap::new();
    form.insert("grant_type", "refresh_token".to_string());
    form.insert("refresh_token", refresh_token.to_string());
    form.insert("client_id", client_id.to_string());
    let token = reqwest::blocking::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()
        .map_err(to_string)?
        .post("https://login.eveonline.com/v2/oauth/token")
        .form(&form)
        .send()
        .map_err(to_string)
        .and_then(|response| token_response(response, "EVE token refresh"))?;
    let expires_at = (Utc::now() + Duration::seconds(token.expires_in)).to_rfc3339();
    conn.execute(
        "update auth_characters
         set access_token = ?1, refresh_token = coalesce(?2, refresh_token), expires_at = ?3, updated_at = ?4
         where character_id = ?5",
        params![
            token.access_token,
            token.refresh_token,
            expires_at,
            Utc::now().to_rfc3339(),
            character_id
        ],
    )
    .map_err(to_string)?;
    Ok(conn
        .query_row(
            "select access_token from auth_characters where character_id = ?1",
            params![character_id],
            |row| row.get(0),
        )
        .map_err(to_string)?)
}

fn token_response(
    response: reqwest::blocking::Response,
    label: &str,
) -> Result<TokenResponse, String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "{} failed: HTTP {} {}",
            label,
            status.as_u16(),
            body
        ));
    }
    response.json::<TokenResponse>().map_err(to_string)
}

fn store_auth_character(
    conn: &Connection,
    token: TokenResponse,
    requested_scopes: &str,
) -> Result<AuthCharacter, String> {
    let claims = parse_jwt_claims(&token.access_token)?;
    let character_id = claims
        .sub
        .rsplit(':')
        .next()
        .ok_or_else(|| "EVE token did not include a character ID.".to_string())?
        .parse::<i64>()
        .map_err(|_| "EVE token character ID was not numeric.".to_string())?;
    let character_name = claims
        .name
        .unwrap_or_else(|| format!("Character {}", character_id));
    let scopes = claims
        .scp
        .map(|values| values.join(" "))
        .unwrap_or_else(|| requested_scopes.to_string());
    let refresh_token = token.refresh_token.ok_or_else(|| {
        "EVE did not return a refresh token. Check the app is configured as a native app."
            .to_string()
    })?;
    let expires_at = (Utc::now() + Duration::seconds(token.expires_in)).to_rfc3339();
    let updated_at = Utc::now().to_rfc3339();
    conn.execute(
        "insert into auth_characters(character_id, character_name, scopes, access_token, refresh_token, expires_at, updated_at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         on conflict(character_id) do update set
           character_name = excluded.character_name,
           scopes = excluded.scopes,
           access_token = excluded.access_token,
           refresh_token = excluded.refresh_token,
           expires_at = excluded.expires_at,
           updated_at = excluded.updated_at",
        params![
            character_id,
            character_name,
            scopes,
            token.access_token,
            refresh_token,
            expires_at,
            updated_at
        ],
    )
    .map_err(to_string)?;
    conn.query_row(
        "select character_id, character_name, scopes, expires_at, updated_at from auth_characters where character_id = ?1",
        params![character_id],
        auth_character_from_row,
    )
    .map_err(to_string)
}

fn parse_jwt_claims(access_token: &str) -> Result<JwtClaims, String> {
    let payload = access_token
        .split('.')
        .nth(1)
        .ok_or_else(|| "EVE access token was not a JWT.".to_string())?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|error| format!("Could not decode EVE token: {}", error))?;
    serde_json::from_slice::<JwtClaims>(&decoded).map_err(to_string)
}

fn valid_character_access_token(conn: &Connection, character_id: i64) -> Result<String, String> {
    let (access_token, refresh_token, expires_at): (String, String, String) = conn
        .query_row(
            "select access_token, refresh_token, expires_at from auth_characters where character_id = ?1",
            params![character_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(to_string)?
        .ok_or_else(|| "Character is not logged in.".to_string())?;
    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|_| "Stored EVE token expiry is invalid.".to_string())?
        .with_timezone(&Utc);
    if expires > Utc::now() + Duration::seconds(60) {
        return Ok(access_token);
    }
    let client_id = setting(conn, "EVE SSO client ID", "").trim().to_string();
    if client_id.is_empty() {
        return Err("Set EVE SSO client ID in Settings first.".to_string());
    }
    refresh_access_token(conn, character_id, &client_id, &refresh_token)
}

fn refresh_character_orders_inner(
    conn: &Connection,
    character_id: i64,
) -> Result<Vec<CharacterOrder>, String> {
    let access_token = valid_character_access_token(conn, character_id)?;
    let base_url = setting(conn, "ESI base URL", "https://esi.evetech.net/latest");
    let url = format!(
        "{}/characters/{}/orders/?datasource=tranquility",
        base_url.trim_end_matches('/'),
        character_id
    );
    let response = reqwest::blocking::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()
        .map_err(to_string)?
        .get(&url)
        .header(
            "User-Agent",
            setting(conn, "User agent", "EVE Metrade local app"),
        )
        .bearer_auth(access_token)
        .send()
        .map_err(to_string)?;
    let status = response.status();
    let _ = record_api_limit_state(conn, &url, status.as_u16() as i64, response.headers());
    if status.as_u16() == 420 || status.as_u16() == 429 {
        wait_from_headers(response.headers(), 60);
        return Err(format!("ESI rate limit {} for {}", status.as_u16(), url));
    }
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(format!("ESI {} for {} {}", status.as_u16(), url, body));
    }
    let orders = response
        .json::<Vec<EsiCharacterOrder>>()
        .map_err(to_string)?;
    let refreshed_at = Utc::now().to_rfc3339();
    conn.execute(
        "delete from character_orders where character_id = ?1",
        params![character_id],
    )
    .map_err(to_string)?;
    for order in orders {
        conn.execute(
            "insert into character_orders(character_id, order_id, type_id, region_id, location_id, is_buy_order, price, volume_remain, volume_total, issued, duration, range, state, refreshed_at)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'open', ?13)",
            params![
                character_id,
                order.order_id,
                order.type_id,
                order.region_id,
                order.location_id,
                if order.is_buy_order { 1 } else { 0 },
                order.price,
                order.volume_remain,
                order.volume_total,
                order.issued,
                order.duration,
                order.range,
                refreshed_at
            ],
        )
        .map_err(to_string)?;
    }
    list_character_orders_inner(conn, Some(character_id))
}

fn list_character_orders_inner(
    conn: &Connection,
    character_id: Option<i64>,
) -> Result<Vec<CharacterOrder>, String> {
    if let Some(id) = character_id {
        let mut stmt = conn
            .prepare("select character_id, order_id, type_id, region_id, location_id, is_buy_order, price, volume_remain, volume_total, issued, duration, range, state, refreshed_at from character_orders where character_id = ?1 order by refreshed_at desc, order_id")
            .map_err(to_string)?;
        return rows(
            stmt.query_map(params![id], character_order_from_row)
                .map_err(to_string)?,
        );
    }
    let mut stmt = conn
        .prepare("select character_id, order_id, type_id, region_id, location_id, is_buy_order, price, volume_remain, volume_total, issued, duration, range, state, refreshed_at from character_orders order by refreshed_at desc, character_id, order_id")
        .map_err(to_string)?;
    let result = rows(
        stmt.query_map([], character_order_from_row)
            .map_err(to_string)?,
    )?;
    Ok(result)
}

fn record_api_limit_state(
    conn: &Connection,
    url: &str,
    status: i64,
    headers: &reqwest::header::HeaderMap,
) -> Result<(), String> {
    let retry_after = header_u64(headers, "retry-after")
        .map(|value| value.to_string())
        .unwrap_or_default();
    let remain = header_u64(headers, "x-esi-error-limit-remain")
        .map(|value| value.to_string())
        .unwrap_or_default();
    let reset = header_u64(headers, "x-esi-error-limit-reset")
        .map(|value| value.to_string())
        .unwrap_or_default();
    let rate_limit_limit = headers
        .get("x-ratelimit-limit")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let rate_limit_remaining = header_u64(headers, "x-ratelimit-remaining")
        .map(|value| value.to_string())
        .unwrap_or_default();
    let rate_limit_used = header_u64(headers, "x-ratelimit-used")
        .map(|value| value.to_string())
        .unwrap_or_default();
    set_api_state(conn, "last_response_at", Utc::now().to_rfc3339())?;
    set_api_state(conn, "last_status", status.to_string())?;
    set_api_state(conn, "retry_after", retry_after)?;
    set_api_state(conn, "error_limit_remain", remain)?;
    set_api_state(conn, "error_limit_reset", reset)?;
    set_api_state(conn, "rate_limit_limit", rate_limit_limit)?;
    set_api_state(conn, "rate_limit_remaining", rate_limit_remaining)?;
    set_api_state(conn, "rate_limit_used", rate_limit_used)?;
    set_api_state(
        conn,
        "rate_limited",
        if status == 420 || status == 429 {
            "TRUE".to_string()
        } else {
            "FALSE".to_string()
        },
    )?;
    set_api_state(conn, "last_url", url.to_string())?;
    Ok(())
}

fn api_state(conn: &Connection, key: &str, fallback: &str) -> String {
    conn.query_row(
        "select value from api_limit_state where key = ?1",
        params![key],
        |row| row.get(0),
    )
    .unwrap_or_else(|_| fallback.to_string())
}

fn set_api_state(conn: &Connection, key: &str, value: String) -> Result<(), String> {
    conn.execute(
        "insert into api_limit_state(key, value) values (?1, ?2) on conflict(key) do update set value = excluded.value",
        params![key, value],
    ).map_err(to_string)?;
    Ok(())
}

fn optional_i64(value: String) -> Option<i64> {
    if value.trim().is_empty() {
        None
    } else {
        value.parse::<i64>().ok()
    }
}

fn setting(conn: &Connection, key: &str, fallback: &str) -> String {
    conn.query_row(
        "select value from settings where key = ?1",
        params![key],
        |row| row.get(0),
    )
    .unwrap_or_else(|_| fallback.to_string())
}

fn set_app_state(conn: &Connection, key: &str, value: String) -> Result<(), String> {
    conn.execute("insert into app_state(key, value) values (?1, ?2) on conflict(key) do update set value = excluded.value", params![key, value]).map_err(to_string)?;
    Ok(())
}

fn seed_trade_hubs(conn: &Connection) -> rusqlite::Result<()> {
    let hubs = [
        (1, "Jita", 10000002, 60003760, 1, 1),
        (2, "Amarr", 10000043, 60008494, 1, 2),
        (3, "Dodixie", 10000032, 60011866, 1, 3),
        (4, "Rens", 10000030, 60004588, 0, 4),
        (5, "Hek", 10000042, 60005686, 0, 5),
    ];
    for hub in hubs {
        conn.execute(
            "insert into trade_hubs(id, name, region_id, station_id, enabled, priority)
             values (?1, ?2, ?3, ?4, ?5, ?6)
             on conflict(id) do update set name=excluded.name, region_id=excluded.region_id, station_id=excluded.station_id, priority=excluded.priority",
            params![hub.0, hub.1, hub.2, hub.3, hub.4, hub.5],
        )?;
    }
    Ok(())
}

fn enabled_or_all_trade_hubs(
    conn: &Connection,
    enabled_only: bool,
) -> Result<Vec<TradeHub>, String> {
    let sql = if enabled_only {
        "select id, name, region_id, station_id, enabled, priority from trade_hubs where enabled = 1 order by priority, id"
    } else {
        "select id, name, region_id, station_id, enabled, priority from trade_hubs order by priority, id"
    };
    let mut stmt = conn.prepare(sql).map_err(to_string)?;
    let mut hubs = rows(stmt.query_map([], trade_hub_from_row).map_err(to_string)?)?;
    if enabled_only {
        let max_hubs = setting(conn, "Max enabled hubs for ESI refresh", "3")
            .parse::<usize>()
            .unwrap_or(3)
            .max(2);
        hubs.truncate(max_hubs);
    }
    Ok(hubs)
}

fn trade_hub_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TradeHub> {
    Ok(TradeHub {
        id: row.get(0)?,
        name: row.get(1)?,
        region_id: row.get(2)?,
        station_id: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        priority: row.get(5)?,
    })
}

fn get_refresh_job(conn: &Connection) -> Result<RefreshJob, String> {
    conn.query_row(
        "select status, kind, current_item, scanned_count, total_count, api_calls, last_error, queued_count, started_at, finished_at from refresh_jobs where id = 1",
        [],
        |row| {
            Ok(RefreshJob {
                status: row.get(0)?,
                kind: row.get(1)?,
                current_item: row.get(2)?,
                scanned_count: row.get(3)?,
                total_count: row.get(4)?,
                api_calls: row.get(5)?,
                last_error: row.get(6)?,
                queued_count: row.get(7)?,
                started_at: row.get(8)?,
                finished_at: row.get(9)?,
            })
        },
    )
    .optional()
    .map_err(to_string)?
    .map(Ok)
    .unwrap_or_else(|| {
        Ok(RefreshJob {
            status: "idle".to_string(),
            kind: String::new(),
            current_item: String::new(),
            scanned_count: 0,
            total_count: 0,
            api_calls: 0,
            last_error: String::new(),
            queued_count: queued_refresh_count(conn).unwrap_or(0),
            started_at: String::new(),
            finished_at: String::new(),
        })
    })
}

fn set_refresh_job(conn: &Connection, job: &RefreshJob) -> Result<(), String> {
    conn.execute(
        "insert into refresh_jobs(id, status, kind, current_item, scanned_count, total_count, api_calls, last_error, queued_count, started_at, finished_at)
         values (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         on conflict(id) do update set status=excluded.status, kind=excluded.kind, current_item=excluded.current_item,
         scanned_count=excluded.scanned_count, total_count=excluded.total_count, api_calls=excluded.api_calls,
         last_error=excluded.last_error, queued_count=excluded.queued_count, started_at=excluded.started_at, finished_at=excluded.finished_at",
        params![
            &job.status,
            &job.kind,
            &job.current_item,
            job.scanned_count,
            job.total_count,
            job.api_calls,
            &job.last_error,
            job.queued_count,
            &job.started_at,
            &job.finished_at
        ],
    ).map_err(to_string)?;
    Ok(())
}

fn update_refresh_job_progress(
    conn: &Connection,
    current_item: &str,
    scanned_count: i64,
    total_count: i64,
    api_calls: i64,
    last_error: &str,
) -> Result<(), String> {
    let mut job = get_refresh_job(conn)?;
    if job.status != "running" {
        return Ok(());
    }
    job.current_item = current_item.to_string();
    job.scanned_count = scanned_count;
    job.total_count = total_count;
    job.api_calls = api_calls;
    if !last_error.is_empty() {
        job.last_error = last_error.to_string();
    }
    job.queued_count = queued_refresh_count(conn).unwrap_or(job.queued_count);
    set_refresh_job(conn, &job)
}

fn recover_stale_refresh_job(conn: &Connection) -> Result<(), String> {
    let mut job = get_refresh_job(conn)?;
    if job.status != "running" || job.started_at.trim().is_empty() {
        return Ok(());
    }
    let timeout_seconds = setting(conn, "Refresh stale timeout seconds", "600")
        .parse::<i64>()
        .unwrap_or(600)
        .max(60);
    let started = chrono::DateTime::parse_from_rfc3339(&job.started_at)
        .map_err(to_string)?
        .with_timezone(&Utc);
    let age_seconds = (Utc::now() - started).num_seconds();
    let appears_complete = job.total_count > 0 && job.scanned_count >= job.total_count;
    let effective_timeout = if appears_complete {
        120.min(timeout_seconds)
    } else {
        timeout_seconds
    };
    if age_seconds <= effective_timeout {
        return Ok(());
    }
    let last_item = if job.current_item.is_empty() {
        "unknown".to_string()
    } else {
        job.current_item.clone()
    };
    job.status = "failed".to_string();
    job.current_item = String::new();
    job.last_error = format!(
        "Refresh marked failed after {} seconds without completion. Last item: {}.",
        age_seconds, last_item
    );
    job.finished_at = Utc::now().to_rfc3339();
    set_refresh_job(conn, &job)
}

fn enqueue_refresh_product(conn: &Connection, type_id: i64) -> Result<(), String> {
    let existing: i64 = conn
        .query_row(
            "select count(*) from refresh_queue where kind = 'product' and type_id = ?1",
            params![type_id],
            |row| row.get(0),
        )
        .map_err(to_string)?;
    if existing == 0 {
        conn.execute(
            "insert into refresh_queue(kind, type_id, created_at) values ('product', ?1, ?2)",
            params![type_id, Utc::now().to_rfc3339()],
        )
        .map_err(to_string)?;
    }
    let mut job = get_refresh_job(conn)?;
    job.queued_count = queued_refresh_count(conn)?;
    set_refresh_job(conn, &job)?;
    Ok(())
}

fn queued_refresh_count(conn: &Connection) -> Result<i64, String> {
    conn.query_row("select count(*) from refresh_queue", [], |row| row.get(0))
        .map_err(to_string)
}

fn pop_next_queued_refresh(
    conn: &Connection,
) -> Result<Option<(i64, String, Option<i64>)>, String> {
    let queued = conn
        .query_row(
            "select id, kind, type_id from refresh_queue order by id limit 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(to_string)?;
    if let Some((id, kind, type_id)) = queued {
        conn.execute("delete from refresh_queue where id = ?1", params![id])
            .map_err(to_string)?;
        Ok(Some((id, kind, type_id)))
    } else {
        Ok(None)
    }
}

fn run_queued_refresh_jobs(conn: &Connection) -> Result<(), String> {
    loop {
        let Some((_id, kind, type_id)) = pop_next_queued_refresh(conn)? else {
            let mut job = get_refresh_job(conn)?;
            job.status = "done".to_string();
            job.kind = String::new();
            job.current_item = String::new();
            job.queued_count = 0;
            job.finished_at = Utc::now().to_rfc3339();
            set_refresh_job(conn, &job)?;
            return Ok(());
        };
        let mut job = get_refresh_job(conn)?;
        job.status = "running".to_string();
        job.kind = kind.clone();
        job.current_item = String::new();
        job.scanned_count = 0;
        job.total_count = 1;
        job.api_calls = 0;
        job.queued_count = queued_refresh_count(conn)?;
        job.started_at = Utc::now().to_rfc3339();
        job.finished_at = String::new();
        set_refresh_job(conn, &job)?;

        if kind == "product" {
            if let Some(queued_type_id) = type_id {
                if let Err(error) = refresh_product_inner(conn, queued_type_id) {
                    let mut failed = get_refresh_job(conn)?;
                    failed.last_error = format!("{}: {}", queued_type_id, error);
                    set_refresh_job(conn, &failed)?;
                }
            }
        }
    }
}

fn get_product(conn: &Connection, type_id: i64) -> Result<Product, String> {
    conn.query_row(
        "select type_id, name, enabled, notes from products where type_id = ?1",
        params![type_id],
        |row| {
            Ok(Product {
                type_id: row.get(0)?,
                name: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
                notes: row.get(3)?,
            })
        },
    )
    .map_err(to_string)
}

fn enabled_products(conn: &Connection) -> Result<Vec<Product>, String> {
    let mut stmt = conn
        .prepare(
            "select p.type_id, p.name, p.enabled, p.notes
         from products p
         left join candidate_products c on c.type_id = p.type_id
         left join opportunities o on o.type_id = p.type_id
         where p.enabled = 1
         order by
           case when o.last_refresh is null then 1 else 0 end desc,
           coalesce(o.estimated_profit, 0) * max(1.0, coalesce((julianday('now') - julianday(o.last_refresh)) * 24.0, 24.0)) desc,
           coalesce(c.score, 0) desc,
           p.rowid",
        )
        .map_err(to_string)?;
    let result = rows(
        stmt.query_map([], |row| {
            Ok(Product {
                type_id: row.get(0)?,
                name: row.get(1)?,
                enabled: true,
                notes: row.get(3)?,
            })
        })
        .map_err(to_string)?,
    )?;
    Ok(result)
}

fn fetch_json<T: serde::de::DeserializeOwned>(
    conn: &Connection,
    url: &str,
    api_calls: &mut i64,
    error_limit_threshold: u64,
) -> Result<T, String> {
    *api_calls += 1;
    let response = reqwest::blocking::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()
        .map_err(to_string)?
        .get(url)
        .header("User-Agent", "EVE Metrade local app")
        .send()
        .map_err(to_string)?;
    let status = response.status();
    let _ = record_api_limit_state(conn, url, status.as_u16() as i64, response.headers());
    maybe_wait_for_esi_reset(response.headers(), error_limit_threshold);
    if status.as_u16() == 404 {
        return serde_json::from_str("[]").map_err(to_string);
    }
    if status.as_u16() == 420 || status.as_u16() == 429 {
        wait_from_headers(response.headers(), 60);
        return Err(format!("ESI rate limit {} for {}", status.as_u16(), url));
    }
    if !status.is_success() {
        return Err(format!("ESI {} for {}", status.as_u16(), url));
    }
    response.json::<T>().map_err(to_string)
}

fn maybe_wait_for_esi_reset(headers: &reqwest::header::HeaderMap, threshold: u64) {
    let remain = header_u64(headers, "x-esi-error-limit-remain");
    if remain.map(|value| value <= threshold).unwrap_or(false) {
        wait_from_headers(headers, 60);
    }
}

fn wait_from_headers(headers: &reqwest::header::HeaderMap, fallback_seconds: u64) {
    let seconds = header_u64(headers, "retry-after")
        .or_else(|| header_u64(headers, "x-esi-error-limit-reset"))
        .unwrap_or(fallback_seconds)
        .clamp(1, 300);
    std::thread::sleep(StdDuration::from_secs(seconds));
}

fn header_u64(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.parse::<u64>().ok()
}

fn recent_volume(rows: &[HistoryRow]) -> f64 {
    let cutoff = Utc::now().date_naive() - Duration::days(30);
    rows.iter()
        .filter_map(|row| {
            chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
                .ok()
                .filter(|date| *date >= cutoff)
                .map(|_| row.volume as f64)
        })
        .sum()
}

fn should_skip_low_target_volume(
    conn: &Connection,
    type_id: i64,
    min_volume: f64,
) -> Result<bool, String> {
    if min_volume <= 0.0 {
        return Ok(false);
    }
    let volume: Option<f64> = conn
        .query_row(
            "select sell_region_volume from opportunities where type_id = ?1",
            params![type_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(to_string)?;
    Ok(volume.map(|value| value < min_volume).unwrap_or(false))
}

fn is_cold_opportunity(row: &Opportunity, min_volume: f64) -> bool {
    row.status == "LOW TRAFFIC"
        || (min_volume > 0.0
            && row
                .sell_region_volume
                .map(|volume| volume < min_volume)
                .unwrap_or(false))
}

fn mark_product_cold(conn: &Connection, type_id: i64) -> Result<(), String> {
    conn.execute(
        "update products set enabled = 0 where type_id = ?1",
        params![type_id],
    )
    .map_err(to_string)?;
    conn.execute(
        "update candidate_products set enabled = 0, reason = 'Disabled after ESI validation: cold target market' where type_id = ?1",
        params![type_id],
    ).map_err(to_string)?;
    Ok(())
}

fn upsert_opportunity(conn: &Connection, row: &Opportunity) -> Result<(), String> {
    conn.execute(
        "insert into opportunities(type_id, status, direction, item_name, buy_hub, sell_hub, buy_price, sell_reference, destination_lowest_sell, profit_per_unit, spread, source_available, estimated_profit, cargo_used_percent, suggested_buy_quantity, buy_region_volume, sell_region_volume, last_refresh, notes, script_notes)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
         on conflict(type_id) do update set status=excluded.status, direction=excluded.direction, item_name=excluded.item_name, buy_hub=excluded.buy_hub, sell_hub=excluded.sell_hub, buy_price=excluded.buy_price, sell_reference=excluded.sell_reference, destination_lowest_sell=excluded.destination_lowest_sell, profit_per_unit=excluded.profit_per_unit, spread=excluded.spread, source_available=excluded.source_available, estimated_profit=excluded.estimated_profit, cargo_used_percent=excluded.cargo_used_percent, suggested_buy_quantity=excluded.suggested_buy_quantity, buy_region_volume=excluded.buy_region_volume, sell_region_volume=excluded.sell_region_volume, last_refresh=excluded.last_refresh, notes=excluded.notes, script_notes=excluded.script_notes",
        params![row.type_id, row.status, row.direction, row.item_name, row.buy_hub, row.sell_hub, row.buy_price, row.sell_reference, row.destination_lowest_sell, row.profit_per_unit, row.spread, row.source_available, row.estimated_profit, row.cargo_used_percent, row.suggested_buy_quantity, row.buy_region_volume, row.sell_region_volume, row.last_refresh, row.notes, row.script_notes],
    ).map_err(to_string)?;
    Ok(())
}

fn empty_opportunity(
    status: &str,
    product: &Product,
    buy_volume: f64,
    sell_volume: f64,
    refreshed: String,
    notes: &str,
) -> Opportunity {
    Opportunity {
        status: status.to_string(),
        direction: String::new(),
        type_id: product.type_id,
        item_name: product.name.clone(),
        buy_hub: String::new(),
        sell_hub: String::new(),
        buy_price: None,
        sell_reference: None,
        destination_lowest_sell: None,
        profit_per_unit: None,
        spread: None,
        source_available: None,
        estimated_profit: None,
        cargo_used_percent: None,
        suggested_buy_quantity: None,
        my_destination_sell_price_min: None,
        my_destination_sell_price_max: None,
        my_destination_sell_quantity: None,
        my_destination_sell_order_count: None,
        buy_region_volume: Some(buy_volume),
        sell_region_volume: Some(sell_volume),
        last_refresh: Some(refreshed),
        last_refresh_minutes: Some(0),
        notes: product.notes.clone(),
        script_notes: notes.to_string(),
    }
}

fn insert_run(conn: &Connection, run: RefreshRun) -> Result<(), String> {
    conn.execute("insert into refresh_runs(refresh_time, items_scanned, opportunities_written, api_calls, errors, skipped, duration_seconds) values (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![run.refresh_time, run.items_scanned, run.opportunities_written, run.api_calls, run.errors, run.skipped, run.duration_seconds]).map_err(to_string)?;
    Ok(())
}

fn latest_refresh_run(conn: &Connection) -> Result<RefreshRun, String> {
    conn.query_row(
        "select refresh_time, items_scanned, opportunities_written, api_calls, errors, skipped, duration_seconds from refresh_runs order by rowid desc limit 1",
        [],
        refresh_run_from_row,
    )
    .map_err(to_string)
}

fn opportunity_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Opportunity> {
    let last_refresh: Option<String> = row.get(21)?;
    let minutes = last_refresh
        .as_ref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|time| (Utc::now() - time.with_timezone(&Utc)).num_minutes().max(0));
    Ok(Opportunity {
        status: row.get(0)?,
        direction: row.get(1)?,
        type_id: row.get(2)?,
        item_name: row.get(3)?,
        buy_hub: row.get(4)?,
        sell_hub: row.get(5)?,
        buy_price: row.get(6)?,
        sell_reference: row.get(7)?,
        destination_lowest_sell: row.get(8)?,
        profit_per_unit: row.get(9)?,
        spread: row.get(10)?,
        source_available: row.get(11)?,
        estimated_profit: row.get(12)?,
        cargo_used_percent: row.get(13)?,
        suggested_buy_quantity: row.get(14)?,
        my_destination_sell_price_min: row.get(15)?,
        my_destination_sell_price_max: row.get(16)?,
        my_destination_sell_quantity: row.get(17)?,
        my_destination_sell_order_count: row.get(18)?,
        buy_region_volume: row.get(19)?,
        sell_region_volume: row.get(20)?,
        last_refresh,
        last_refresh_minutes: minutes,
        notes: row.get(22)?,
        script_notes: row.get(23)?,
    })
}

fn refresh_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefreshRun> {
    Ok(RefreshRun {
        refresh_time: row.get(0)?,
        items_scanned: row.get(1)?,
        opportunities_written: row.get(2)?,
        api_calls: row.get(3)?,
        errors: row.get(4)?,
        skipped: row.get(5)?,
        duration_seconds: row.get(6)?,
    })
}
