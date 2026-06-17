use chrono::{Duration, Utc};
use flate2::read::GzDecoder;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io::Read;
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
    profit_per_unit: Option<f64>,
    spread: Option<f64>,
    source_available: Option<f64>,
    estimated_profit: Option<f64>,
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
            list_settings,
            list_refresh_runs,
            list_discovery_summary,
            list_api_limit_status,
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
fn get_refresh_status(state: State<AppState>) -> Result<RefreshJob, String> {
    let conn = open(&state)?;
    get_refresh_job(&conn)
}

#[tauri::command]
fn list_opportunities(
    state: State<AppState>,
    filters: Filters,
) -> Result<Vec<Opportunity>, String> {
    let conn = open(&state)?;
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
                o.profit_per_unit,
                o.spread,
                o.source_available,
                o.estimated_profit,
                o.buy_region_volume,
                o.sell_region_volume,
                o.last_refresh,
                coalesce(o.notes, p.notes),
                coalesce(o.script_notes, 'Awaiting ESI validation')
         from products p
         left join opportunities o on o.type_id = p.type_id
         where p.enabled = 1",
        )
        .map_err(to_string)?;
    let mut result = rows(
        stmt.query_map([], opportunity_from_row)
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
    let current = get_refresh_job(&conn)?;
    if current.status == "running" {
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
            job.finished_at = Utc::now().to_rfc3339();
            set_refresh_job(&conn, &job)
        }
        Err(error) => {
            let mut job = get_refresh_job(&conn)?;
            job.status = "failed".to_string();
            job.last_error = error;
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
    let cursor = app_state(conn, "cursor").parse::<usize>().unwrap_or(0);
    let products = enabled_products(conn)?;
    let selected: Vec<Product> = products
        .iter()
        .skip(cursor)
        .take(max_items)
        .cloned()
        .collect();
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

    let next_cursor = cursor + selected.len();
    let complete = next_cursor >= products.len();
    set_app_state(
        conn,
        "cursor",
        if complete {
            "0".to_string()
        } else {
            next_cursor.to_string()
        },
    )?;
    let skipped = format!(
        "{}{}",
        if complete {
            "Complete".to_string()
        } else {
            format!(
                "Next starts at item {} of {}",
                next_cursor + 1,
                products.len()
            )
        },
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
    let forge_region = setting(conn, "The Forge region ID", "10000002")
        .parse::<i64>()
        .unwrap_or(10000002);
    let domain_region = setting(conn, "Domain region ID", "10000043")
        .parse::<i64>()
        .unwrap_or(10000043);
    let error_limit_threshold = setting(conn, "ESI low error-limit threshold", "20")
        .parse::<u64>()
        .unwrap_or(20);
    let metadata = product_with_metadata(conn, product, &base, api_calls, error_limit_threshold);
    let product = metadata.product;
    let forge_orders: Vec<EsiOrder> = fetch_json(
        conn,
        &format!(
            "{}/markets/{}/orders/?datasource=tranquility&order_type=sell&type_id={}&page=1",
            base, forge_region, product.type_id
        ),
        api_calls,
        error_limit_threshold,
    )?;
    let domain_orders: Vec<EsiOrder> = fetch_json(
        conn,
        &format!(
            "{}/markets/{}/orders/?datasource=tranquility&order_type=sell&type_id={}&page=1",
            base, domain_region, product.type_id
        ),
        api_calls,
        error_limit_threshold,
    )?;
    let forge_history: Vec<HistoryRow> = fetch_json(
        conn,
        &format!(
            "{}/markets/{}/history/?datasource=tranquility&type_id={}",
            base, forge_region, product.type_id
        ),
        api_calls,
        error_limit_threshold,
    )?;
    let domain_history: Vec<HistoryRow> = fetch_json(
        conn,
        &format!(
            "{}/markets/{}/history/?datasource=tranquility&type_id={}",
            base, domain_region, product.type_id
        ),
        api_calls,
        error_limit_threshold,
    )?;
    Ok(analyze(
        conn,
        &product,
        metadata.volume_m3,
        forge_orders,
        domain_orders,
        recent_volume(&forge_history),
        recent_volume(&domain_history),
    ))
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
    forge_orders: Vec<EsiOrder>,
    domain_orders: Vec<EsiOrder>,
    forge_volume: f64,
    domain_volume: f64,
) -> Opportunity {
    let jita_station = setting(conn, "Jita station ID", "60003760")
        .parse::<i64>()
        .unwrap_or(60003760);
    let amarr_station = setting(conn, "Amarr station ID", "60008494")
        .parse::<i64>()
        .unwrap_or(60008494);
    let min_spread = setting(conn, "Minimum spread", "0.2")
        .parse::<f64>()
        .unwrap_or(0.2);
    let min_profit = setting(conn, "Minimum estimated profit", "500000")
        .parse::<f64>()
        .unwrap_or(500000.0);
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
    let cargo_m3 = setting(conn, "Ship cargo capacity m3", "60000")
        .parse::<f64>()
        .unwrap_or(60000.0)
        .max(0.0);
    let refreshed = Utc::now().to_rfc3339();
    let mut jita_sells: Vec<&EsiOrder> = forge_orders
        .iter()
        .filter(|order| !order.is_buy_order && order.location_id == jita_station)
        .collect();
    let mut amarr_sells: Vec<&EsiOrder> = domain_orders
        .iter()
        .filter(|order| !order.is_buy_order && order.location_id == amarr_station)
        .collect();
    jita_sells.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
    amarr_sells.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
    let jita_prices = hub_prices(&jita_sells, sell_ref_min_units, sell_ref_min_isk);
    let amarr_prices = hub_prices(&amarr_sells, sell_ref_min_units, sell_ref_min_isk);
    if jita_prices.lowest_sell == 0.0 && amarr_prices.lowest_sell == 0.0 {
        return empty_opportunity(
            "NO SELL ORDERS",
            product,
            forge_volume,
            domain_volume,
            refreshed,
            "No sell orders at either hub station",
        );
    }
    if jita_prices.lowest_sell == 0.0 {
        return empty_opportunity(
            "NO JITA SELL",
            product,
            forge_volume,
            domain_volume,
            refreshed,
            "No Jita sell orders at hub station",
        );
    }
    if amarr_prices.lowest_sell == 0.0 {
        return empty_opportunity(
            "NO AMARR SELL",
            product,
            forge_volume,
            domain_volume,
            refreshed,
            "No Amarr sell orders at hub station",
        );
    }
    let jita_to_amarr_profit = amarr_prices.reference_sell - jita_prices.lowest_sell;
    let amarr_to_jita_profit = jita_prices.reference_sell - amarr_prices.lowest_sell;
    let (buy_hub, sell_hub, buy_price, sell_reference, source_available, buy_volume, sell_volume): (&str, &str, f64, f64, f64, f64, f64) = if jita_to_amarr_profit >= amarr_to_jita_profit {
        (
            "Jita",
            "Amarr",
            jita_prices.lowest_sell,
            amarr_prices.reference_sell,
            jita_prices.available_at_lowest,
            forge_volume,
            domain_volume,
        )
    } else {
        (
            "Amarr",
            "Jita",
            amarr_prices.lowest_sell,
            jita_prices.reference_sell,
            amarr_prices.available_at_lowest,
            domain_volume,
            forge_volume,
        )
    };
    let profit = sell_reference - buy_price;
    let spread = if buy_price > 0.0 {
        profit / buy_price
    } else {
        0.0
    };
    let cargo_units = cargo_unit_capacity(cargo_m3, volume_m3);
    let estimated_units = cargo_units
        .map(|units| source_available.min(units))
        .unwrap_or(source_available);
    let estimated_profit: f64 = (estimated_units * profit).max(0.0);
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
        profit_per_unit: Some(profit),
        spread: Some(spread),
        source_available: Some(source_available),
        estimated_profit: Some(estimated_profit),
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

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        create table if not exists products(type_id integer primary key, name text not null, enabled integer not null, notes text not null);
        create table if not exists settings(key text primary key, value text not null, notes text not null);
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
          buy_hub text not null, sell_hub text not null, buy_price real, sell_reference real, profit_per_unit real,
          spread real, source_available real, estimated_profit real, buy_region_volume real, sell_region_volume real,
          last_refresh text, notes text not null, script_notes text not null
        );
        create table if not exists refresh_runs(refresh_time text not null, items_scanned integer not null, opportunities_written integer not null, api_calls integer not null, errors text not null, skipped text not null, duration_seconds integer not null);
        create table if not exists refresh_jobs(
          id integer primary key check(id = 1),
          status text not null, kind text not null, current_item text not null,
          scanned_count integer not null, total_count integer not null, api_calls integer not null,
          last_error text not null, started_at text not null, finished_at text not null
        );
        create table if not exists discovery_runs(run_time text not null, item_types_imported integer not null, market_rows_imported integer not null, candidates_found integer not null, products_enabled integer not null, errors text not null, duration_seconds integer not null);
        create table if not exists app_state(key text primary key, value text not null);
        create table if not exists api_limit_state(key text primary key, value text not null);
        "
    )?;
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
            "60000",
            "Maximum cargo volume used to cap estimated profit.",
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
            "60000",
            "Maximum cargo volume used to cap estimated profit.",
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
            "insert into opportunities(type_id, status, direction, item_name, buy_hub, sell_hub, buy_price, sell_reference, profit_per_unit, spread, source_available, estimated_profit, buy_region_volume, sell_region_volume, last_refresh, notes, script_notes)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, '', ?16)",
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

fn app_state(conn: &Connection, key: &str) -> String {
    conn.query_row(
        "select value from app_state where key = ?1",
        params![key],
        |row| row.get(0),
    )
    .unwrap_or_else(|_| "0".to_string())
}

fn set_app_state(conn: &Connection, key: &str, value: String) -> Result<(), String> {
    conn.execute("insert into app_state(key, value) values (?1, ?2) on conflict(key) do update set value = excluded.value", params![key, value]).map_err(to_string)?;
    Ok(())
}

fn get_refresh_job(conn: &Connection) -> Result<RefreshJob, String> {
    conn.query_row(
        "select status, kind, current_item, scanned_count, total_count, api_calls, last_error, started_at, finished_at from refresh_jobs where id = 1",
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
                started_at: row.get(7)?,
                finished_at: row.get(8)?,
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
            started_at: String::new(),
            finished_at: String::new(),
        })
    })
}

