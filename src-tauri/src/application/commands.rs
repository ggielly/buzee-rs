use crate::application::AppServices;
use crate::domain::types::{AppStatistics, DBStat, DashboardStats, DateLimit};
use crate::infrastructure::database::models::{DocumentSearchResult, IgnoreList};
use crate::infrastructure::database::{self, search};
use crate::infrastructure::indexing;
use crate::infrastructure::platform::{hotkey, lifecycle};
use crate::infrastructure::tantivy_index;
use crate::infrastructure::user_prefs;
use crate::infrastructure::utils;
use std::sync::Arc;

/// Blocking command handlers executed on the worker thread. Each returns a typed
/// result; the caller maps it onto a `UiEvent`.
pub fn search_fts(
    services: &Arc<AppServices>,
    query: String,
    page: i32,
    limit: i32,
    file_type: Option<String>,
    date_limit: Option<DateLimit>,
) -> Vec<DocumentSearchResult> {
    let conn = services.db_pool.get().expect("Failed to get DB connection");
    search::search_fts_index(
        query,
        page,
        limit,
        file_type,
        date_limit,
        conn,
        &services.tantivy_reader,
        &services.tantivy_index,
    )
    .unwrap_or_default()
}

pub fn search_suggestions(services: &Arc<AppServices>, query: String) -> Vec<String> {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    search::get_metadata_title_matches(query, &mut conn).unwrap_or_default()
}

pub fn recent_docs(
    services: &Arc<AppServices>,
    page: i32,
    limit: i32,
    file_type: Option<String>,
) -> Vec<DocumentSearchResult> {
    let conn = services.db_pool.get().expect("Failed to get DB connection");
    search::get_recently_opened_docs(page, limit, file_type, conn).unwrap_or_default()
}

pub fn db_stats(services: &Arc<AppServices>) -> Vec<DBStat> {
    let conn = services.db_pool.get().expect("Failed to get DB connection");
    search::get_counts_for_all_filetypes(conn).unwrap_or_default()
}

pub fn count_parsed(services: &Arc<AppServices>) -> i64 {
    search::get_file_parsed_count(
        services.db_pool.get().expect("Failed to get DB connection"),
    )
    .unwrap_or_default()
}

pub fn text_for_file(services: &Arc<AppServices>, document_id: i32) -> Vec<String> {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    search::get_parsed_text_for_file(document_id, &mut conn).unwrap_or_default()
}

pub async fn extract_pdf_text(services: &Arc<AppServices>, file_path: String) -> Vec<String> {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    utils::extract_text_from_pdf(file_path, &mut conn, &services.db_pool, &services.preferences)
        .await
        .unwrap_or_default()
}

pub fn write_text(services: &Arc<AppServices>, file_path: String, text: String) {
    let _ = services;
    let _ = std::fs::File::create(&file_path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
}

pub fn read_text(services: &Arc<AppServices>, file_path: String) -> String {
    let _ = services;
    std::fs::read_to_string(file_path).unwrap_or_default()
}

pub fn image_base64(services: &Arc<AppServices>, file_path: String) -> String {
    let _ = services;
    use base64::prelude::*;
    std::fs::read(file_path)
        .map(|data| BASE64_STANDARD.encode(&data))
        .unwrap_or_default()
}

pub fn chrome_profiles() -> Vec<String> {
    crate::infrastructure::browser_readers::get_chrome_profiles()
}

pub fn arc_profiles() -> Vec<String> {
    crate::infrastructure::browser_readers::get_arc_profiles()
}

pub fn browser_history(
    profile: String,
    query: String,
    limit: i32,
    page: i32,
) -> Vec<DocumentSearchResult> {
    crate::infrastructure::browser_readers::search_chrome(
        profile,
        query,
        limit as i64,
        page as i64,
    )
    .unwrap_or_default()
}

pub fn tantivy_files_search(
    services: &Arc<AppServices>,
    query: String,
    limit: i32,
    page: i32,
) -> Vec<crate::domain::types::TantivyDocumentSearchResult> {
    let searcher = tantivy_index::acquire_searcher_from_reader(&services.tantivy_reader);
    let top_docs = tantivy_index::parse_query_and_get_top_docs(
        &services.tantivy_index,
        &searcher,
        query,
        limit,
        page * limit,
    )
    .unwrap_or_default();
    tantivy_index::return_document_search_results(&services.tantivy_index, &searcher, top_docs)
        .unwrap_or_default()
}

pub fn tantivy_bookmarks_search(
    services: &Arc<AppServices>,
    query: String,
    limit: i32,
    page: i32,
) -> Vec<crate::domain::types::TantivyBookmarkSearchResult> {
    let searcher = tantivy_index::acquire_searcher_from_reader(&services.tantivy_reader);
    let top_docs = tantivy_index::parse_query_and_get_top_docs(
        &services.tantivy_index,
        &searcher,
        query,
        limit,
        page * limit,
    )
    .unwrap_or_default();
    tantivy_index::return_bookmark_search_results(&services.tantivy_index, &searcher, top_docs)
        .unwrap_or_default()
}

pub fn csv_dump(services: &Arc<AppServices>) {
    let _ = services;
    log::info!("CSV dump requested");
}

pub fn clear_index(services: &Arc<AppServices>) {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    let _ = tantivy_index::delete_all_docs_from_index(&services.tantivy_writer);
    indexing::clear_last_parsed_dates_from_db(&mut conn);
}

pub fn ignore_path(services: &Arc<AppServices>, path: String, is_folder: bool, ignore_indexing: bool) {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    if let Err(e) = indexing::add_path_to_ignore_list(path.clone(), is_folder, ignore_indexing, &mut conn) {
        log::error!("Failed to add path to ignore list: {}", e);
        return;
    }
    refresh_ignore_allow_cache(services);
    indexing::remove_nonexistent_and_ignored_files(
        &mut conn,
        &services.ignore_allow_cache.read().unwrap_or_else(|e| e.into_inner()).clone(),
        &services.tantivy_writer,
    );
}

pub fn remove_from_ignore_list(services: &Arc<AppServices>, paths: Vec<String>) {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    let _ = indexing::remove_paths_from_ignore_list(paths, &mut conn);
    refresh_ignore_allow_cache(services);
}

pub fn ignored_paths(services: &Arc<AppServices>) -> Vec<IgnoreList> {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    indexing::get_all_ignored_paths(&mut conn)
}

fn refresh_ignore_allow_cache(services: &Arc<AppServices>) {
    if let Ok(mut conn) = database::establish_direct_connection_to_db() {
        let cache = crate::domain::IgnoreAllowCacheState::from_db(&mut conn);
        if let Ok(mut current) = services.ignore_allow_cache.write() {
            *current = cache;
        }
    }
}

pub fn statistics(services: &Arc<AppServices>) -> AppStatistics {
    crate::infrastructure::statistics::get_app_statistics(
        &services.db_pool,
        services.sync.running.load(std::sync::atomic::Ordering::SeqCst),
        services.sync.last_sync_time.load(std::sync::atomic::Ordering::SeqCst),
        &services.preferences.read().unwrap_or_else(|e| e.into_inner()).clone(),
    )
}

pub fn dashboard(services: &Arc<AppServices>) -> DashboardStats {
    crate::infrastructure::dashboard::get_dashboard_stats(
        &services.db_pool,
        services.sync.running.load(std::sync::atomic::Ordering::SeqCst),
        services.sync.last_sync_time.load(std::sync::atomic::Ordering::SeqCst),
        &services.preferences.read().unwrap_or_else(|e| e.into_inner()).clone(),
    )
}

pub fn set_user_preference(
    services: &Arc<AppServices>,
    key: String,
    value: bool,
) -> Result<(), String> {
    let pool = &services.db_pool;
    match key.as_str() {
        "launch_at_startup" => user_prefs::set_launch_at_startup_flag_in_db(value, pool),
        "show_search_suggestions" => user_prefs::set_show_search_suggestions_flag_in_db(value, pool),
        "onboarding_done" => user_prefs::set_onboarding_done_flag_in_db(value, pool),
        "automatic_background_sync" => {
            user_prefs::set_automatic_background_sync_flag_in_db(value, pool)
        }
        "detailed_scan" => user_prefs::set_detailed_scan_flag_in_db(value, pool),
        "roadmap_survey_answered" => user_prefs::set_roadmap_survey_answered_flag_in_db(value, pool),
        "parse_pdfs" => user_prefs::set_parse_pdfs_flag_in_db(value, pool),
        "manual_setup" => user_prefs::set_manual_setup_flag_in_db(value, pool),
        "enable_logs" => user_prefs::set_enable_logs_flag_in_db(value, pool),
        "global_shortcut_enabled" => user_prefs::set_global_shortcut_flag_in_db(value, pool),
        _ => return Err("Invalid preference key".to_string()),
    }

    user_prefs::set_user_preferences_state_from_db_value(&services.preferences, pool);
    Ok(())
}

pub fn reset_user_preferences(services: &Arc<AppServices>) {
    let mut conn = services.db_pool.get().expect("Failed to get DB connection");
    user_prefs::set_default_user_prefs(&mut conn, true);
    user_prefs::set_user_preferences_state_from_db_value(&services.preferences, &services.db_pool);
    lifecycle::graceful_restart(30);
}

pub fn set_pdf_max_ocr_pages(services: &Arc<AppServices>, pages: i64) {
    let pages = pages.clamp(1, 5000);
    user_prefs::set_pdf_max_ocr_pages_in_db(pages, &services.db_pool);
    user_prefs::set_user_preferences_state_from_db_value(&services.preferences, &services.db_pool);
}

pub fn set_ocr_threads(services: &Arc<AppServices>, threads: i64) {
    let threads = threads.clamp(1, 4);
    user_prefs::set_ocr_threads_in_db(threads, &services.db_pool);
    user_prefs::set_user_preferences_state_from_db_value(&services.preferences, &services.db_pool);
}

pub fn set_ocr_sort_order(services: &Arc<AppServices>, sort_order: String) {
    user_prefs::set_ocr_sort_order_in_db(sort_order, &services.db_pool);
    user_prefs::set_user_preferences_state_from_db_value(&services.preferences, &services.db_pool);
}

pub fn set_new_global_shortcut(services: &Arc<AppServices>, new_shortcut_string: String) {
    let new_shortcut_string = user_prefs::fix_global_shortcut_string(new_shortcut_string);
    let _ = hotkey::parse_hotkey(&new_shortcut_string);
    user_prefs::set_new_global_shortcut_in_db(new_shortcut_string, &services.db_pool);
    user_prefs::set_user_preferences_state_from_db_value(&services.preferences, &services.db_pool);
    lifecycle::graceful_restart(30);
}