fn set_refresh_job(conn: &Connection, job: &RefreshJob) -> Result<(), String> {
    conn.execute(
        "insert into refresh_jobs(id, status, kind, current_item, scanned_count, total_count, api_calls, last_error, started_at, finished_at)
         values (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         on conflict(id) do update set status=excluded.status, kind=excluded.kind, current_item=excluded.current_item,
         scanned_count=excluded.scanned_count, total_count=excluded.total_count, api_calls=excluded.api_calls,
         last_error=excluded.last_error, started_at=excluded.started_at, finished_at=excluded.finished_at",
        params![
            &job.status,
            &job.kind,
            &job.current_item,
            job.scanned_count,
            job.total_count,
            job.api_calls,
            &job.last_error,
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
    set_refresh_job(conn, &job)
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
         where p.enabled = 1
         order by c.score desc, p.rowid",
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
    let response = reqwest::blocking::Client::new()
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
        "insert into opportunities(type_id, status, direction, item_name, buy_hub, sell_hub, buy_price, sell_reference, profit_per_unit, spread, source_available, estimated_profit, buy_region_volume, sell_region_volume, last_refresh, notes, script_notes)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         on conflict(type_id) do update set status=excluded.status, direction=excluded.direction, item_name=excluded.item_name, buy_hub=excluded.buy_hub, sell_hub=excluded.sell_hub, buy_price=excluded.buy_price, sell_reference=excluded.sell_reference, profit_per_unit=excluded.profit_per_unit, spread=excluded.spread, source_available=excluded.source_available, estimated_profit=excluded.estimated_profit, buy_region_volume=excluded.buy_region_volume, sell_region_volume=excluded.sell_region_volume, last_refresh=excluded.last_refresh, notes=excluded.notes, script_notes=excluded.script_notes",
        params![row.type_id, row.status, row.direction, row.item_name, row.buy_hub, row.sell_hub, row.buy_price, row.sell_reference, row.profit_per_unit, row.spread, row.source_available, row.estimated_profit, row.buy_region_volume, row.sell_region_volume, row.last_refresh, row.notes, row.script_notes],
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
        profit_per_unit: None,
        spread: None,
        source_available: None,
        estimated_profit: None,
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
    let last_refresh: Option<String> = row.get(14)?;
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
        profit_per_unit: row.get(8)?,
        spread: row.get(9)?,
        source_available: row.get(10)?,
        estimated_profit: row.get(11)?,
        buy_region_volume: row.get(12)?,
        sell_region_volume: row.get(13)?,
        last_refresh,
        last_refresh_minutes: minutes,
        notes: row.get(15)?,
        script_notes: row.get(16)?,
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
