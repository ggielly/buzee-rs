use crate::custom_types::{IgnoreAllowCacheState, OcrFailedFile, OcrRescanProgress, OcrRescanState, OcrSuccessFile, TantivyDocumentItem};
use crate::database::establish_connection;
use crate::database::models::{AllowList, BodyItem, DocumentItem, FileTypes, IgnoreList};
use crate::database::schema::{
    allow_list, body, document, file_types, ignore_list, metadata, metadata_fts,
};
use crate::db_sync::sync_status;
use crate::ipc::send_message_to_frontend;
use crate::tantivy_index;
use crate::text_extraction::Extractor;
use crate::user_prefs::{return_user_prefs_state, set_scan_running_status};
use crate::utils::{self, get_metadata};
use diesel::connection::Connection;
use diesel::{BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection};
use futures::{pin_mut, StreamExt};
use ignore::{Walk, WalkBuilder};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanSkipReason {
    /// The entry vanished between the directory listing and the stat call (NotFound).
    Disappeared,
    /// The OS denied access to the entry's metadata (PermissionDenied).
    PermissionDenied,
    /// The entry's metadata or timestamps could not be read for another reason.
    MetadataUnavailable,
    /// The entry does not match the indexing criteria (extension, name, symlink, folder...).
    NotIndexable,
}

fn should_log_scan_skip(reason: ScanSkipReason) -> bool {
    matches!(
        reason,
        ScanSkipReason::PermissionDenied | ScanSkipReason::MetadataUnavailable
    )
}

pub fn all_allowed_filetypes(
    connection: &mut SqliteConnection,
    only_allowed: bool,
) -> Vec<FileTypes> {
    let filetypes = file_types::table
        .select((
            file_types::file_type,
            file_types::file_type_category,
            file_types::file_type_allowed,
            file_types::added_by_user,
        ))
        .load::<FileTypes>(connection)
        .unwrap();

    if only_allowed {
        filetypes
            .into_iter()
            .filter(|filetype| filetype.file_type_allowed == true)
            .collect()
    } else {
        filetypes
    }
}

fn get_all_forbidden_directories() -> Vec<String> {
    let mut all_forbidden_directories: Vec<String> = vec![];
    let forbidden_directories: [&str; 4] = ["node_modules", "venv", "bower_components", "pycache"];
    all_forbidden_directories.extend(forbidden_directories.iter().map(|&s| s.to_string()));
    #[cfg(target_os = "windows")]
    {
        let windows_forbidden_directories: [&str; 6] = [
            "$RECYCLE.BIN",
            "System Volume Information",
            "AppData",
            "ProgramData",
            "Windows",
            "Program Files",
        ];
        all_forbidden_directories
            .extend(windows_forbidden_directories.iter().map(|&s| s.to_string()));
    }
    #[cfg(target_os = "macos")]
    {
        let home_dir: String = get_home_directory().unwrap();
        let mac_forbidden_directories: [&str; 2] = [
            &format!("{}/Library", home_dir),
            &format!("{}/Applications", home_dir),
        ];
        all_forbidden_directories.extend(mac_forbidden_directories.iter().map(|&s| s.to_string()));
    }
    log::info!("Forbidden directories: {:?}", all_forbidden_directories);
    all_forbidden_directories
}

fn build_walk_dir(path: &String, skip_path: Vec<String>) -> Walk {
    let mut builder = WalkBuilder::new(path);
    // The app manages its own ignore/allow lists in the DB, so disable the
    // standard .gitignore/hidden/ignore-file filters.
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            // Only prune directories whose path contains a forbidden fragment;
            // files are never filtered out here (matches the old jwalk behavior
            // of clearing read_children_path only).
            if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                return true;
            }
            let curr_path = utils::norm(entry.path().to_str().unwrap_or(""));
            !skip_path.iter().any(|x| curr_path.contains(x))
        });
    builder.build()
}

pub fn create_document_item(
    file_path: &Path,
    allowed_extensions: &Vec<String>,
) -> Result<DocumentItem, ScanSkipReason> {
    let filename = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let extension = file_path.extension().and_then(|s| s.to_str());

    // if extension is not in allowed filetypes, continue
    if extension.is_none() || !allowed_extensions.contains(&extension.unwrap().to_string()) {
        return Err(ScanSkipReason::NotIndexable);
    }
    // if filename starts with a dot or ~$, continue
    if filename.starts_with(".") || filename.starts_with("~$") {
        return Err(ScanSkipReason::NotIndexable);
    }

    // a single stat call; if it fails the file may have been moved/deleted mid-scan
    // or access may be denied, so classify the failure instead of panicking
    let metadata = match get_metadata(file_path) {
        Ok(metadata) => metadata,
        Err(e) => {
            return Err(match e.kind() {
                std::io::ErrorKind::NotFound => ScanSkipReason::Disappeared,
                std::io::ErrorKind::PermissionDenied => ScanSkipReason::PermissionDenied,
                _ => ScanSkipReason::MetadataUnavailable,
            })
        }
    };
    // if metadata is a symlink or shortcut file, continue
    if metadata.file_type().is_symlink() {
        return Err(ScanSkipReason::NotIndexable);
    }

    // non-regular entries (folders, devices...) are not handled here
    if !metadata.is_file() {
        return Err(ScanSkipReason::NotIndexable);
    }
    let filesize = metadata.len();

    // get UNIX timestamp for last_modified, last_opened and created_at and store it as text string
    // last_modified drives the re-parse heuristic, so skip the file if it is unavailable;
    // created/accessed are non-critical and fall back to 0
    let last_modified_secs = match metadata.modified() {
        Ok(t) => t
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        Err(_) => return Err(ScanSkipReason::MetadataUnavailable),
    };
    let created_at = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last_opened = metadata
        .accessed()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let file_item = DocumentItem {
        source_domain: "local".to_string(),
        created_at: created_at as i64,
        name: filename.to_string(),
        path: file_path.to_str().unwrap_or("").to_string(),
        size: Some(filesize as f64),
        file_type: extension.unwrap().to_string(),
        last_modified: last_modified_secs as i64,
        last_opened: last_opened as i64,
        last_synced: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        last_parsed: 0,
        is_pinned: false,
        frecency_rank: 0.0,
        frecency_last_accessed: 0,
        comment: None,
    };

    Ok(file_item)
}

pub fn walk_directory(
    conn: &mut SqliteConnection,
    window: &tauri::WebviewWindow,
    file_paths: Vec<String>,
    app: tauri::AppHandle,
) -> usize {
    let mut files_array: Vec<DocumentItem> = vec![];
    let all_forbidden_directories = get_all_forbidden_directories();
    let mut files_added = 0;
    let allowed_filetypes = all_allowed_filetypes(conn, true);
    let allowed_extensions: Vec<String> = allowed_filetypes
        .iter()
        .map(|filetype| filetype.file_type.to_string())
        .collect();
    // Use cached ignore/allow lists instead of querying the DB per file
    let ignore_allow_cache_ref = app.state::<Mutex<IgnoreAllowCacheState>>();
    let ignore_allow_cache = ignore_allow_cache_ref.lock().unwrap();

    let user_preferences = return_user_prefs_state(&app);

    for path in file_paths {
        log::info!("Indexing file path: {}", path);
        let walk_dir = build_walk_dir(&path, all_forbidden_directories.clone());

        for entry in walk_dir {
            // a directory may be deleted or renamed while walking; skip the entry
            // instead of panicking, and only report genuine errors
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    let is_not_found = e
                        .io_error()
                        .map(|ioe| ioe.kind() == std::io::ErrorKind::NotFound)
                        .unwrap_or(false);
                    if !is_not_found {
                        // ignore::Error Display already carries the path context.
                        log::error!("Error while walking directory: {}", e);
                    }
                    continue;
                }
            };
            let entry_path = entry.path();
            // info!("Indexing: {}", path.to_str().unwrap());

            let file_item = create_document_item(&entry_path, &allowed_extensions);
            let file_item = match file_item {
                Ok(file_item) => file_item,
                Err(reason) => {
                    // files vanishing mid-scan (NotFound) and normal criteria skips are
                    // expected, so keep those quiet; log the rest with the path
                    if should_log_scan_skip(reason) {
                        log::info!(
                            "Skipping {} during scan: {:?}",
                            entry_path.to_string_lossy(),
                            reason
                        );
                    }
                    continue;
                }
            };

            // Skip files that are ignored (override by allow list handled inside should_skip)
            if ignore_allow_cache.should_skip(&file_item.path) {
                continue;
            }

            // if user is running a manual_setup, then skip all files that are not in allowed_files or allowed_folders
            if user_preferences.manual_setup {
                if !ignore_allow_cache.is_allowed(&file_item.path) {
                    // skip this file
                    continue;
                }
            }

            // if all clear, add file_item to files_array
            files_array.push(file_item);

            // if there are 500 items in files_array, add them to the database and clear the array
            if files_array.len() == 500 {
                add_file_metadata_to_database(&files_array, conn);
                files_added += files_array.len();
                // This message gives incremental updates to the frontend
                // And is necessary for setting dbReady = true in the frontend
                send_message_to_frontend(
                    &window,
                    "files-added".to_string(),
                    "files_added".to_string(),
                    files_added.to_string(),
                );
                files_array.clear();
            }
        }
        // process the leftover files from the last iteration (because count may be < 500)
        if files_array.len() > 0 {
            // let cloned_files_array = files_array.clone();
            add_file_metadata_to_database(&files_array, conn);
            files_added += files_array.len();
            // This message sets onboardingDone = true in the frontend
            send_message_to_frontend(
                &window,
                "files-added".to_string(),
                "files_added_complete".to_string(),
                files_added.to_string(),
            );
            files_array.clear();
        }
    }

    // remove files from the database that do not exist in the filesystem
    remove_nonexistent_and_ignored_files(conn, &app);
    // add folders to the database
    add_folders_to_db(conn);
    // return number of files_added
    files_added
}

pub fn add_file_metadata_to_database(
    files_array: &Vec<DocumentItem>,
    connection: &mut SqliteConnection,
) {
    // collect all file paths from files_array
    let file_paths: Vec<_> = files_array.iter().map(|file| &file.path).collect();

    // get all existing files from the database
    let existing_files = document::table
        .select((
            document::path,
            document::last_modified,
            document::last_opened,
            document::size,
        ))
        .filter(document::path.eq_any(&file_paths))
        .load::<(String, i64, i64, Option<f64>)>(connection)
        .unwrap();

    // Build a HashMap for O(1) lookups instead of O(n) Vec scans
    let existing_map: std::collections::HashMap<String, (i64, i64, Option<f64>)> = existing_files
        .into_iter()
        .map(|(path, lm, lo, size)| (path, (lm, lo, size)))
        .collect();

    let mut files_to_add: Vec<&DocumentItem> = Vec::new();
    let mut files_to_update: Vec<&DocumentItem> = Vec::new();

    for file in files_array {
        match existing_map.get(&file.path) {
            Some((last_modified, last_opened, size)) => {
                if last_modified != &file.last_modified
                    || last_opened != &file.last_opened
                    || size != &file.size
                {
                    files_to_update.push(file);
                }
            }
            None => {
                files_to_add.push(file);
            }
        }
    }

    // add the new files to the database
    if !files_to_add.is_empty() {
        connection
            .transaction::<_, diesel::result::Error, _>(|connection| {
                diesel::insert_into(document::table)
                    .values(files_to_add)
                    .execute(connection)
            })
            .unwrap();
    }

    // Batch UPDATE: build a single CASE expression for all modified files
    if !files_to_update.is_empty() {
        log::info!(">>> Updating {} existing files", files_to_update.len());
        let mut case_clauses: Vec<String> = Vec::new();
        let mut paths: Vec<String> = Vec::new();

        for file in &files_to_update {
            case_clauses.push(format!(
                "WHEN path = ?{} THEN {}",
                paths.len() + 1,
                file.last_modified
            ));
            paths.push(file.path.clone());
        }

        let case_sql = format!(
            "UPDATE document SET last_modified = CASE {} END WHERE path IN ({})",
            case_clauses.join(" "),
            paths
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(",")
        );

        // For simplicity, fall back to individual updates if diesel can't do raw SQL easily.
        // The HashSet fix above already eliminates the O(n²) lookup; the loop here
        // is acceptable for the typical batch size (50-200 changed files).
        for file in files_to_update {
            let _ = diesel::update(document::table.filter(document::path.eq(&file.path)))
                .set((
                    document::last_modified.eq(&file.last_modified),
                    document::last_opened.eq(&file.last_opened),
                    document::size.eq(&file.size),
                ))
                .execute(connection)
                .unwrap();
        }
    }
}

pub async fn parse_content_from_files(
    conn: &mut SqliteConnection,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> usize {
    let mut files_parsed = 0;

    let document_filetypes = ["docx", "md", "pptx", "txt", "epub"];
    // Keep in sync with win_ocr::is_supported_image so every raster format that
    // Windows OCR can decode is actually indexed.
    let image_filetypes = ["png", "jpeg", "jpg", "bmp", "tif", "tiff"];
    let image_cutoff_size: f64 = 50_000.0;

    log::info!("Document filetypes: {:?}", document_filetypes);
    log::info!("Image filetypes: {:?}", image_filetypes);

    // Use cached ignore/allow lists instead of querying the DB per file.
    // Scope the lock so the MutexGuard is dropped before the async loop below.
    let all_files_data = {
        let ignore_allow_cache_ref = app.state::<Mutex<IgnoreAllowCacheState>>();
        let ignore_allow_cache = ignore_allow_cache_ref.lock().unwrap();

        let user_preferences = return_user_prefs_state(&app);

        // Get all documents from the database
        // For all files that have the filetype in the array above
        let not_pdf_files_data = document::table
            .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
            .filter(document::file_type.eq_any(document_filetypes))
            .select((
                metadata::id,
                document::id,
                document::source_domain,
                document::name,
                document::path,
                document::file_type,
                document::last_modified,
                document::last_parsed,
                document::comment,
                document::size,
            ))
            .order_by(document::size.asc())
            .load::<(
                i32,
                i32,
                String,
                String,
                String,
                String,
                i64,
                i64,
                Option<String>,
                Option<f64>,
            )>(conn)
            .unwrap();

        log::info!("Not PDF files: {}", not_pdf_files_data.len());
        let mut all_files_data = not_pdf_files_data.clone();

        if user_preferences.parse_pdfs {
            log::info!("Parsing PDFs and images");
            // Get the same for all PDF files
            let pdf_files_data = document::table
                .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
                .filter(document::file_type.eq_any(["pdf"]))
                .select((
                    metadata::id,
                    document::id,
                    document::source_domain,
                    document::name,
                    document::path,
                    document::file_type,
                    document::last_modified,
                    document::last_parsed,
                    document::comment,
                    document::size,
                ))
                .order_by(document::size.asc())
                .load::<(
                    i32,
                    i32,
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    Option<String>,
                    Option<f64>,
                )>(conn)
                .unwrap();
            // Get the same for all Image files (only files > 50KB)
            let image_files_data = document::table
                .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
                .filter(document::file_type.eq_any(image_filetypes))
                .filter(document::size.gt(image_cutoff_size))
                .select((
                    metadata::id,
                    document::id,
                    document::source_domain,
                    document::name,
                    document::path,
                    document::file_type,
                    document::last_modified,
                    document::last_parsed,
                    document::comment,
                    document::size,
                ))
                .order_by(document::size.asc())
                .load::<(
                    i32,
                    i32,
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    Option<String>,
                    Option<f64>,
                )>(conn)
                .unwrap();

            log::info!("PDF files: {}", pdf_files_data.len());
            log::info!("Image files: {}", image_files_data.len());

            // Append the pdf_files_data to all_files_data
            all_files_data = all_files_data
                .into_iter()
                .chain(pdf_files_data.into_iter())
                .collect();
            // Append the image_files_data to all_files_data
            all_files_data = all_files_data
                .into_iter()
                .chain(image_files_data.into_iter())
                .collect();
        }

        // Filter the files based on the cached ignore/allow lists and last_parsed/last_modified
        let filtered: Vec<(
            i32,
            i32,
            String,
            String,
            String,
            String,
            i64,
            i64,
            Option<String>,
            Option<f64>,
        )> = all_files_data
            .into_iter()
            .filter(|item| {
                let path = &item.4;
                // Skip files that are ignored (allow list override handled inside should_skip)
                if ignore_allow_cache.should_skip(path) {
                    return false;
                }
                // Check if last_parsed is 0 (default) OR last_modified > last_parsed
                if item.7 != 0 && item.6 < item.7 {
                    return false;
                }
                true
            })
            .collect();
        filtered
    };

    // Get sync_running status
    let mut sync_running = sync_status(&app).0;
    let total_to_parse = all_files_data.len();

    // Emit an initial progress message so the frontend can show a determinate bar.
    send_message_to_frontend(
        &window,
        "scan-progress".to_string(),
        "scan_started".to_string(),
        format!("{}", total_to_parse),
    );

    // Set up body_tantivy_items and body_items
    let mut body_items: Vec<BodyItem> = vec![];
    let mut body_tantivy_items: Vec<TantivyDocumentItem> = vec![];
    let mut body_tantivy_source_ids: Vec<i32> = vec![];
    let mut body_file_chunk_cutoff = 500;
    let mut average_body_file_size = 0.0;
    // Running sum/count used to compute the average over the files parsed since
    // the last Tantivy commit. Dividing by the number of FILES (not the number of
    // text chunks) keeps the average accurate regardless of how a file is chunked.
    let mut body_file_size_sum = 0.0;
    let mut body_file_size_count = 0f64;

    // Iterate over all_files_data and extract text from each file
    for file_item in all_files_data {
        let metadata_id = file_item.0;
        let source_id = file_item.1;
        let source_domain = file_item.2;
        let name = file_item.3;
        let path = file_item.4;
        let file_type = file_item.5;
        let last_modified = file_item.6;
        let last_parsed = file_item.7;
        let comment = file_item.8;
        let file_size = file_item.9;

        // 1. BEFORE EXTRACTING TEXT: Break the loop if sync_running is false
        if sync_running == "false" {
            break;
        }

        // If last_parsed is 0 (default) OR last_modified > last_parsed
        if last_parsed == 0 || last_modified > last_parsed {
            // Extract text from the file
            // info!("Extracting text from: {}", path.clone());
            let text = extract_text_from_path(path.clone(), file_type.clone(), &app).await;
            // If there is no text, still add this file so that next time its last_parsed is compared
            // Chunk the text into 2000 character chunks
            let chunks = chunk_text(text);

            // For each chunk, create a TantivyDocumentItem, with the body key as the chunk
            for chunk in chunks {
                body_tantivy_items.push(TantivyDocumentItem {
                    source_id: i64::from(source_id),
                    source_table: "document".to_string(),
                    source_domain: source_domain.clone(),
                    name: name.clone(),
                    url: path.clone(),
                    body: chunk.clone(),
                    file_type: file_type.clone(),
                    last_modified: i64::from(last_modified),
                    comment: comment.clone().unwrap_or_else(|| {
                        return "".to_string();
                    }),
                });
                // Also createa a BodyItem for the chunk
                body_items.push(BodyItem {
                    metadata_id: metadata_id,
                    source_id: source_id,
                    text: chunk,
                    last_parsed: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                });
            }

            body_tantivy_source_ids.push(source_id);
            body_file_size_sum += file_size.unwrap_or(0.0);
            body_file_size_count += 1.0;
            average_body_file_size = body_file_size_sum / body_file_size_count;
            files_parsed += 1;

            // Emit progressive scan progress to the frontend (e.g. every 50 files) so
            // the status bar can show a live progress bar and a scan speed.
            if files_parsed % 50 == 0 {
                send_message_to_frontend(
                    &window,
                    "scan-progress".to_string(),
                    "scan_progress".to_string(),
                    format!("{}/{}", files_parsed, total_to_parse),
                );
                log::info!("{} files parsed", files_parsed);
            }

            if files_parsed % 10 == 0 {
                if average_body_file_size >= 500_000.0 {
                    log::info!("Changing body_file_chunk_cutoff to 10");
                    body_file_chunk_cutoff = 10;
                } else if average_body_file_size >= 250_000.0 {
                    log::info!("Changing body_file_chunk_cutoff to 50");
                    body_file_chunk_cutoff = 50;
                } else {
                    log::info!("Changing body_file_chunk_cutoff to 500");
                    body_file_chunk_cutoff = 500;
                }
            }

            // if there are >= 500 items in body_tantivy_items, add them to the database and clear the array
            if body_tantivy_items.len() >= body_file_chunk_cutoff {
                log::info!("Adding {} items to Tantivy Index", body_tantivy_items.len());
                // Delete old versions and add new documents in a single commit
                let indexing_commit_response = tantivy_index::delete_and_add_docs_to_index(
                    &app,
                    &body_tantivy_source_ids,
                    &body_tantivy_items,
                );
                if indexing_commit_response.is_err() {
                    log::error!(
                        "Error updating Tantivy Index: {:?}",
                        indexing_commit_response
                    );
                } else {
                    log::info!("Successfully updated Tantivy index");
                }
                // Add all body_items to the Body table
                add_body_to_database(&body_items, conn);
                // Update last_parsed in document table for these files
                update_last_parsed_in_document_table(conn, body_tantivy_source_ids.clone());
                body_tantivy_items.clear();
                body_tantivy_source_ids.clear();
                body_file_size_sum = 0.0;
                body_file_size_count = 0.0;
                average_body_file_size = 0.0;
            }
        }

        // 2. AFTER ADDING TO DB: Break the loop if sync_running is false
        if sync_running == "false" {
            break;
        }
        // Update sync_running status
        sync_running = sync_status(&app).0;
    }

    // 1.5 process leftover files from the last iteration
    if body_tantivy_items.len() > 0 {
        // Delete old versions and add new documents in a single commit
        let indexing_commit_response = tantivy_index::delete_and_add_docs_to_index(
            &app,
            &body_tantivy_source_ids,
            &body_tantivy_items,
        );
        if indexing_commit_response.is_err() {
            log::error!(
                "Error updating Tantivy Index: {:?}",
                indexing_commit_response
            );
        }
        // Add all body_items to the Body table
        add_body_to_database(&body_items, conn);
        // Update last_parsed in document table for these files
        update_last_parsed_in_document_table(conn, body_tantivy_source_ids.clone());
        body_tantivy_items.clear();
        body_tantivy_source_ids.clear();
    }

    files_parsed
}

/// Commit a batch of parsed bodies to the Tantivy index and the `body` table,
/// then reset the in-memory vectors. Shared by the normal scan and the OCR
/// rescan so both keep the same batching behaviour.
fn commit_index_batch(
    app: &tauri::AppHandle,
    conn: &mut SqliteConnection,
    body_tantivy_items: &mut Vec<TantivyDocumentItem>,
    body_tantivy_source_ids: &mut Vec<i32>,
    body_items: &mut Vec<BodyItem>,
) {
    let indexing_commit_response =
        tantivy_index::delete_and_add_docs_to_index(app, body_tantivy_source_ids, body_tantivy_items);
    if indexing_commit_response.is_err() {
        log::error!(
            "Error updating Tantivy Index: {:?}",
            indexing_commit_response
        );
    }
    add_body_to_database(body_items, conn);
    update_last_parsed_in_document_table(conn, body_tantivy_source_ids.clone());
    body_tantivy_items.clear();
    body_tantivy_source_ids.clear();
    body_items.clear();
}

/// Emit a rich OCR rescan progress payload to the frontend as a JSON string in
/// the `data` field of an "ocr-rescan-progress" event.
fn emit_ocr_rescan_progress(
    window: &tauri::WebviewWindow,
    message: &str,
    total: usize,
    processed: usize,
    success: usize,
    failed: usize,
    threads: i64,
    current_file: &str,
    failed_files: &[OcrFailedFile],
    success_files: &[OcrSuccessFile],
) {
    let payload = OcrRescanProgress {
        message: message.to_string(),
        total,
        processed,
        success,
        failed,
        remaining: total.saturating_sub(processed),
        threads,
        current_file: current_file.to_string(),
        failed_files: failed_files.to_vec(),
        success_files: success_files.to_vec(),
    };
    let data = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to serialise OCR rescan progress: {}", e);
            return;
        }
    };
    send_message_to_frontend(window, "ocr-rescan-progress".to_string(), message.to_string(), data);
}

/// Shared OCR rescan engine. Runs extraction on the given candidates with a
/// bounded worker pool, streams progress to the frontend, records failures and
/// (unlike a normal scan) re-processes already-parsed files.
///
/// Returns `true` when all candidates were processed, `false` when cancelled.
async fn run_ocr_rescan(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    all_files_data: Vec<(
        i32, i32, String, String, String, String, i64, i64, Option<String>, Option<f64>,
    )>,
    threads: i64,
) -> bool {
    let total_to_parse = all_files_data.len();
    log::info!("OCR rescan: {} files to process", total_to_parse);

    let mut conn = establish_connection(app);
    let concurrency = threads.clamp(1, 4) as usize;

    // Emit an initial progress message so the frontend can show a determinate bar.
    emit_ocr_rescan_progress(window, "started", total_to_parse, 0, 0, 0, threads, "", &[], &[]);
    // Also emit the legacy scan_started event so the status bar's parsing bar
    // knows the total before the first scan_progress arrives.
    send_message_to_frontend(
        window,
        "scan-progress".to_string(),
        "scan_started".to_string(),
        format!("{}", total_to_parse),
    );

    let mut body_items: Vec<BodyItem> = vec![];
    let mut body_tantivy_items: Vec<TantivyDocumentItem> = vec![];
    let mut body_tantivy_source_ids: Vec<i32> = vec![];
    const BATCH_CUTOFF: usize = 500;
    let mut files_parsed = 0usize;
    let mut files_success = 0usize;
    let mut failed_files: Vec<OcrFailedFile> = vec![];
    let mut success_files: Vec<OcrSuccessFile> = vec![];
    let mut completed = true;

    // Extract text with bounded concurrency. Each in-flight task checks the
    // cancellation flag and the file integrity before starting so a stopped
    // rescan does not kick off new extractions (tasks already running are
    // allowed to finish, like the existing per-PDF OCR timeout).
    let stream = futures::stream::iter(all_files_data.into_iter().map(|item| {
        let app = app.clone();
        async move {
            let cancelled = app
                .state::<Mutex<OcrRescanState>>()
                .lock()
                .unwrap()
                .cancelled
                .load(std::sync::atomic::Ordering::SeqCst);
            let (text, error) = if cancelled {
                (String::new(), Some("Rescan cancelled".to_string()))
            } else if let Err(reason) = validate_file_for_ocr(&item.4, &item.5) {
                (String::new(), Some(reason))
            } else {
                extract_text_from_path_with_error(item.4.clone(), item.5.clone(), &app).await
            };
            (item, text, error)
        }
    }))
    .buffer_unordered(concurrency);
    pin_mut!(stream);

    // Store a copy of the handle so the inner futures can mutate state safely;
    // we read back failed files at the end.
    let state = app.state::<Mutex<OcrRescanState>>();

    while let Some((file_item, text, error)) = stream.next().await {
        // Bail out as soon as a stop was requested or the background scan was
        // turned off (e.g. via the status-bar stop button).
        if state.lock().unwrap().cancelled.load(std::sync::atomic::Ordering::SeqCst) || sync_status(app).0 == "false" {
            completed = false;
            break;
        }

        let metadata_id = file_item.0;
        let source_id = file_item.1;
        let source_domain = file_item.2;
        let name = file_item.3.clone();
        let path = file_item.4.clone();
        let file_type = file_item.5.clone();
        let last_modified = file_item.6;
        let comment = file_item.8;

        // A non-empty extracted text is a success. Empty text with no error can
        // legitimately happen (e.g. a dummy image), but after OCR we treat it as
        // a soft failure so the user can retry it from the dialog.
        let is_success = error.is_none() && !text.trim().is_empty();
        if is_success {
            files_success += 1;
            success_files.push(OcrSuccessFile {
                path: path.clone(),
                name: name.clone(),
            });
        } else {
            let why = error.unwrap_or_else(|| "No text was extracted".to_string());
            failed_files.push(OcrFailedFile {
                path: path.clone(),
                name: name.clone(),
                error: why,
            });
        }

        // Keep the shared state in sync so the frontend can pull the current
        // lists at any moment (e.g. when the user expands a list mid-run).
        {
            let state = state.lock().unwrap();
            *state.failed_files.lock().unwrap() = failed_files.clone();
            *state.success_files.lock().unwrap() = success_files.clone();
        }

        if is_success {
            for chunk in chunk_text(text) {
                body_tantivy_items.push(TantivyDocumentItem {
                    source_id: i64::from(source_id),
                    source_table: "document".to_string(),
                    source_domain: source_domain.clone(),
                    name: name.clone(),
                    url: path.clone(),
                    body: chunk.clone(),
                    file_type: file_type.clone(),
                    last_modified: i64::from(last_modified),
                    comment: comment.clone().unwrap_or_default(),
                });
                body_items.push(BodyItem {
                    metadata_id,
                    source_id,
                    text: chunk,
                    last_parsed: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                });
            }

            body_tantivy_source_ids.push(source_id);
        }

        files_parsed += 1;

        // Stream rich progress after every file so the current file, remaining
        // count and error tally stay live in the dialog. The current file is the
        // full path so the dialog can show exactly which file is being OCR-ed.
        // The full file lists are NOT re-sent here (they can be huge); the
        // frontend fetches them on demand via the getter commands, which now
        // read the incrementally-updated state.
        emit_ocr_rescan_progress(
            window,
            "progress",
            total_to_parse,
            files_parsed,
            files_success,
            failed_files.len(),
            threads,
            &path,
            &[],
            &[],
        );
        // Also emit the legacy scan-progress event so the status bar stays in sync.
        if files_parsed % 50 == 0 {
            send_message_to_frontend(
                window,
                "scan-progress".to_string(),
                "scan_progress".to_string(),
                format!("{}/{}", files_parsed, total_to_parse),
            );
            log::info!("OCR rescan: {} files parsed", files_parsed);
        }

        if body_tantivy_items.len() >= BATCH_CUTOFF {
            commit_index_batch(
                app,
                &mut conn,
                &mut body_tantivy_items,
                &mut body_tantivy_source_ids,
                &mut body_items,
            );
        }
    }

    // Flush any remaining files.
    if !body_tantivy_items.is_empty() {
        commit_index_batch(
            app,
            &mut conn,
            &mut body_tantivy_items,
            &mut body_tantivy_source_ids,
            &mut body_items,
        );
    }

    // Save the failed and success files so they can be retried/listed from the
    // dialog after the run.
    {
        let state = state.lock().unwrap();
        let mut stored = state.failed_files.lock().unwrap();
        *stored = failed_files.clone();
        let mut stored_success = state.success_files.lock().unwrap();
        *stored_success = success_files.clone();
    }

    // Final progress message, clear the "scan running" flag and notify the
    // frontend that the rescan finished (completed or cancelled).
    send_message_to_frontend(
        window,
        "scan-progress".to_string(),
        "scan_progress".to_string(),
        format!("{}/{}", files_parsed, total_to_parse),
    );
    set_scan_running_status(&mut conn, false, true, app);
    emit_ocr_rescan_progress(
        window,
        if completed { "finished" } else { "cancelled" },
        total_to_parse,
        files_parsed,
        files_success,
        failed_files.len(),
        threads,
        "",
        &failed_files,
        &success_files,
    );

    completed
}

/// OCR-only rescan of every PDF and image already present in the database.
///
/// Unlike a regular scan this ignores `last_parsed`, so already-OCR-ed files are
/// re-processed (useful after changing OCR settings). Extraction is parallelised
/// up to `threads` concurrent workers, progress is streamed to the frontend and
/// the run can be cancelled via `OcrRescanState`.
///
/// Returns `true` when all candidates were processed, `false` when cancelled.
pub async fn rescan_ocr_documents(
    app: &tauri::AppHandle,
    window: tauri::WebviewWindow,
    sort_order: String,
    threads: i64,
) -> bool {
    const IMAGE_FILETYPES: [&str; 6] = ["png", "jpeg", "jpg", "bmp", "tif", "tiff"];
    const IMAGE_CUTOFF_SIZE: f64 = 50_000.0;

    let mut conn = establish_connection(app);

    // Candidates: every PDF plus every image above the size cutoff (mirrors the
    // eligibility rules of parse_content_from_files). Ordered per the selected
    // sort preference.
    let all_files_data: Vec<(
        i32, i32, String, String, String, String, i64, i64, Option<String>, Option<f64>,
    )> = {
        let mut query = document::table
            .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
            .filter(
                document::file_type
                    .eq("pdf")
                    .or(document::file_type
                        .eq_any(IMAGE_FILETYPES)
                        .and(document::size.gt(IMAGE_CUTOFF_SIZE))),
            )
            .select((
                metadata::id,
                document::id,
                document::source_domain,
                document::name,
                document::path,
                document::file_type,
                document::last_modified,
                document::last_parsed,
                document::comment,
                document::size,
            ))
            .into_boxed();

        query = match sort_order.as_str() {
            "size_desc" => query.order_by(document::size.desc()),
            "name_asc" => query.order_by(document::name.asc()),
            "name_desc" => query.order_by(document::name.desc()),
            "modified_desc" => query.order_by(document::last_modified.desc()),
            "modified_asc" => query.order_by(document::last_modified.asc()),
            "opened_desc" => query.order_by(document::last_opened.desc()),
            "opened_asc" => query.order_by(document::last_opened.asc()),
            _ => query.order_by(document::size.asc()),
        };

        query.load(&mut conn).unwrap_or_default()
    };

    run_ocr_rescan(app, &window, all_files_data, threads).await
}

/// Re-run OCR on a specific set of files (used to retry the files that failed a
/// previous rescan). Returns `true` when all given files were processed.
pub async fn rescan_ocr_files(
    app: &tauri::AppHandle,
    window: tauri::WebviewWindow,
    paths: Vec<String>,
    threads: i64,
) -> bool {
    if paths.is_empty() {
        return true;
    }

    const IMAGE_FILETYPES: [&str; 6] = ["png", "jpeg", "jpg", "bmp", "tif", "tiff"];
    const IMAGE_CUTOFF_SIZE: f64 = 50_000.0;

    let mut conn = establish_connection(app);

    let all_files_data: Vec<(
        i32, i32, String, String, String, String, i64, i64, Option<String>, Option<f64>,
    )> = document::table
        .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
        .filter(document::path.eq_any(&paths))
        .filter(
            document::file_type
                .eq("pdf")
                .or(document::file_type
                    .eq_any(IMAGE_FILETYPES)
                    .and(document::size.gt(IMAGE_CUTOFF_SIZE))),
        )
        .select((
            metadata::id,
            document::id,
            document::source_domain,
            document::name,
            document::path,
            document::file_type,
            document::last_modified,
            document::last_parsed,
            document::comment,
            document::size,
        ))
        .load(&mut conn)
        .unwrap_or_default();

    run_ocr_rescan(app, &window, all_files_data, threads).await
}

fn add_body_to_database(body_items: &Vec<BodyItem>, connection: &mut SqliteConnection) {
    if body_items.is_empty() {
        return;
    }

    // check that metadata_ids in body_items don't already exist in body::table
    let metadata_ids: Vec<i32> = body_items.iter().map(|item| item.metadata_id).collect();
    let existing_metadata_ids: HashSet<i32> = body::table
        .select(body::metadata_id)
        .filter(body::metadata_id.eq_any(&metadata_ids))
        .load::<i32>(connection)
        .unwrap()
        .into_iter()
        .collect();

    let new_body_items: Vec<&BodyItem> = body_items
        .iter()
        .filter(|item| !existing_metadata_ids.contains(&item.metadata_id))
        .collect();

    // add unique body_items using a transaction
    if !new_body_items.is_empty() {
        connection
            .transaction::<_, diesel::result::Error, _>(|connection| {
                diesel::insert_into(body::table)
                    .values(new_body_items)
                    .execute(connection)
            })
            .unwrap();
    }

    // update the last_parsed date in body::table for all the existing_metadata_ids
    // use the last_parsed date as in the body_items
    let existing_body_items: Vec<&BodyItem> = body_items
        .iter()
        .filter(|item| existing_metadata_ids.contains(&item.metadata_id))
        .collect();
    // use a transaction to update the last_parsed date in body::table for all the existing_metadata_ids
    connection
        .transaction::<_, diesel::result::Error, _>(|connection| {
            for item in existing_body_items {
                diesel::update(body::table.filter(body::metadata_id.eq(item.metadata_id)))
                    // Refresh BOTH the full text and last_parsed so a re-OCR of an
                    // already-indexed file actually replaces its stored body text.
                    .set((
                        body::text.eq(&item.text),
                        body::last_parsed.eq(item.last_parsed),
                    ))
                    .execute(connection)
                    .unwrap();
            }
            Ok(())
        })
        .unwrap();
}

pub fn update_last_parsed_in_document_table(
    conn: &mut SqliteConnection,
    body_tantivy_source_ids: Vec<i32>,
) {
    let last_parsed_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // update last_parsed in document table for all files in body_tantivy_source_ids using a SQLite Transaction
    conn.transaction::<_, diesel::result::Error, _>(|connection| {
        diesel::update(document::table.filter(document::id.eq_any(body_tantivy_source_ids)))
            .set(document::last_parsed.eq(last_parsed_timestamp))
            .execute(connection)
    })
    .unwrap();
}

pub async fn extract_text_from_path(
    path: String,
    file_type: String,
    app: &tauri::AppHandle,
) -> String {
    let extractor: Extractor = Extractor::new();
    let extracted_text = extractor.extract_text_from_file(path, file_type, app).await;
    match extracted_text {
        Ok(text) => text,
        Err(e) => {
            log::error!("Error extracting text: {}", e);
            String::new()
        }
    }
}

/// Like `extract_text_from_path` but keeps the error message so callers (e.g. the
/// OCR rescan) can report *why* a file failed instead of just seeing empty text.
pub async fn extract_text_from_path_with_error(
    path: String,
    file_type: String,
    app: &tauri::AppHandle,
) -> (String, Option<String>) {
    let extractor: Extractor = Extractor::new();
    let extracted_text = extractor.extract_text_from_file(path, file_type, app).await;
    match extracted_text {
        Ok(text) => (text, None),
        Err(e) => {
            log::error!("Error extracting text: {}", e);
            (String::new(), Some(e.to_string()))
        }
    }
}

/// Cheap sanity check that a file is readable and does not look corrupt before we
/// spend time running OCR on it. Checks existence, non-zero size and (for the
/// formats Buzee OCRs) a magic-bytes signature match. Returns `Ok` when the file
/// looks fine, or a human-readable reason why it should be skipped.
fn validate_file_for_ocr(path: &str, file_type: &str) -> Result<(), String> {
    use std::io::Read;

    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Err(format!("File not accessible: {}", e)),
    };
    if !metadata.is_file() {
        return Err("Not a regular file".to_string());
    }
    if metadata.len() == 0 {
        return Err("File is empty".to_string());
    }

    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return Err(format!("Cannot open file: {}", e)),
    };
    let mut header = [0u8; 8];
    let read = file.read(&mut header).unwrap_or(0);

    // Expected magic bytes per file type. The matching is deliberately forgiving
    // (e.g. no BOM handling) — it only guards against clearly wrong/garbage files.
    let expected: &[u8] = match file_type {
        "pdf" => b"%PDF",
        "png" => &[0x89, 0x50, 0x4E, 0x47],
        "jpeg" | "jpg" => &[0xFF, 0xD8],
        "bmp" => b"BM",
        "tif" | "tiff" => b"II",
        _ => &[],
    };

    if !expected.is_empty() && (read < expected.len() || &header[..expected.len()] != expected) {
        return Err(format!(
            "File signature does not match a '{}' file (likely corrupt or renamed)",
            file_type
        ));
    }

    Ok(())
}

fn chunk_text(text: String) -> Vec<String> {
    const MAX_CHUNK_CHARS: usize = 2000;
    // chunk the text into 2000 character chunks, never splitting a word across
    // chunks so every word stays searchable in Tantivy. If a single word is
    // longer than the limit it is kept intact on its own chunk rather than being
    // cut in half.
    let mut chunks: Vec<String> = vec![];
    let mut chunk = String::new();

    for word in text.split_whitespace() {
        // +1 accounts for the space we append between words.
        if chunk.is_empty() || chunk.len() + word.len() + 1 <= MAX_CHUNK_CHARS {
            if !chunk.is_empty() {
                chunk.push(' ');
            }
            chunk.push_str(word);
        } else {
            chunks.push(std::mem::take(&mut chunk));
            if word.len() <= MAX_CHUNK_CHARS {
                chunk.push_str(word);
            } else {
                // Oversized word: push it alone and keep the chunk empty for the next word.
                chunks.push(word.to_string());
            }
        }
    }

    if !chunk.is_empty() {
        chunks.push(chunk);
    }

    chunks
}

pub fn remove_nonexistent_and_ignored_files(conn: &mut SqliteConnection, app: &tauri::AppHandle) {
    let all_file_paths = document::table
        .select(document::path)
        .load::<String>(conn)
        .unwrap();

    // Use cached ignore/allow lists instead of querying the DB
    let ignore_allow_cache_ref = app.state::<Mutex<IgnoreAllowCacheState>>();
    let ignore_allow_cache = ignore_allow_cache_ref.lock().unwrap();

    log::info!("All files: {}", &all_file_paths.len());

    let mut files_to_remove: Vec<String> = vec![];
    let mut files_to_remove_from_index_only: Vec<String> = vec![];
    for path in &all_file_paths {
        // Only remove a file from the database when it is confirmed missing
        // (NotFound). Permission denied or other transient stat errors must not
        // delete the row, otherwise a file that is merely locked/moved mid-scan
        // would disappear from the index.
        match std::fs::metadata(path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                files_to_remove.push(path.clone());
            }
            Err(e) => {
                log::error!("Skipping removal of {} (metadata error: {})", path, e);
                continue;
            }
        }
        // if path is in the allowed_files list (or under an allowed folder), skip removal
        if ignore_allow_cache.is_allowed(path) {
            continue;
        }
        // if path is in ignored_files with ignore_indexing=true, remove from DB
        if ignore_allow_cache.is_ignored(path) {
            files_to_remove.push(path.clone());
            continue;
        }
        // if path is in ignored_files with ignore_indexing=false, remove from index only
        if ignore_allow_cache.is_ignored_index_only(path) {
            files_to_remove_from_index_only.push(path.clone());
            continue;
        }
    }

    log::info!("Files to remove: {}", files_to_remove.len());
    log::info!(
        "Files to remove from index only: {}",
        files_to_remove_from_index_only.len()
    );

    if files_to_remove.len() > 0 {
        // create transactions of 500 files each
        let mut chunked_files_to_remove: Vec<Vec<String>> = vec![];
        let mut chunk: Vec<String> = vec![];
        for file in files_to_remove {
            if chunk.len() == 500 {
                chunked_files_to_remove.push(chunk.clone());
                chunk.clear();
            }
            chunk.push(file);
        }
        chunked_files_to_remove.push(chunk.clone());
        // remove files from the database
        for chunks_of_files_to_remove in chunked_files_to_remove {
            log::info!(
                "Removing {} files from chunk",
                chunks_of_files_to_remove.len()
            );
            remove_vector_of_file_paths_from_db(&chunks_of_files_to_remove, conn, false, app);
        }
    }

    if files_to_remove_from_index_only.len() > 0 {
        // create transactions of 500 files each
        let mut chunked_files_to_remove: Vec<Vec<String>> = vec![];
        let mut chunk: Vec<String> = vec![];
        for file in files_to_remove_from_index_only {
            if chunk.len() == 500 {
                chunked_files_to_remove.push(chunk.clone());
                chunk.clear();
            }
            chunk.push(file);
        }
        chunked_files_to_remove.push(chunk.clone());
        // remove files from the database
        for chunks_of_files_to_remove in chunked_files_to_remove {
            log::info!(
                "Removing {} files from chunk",
                chunks_of_files_to_remove.len()
            );
            remove_vector_of_file_paths_from_db(&chunks_of_files_to_remove, conn, true, app);
        }
    }
}

fn remove_vector_of_file_paths_from_db(
    file_paths: &Vec<String>,
    conn: &mut SqliteConnection,
    remove_from_index_only: bool,
    app: &tauri::AppHandle,
) {
    if file_paths.is_empty() {
        return;
    }

    // get metadata_id for all file_paths
    let metadata_ids = document::table
        .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
        .filter(document::path.eq_any(file_paths))
        .select(metadata::id)
        .load::<i32>(conn)
        .unwrap();

    if metadata_ids.is_empty() {
        return;
    }

    // first delete from Body table using metadata_ids because depends on metadata_id as foreign key
    conn.transaction::<_, diesel::result::Error, _>(|connection| {
        diesel::delete(body::table.filter(body::metadata_id.eq_any(metadata_ids.clone())))
            .execute(connection)
    })
    .unwrap();

    if !remove_from_index_only {
        // delete from Metadata_fts table using metadata_ids
        conn.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::delete(
                metadata_fts::table.filter(metadata_fts::id.eq_any(metadata_ids.clone())),
            )
            .execute(connection)
        })
        .unwrap();
        // delete from Metadata table using metadata_ids
        conn.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::delete(metadata::table.filter(metadata::id.eq_any(metadata_ids)))
                .execute(connection)
        })
        .unwrap();
        // lastly delete from Document table using file_paths because metadata depends on document_id as foreign key
        conn.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::delete(document::table.filter(document::path.eq_any(file_paths)))
                .execute(connection)
        })
        .unwrap();
    }

    // delete from the Tantivy Index using document_ids
    // get document_id for all the file_paths where last_parsed > 0 (these are the ones in tantivy index)
    let document_ids = document::table
        .filter(document::path.eq_any(file_paths))
        .filter(document::last_parsed.gt(0))
        .select(document::id)
        .load::<i32>(conn)
        .unwrap();
    if !document_ids.is_empty() {
        let indexing_commit_response =
            tantivy_index::delete_docs_from_index_with_ids(app, &document_ids);
        if indexing_commit_response.is_err() {
            log::error!(
                "Error deleting files from Tantivy Index: {:?}",
                indexing_commit_response
            );
        }
    }
}

pub fn add_folders_to_db(conn: &mut SqliteConnection) {
    // Get all file paths from the document table (excluding folders)
    let all_files = document::table
        .select(document::path)
        .filter(document::file_type.ne("folder"))
        .load::<String>(conn)
        .unwrap();

    // Get parent folders for all the files
    let all_folders: Vec<String> = all_files
        .iter()
        .filter_map(|file| {
            std::path::Path::new(file)
                .parent()
                .and_then(|parent| parent.to_str())
                .map(|parent| parent.to_string())
        })
        .collect();

    log::info!("All folders (= Num files): {}", all_folders.len());
    // Get all existing folders from the database
    let existing_folders: HashSet<String> = document::table
        .select(document::path)
        .filter(document::file_type.eq("folder"))
        .load::<String>(conn)
        .unwrap()
        .into_iter()
        .collect();

    // Iterate over all_folders and add only unique folders using HashSet for O(1) lookups
    let mut unique_folders: HashSet<String> = HashSet::new();
    for folder in &all_folders {
        if !existing_folders.contains(folder) && unique_folders.insert(folder.clone()) {
            // newly inserted, not in existing
        }
    }
    log::info!("Unique folders: {}", unique_folders.len());

    if unique_folders.is_empty() {
        return;
    }
    // Get metadata for each folder and add it to the document table.
    // A folder can disappear between listing and stat; skip it instead of panicking.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let folder_items: Vec<DocumentItem> = unique_folders
        .into_iter()
        .filter_map(|folder| {
            let folder_metadata = get_metadata(std::path::Path::new(&folder)).ok()?;
            let created_at = folder_metadata
                .created()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(now);
            let last_modified = folder_metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(now);
            let last_opened = folder_metadata
                .accessed()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(now);
            Some(DocumentItem {
                source_domain: "local".to_string(),
                created_at,
                name: std::path::Path::new(&folder)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                path: folder.to_string(),
                size: None,
                file_type: "folder".to_string(),
                last_modified,
                last_opened,
                last_synced: now,
                last_parsed: now,
                is_pinned: false,
                frecency_rank: 0.0,
                frecency_last_accessed: 0,
                comment: None,
            })
        })
        .collect();

    let _ = diesel::insert_into(document::table)
        .values(folder_items)
        .execute(conn)
        .unwrap();
}

pub fn add_path_to_ignore_list(
    path: String,
    is_folder: bool,
    ignore_indexing: bool,
    conn: &mut SqliteConnection,
) -> Result<usize, diesel::result::Error> {
    // remove path from allow_list if it exists
    let _ =
        diesel::delete(allow_list::table.filter(allow_list::path.eq(path.clone()))).execute(conn);
    // add path to ignore_list
    diesel::insert_into(ignore_list::table)
        .values(IgnoreList {
            path,
            is_folder,
            ignore_indexing,
        })
        .execute(conn)
}

pub fn get_all_ignored_paths(conn: &mut SqliteConnection) -> Vec<IgnoreList> {
    // get all columns from ignore_list except id
    ignore_list::table
        .select((
            ignore_list::path,
            ignore_list::is_folder,
            ignore_list::ignore_indexing,
        ))
        .load::<IgnoreList>(conn)
        .unwrap()
}

pub fn remove_paths_from_ignore_list(
    paths: Vec<String>,
    conn: &mut SqliteConnection,
) -> Result<usize, diesel::result::Error> {
    log::info!("Removing {} paths from ignore_list", paths.len());
    // remove paths from ignore_list
    conn.transaction::<_, diesel::result::Error, _>(|connection| {
        diesel::delete(ignore_list::table.filter(ignore_list::path.eq_any(paths)))
            .execute(connection)
    })
}

pub fn add_path_to_allow_list(
    path: String,
    is_folder: bool,
    conn: &mut SqliteConnection,
) -> Result<usize, diesel::result::Error> {
    // remove path from ignore_list if it exists
    let _ =
        diesel::delete(ignore_list::table.filter(ignore_list::path.eq(path.clone()))).execute(conn);
    // add path to allow_list
    diesel::insert_into(allow_list::table)
        .values(AllowList { path, is_folder })
        .execute(conn)
}

pub fn get_all_allowed_paths(conn: &mut SqliteConnection) -> Vec<AllowList> {
    // get all columns from allow_list except id
    allow_list::table
        .select((allow_list::path, allow_list::is_folder))
        .load::<AllowList>(conn)
        .unwrap()
}

pub fn clear_last_parsed_dates_from_db(conn: &mut SqliteConnection) {
    // set last_parsed to 0 for all files in the document table
    diesel::update(document::table)
        .set(document::last_parsed.eq(0))
        .execute(conn)
        .unwrap();
    // set last_parsed to 0 for all files in the body table
    diesel::update(body::table)
        .set(body::last_parsed.eq(0))
        .execute(conn)
        .unwrap();
}

#[cfg(test)]
mod chunk_tests {
    use super::chunk_text;

    #[test]
    fn short_text_is_a_single_chunk() {
        let chunks = chunk_text("the quick brown fox".to_string());
        assert_eq!(chunks, vec!["the quick brown fox"]);
    }

    #[test]
    fn long_text_is_split_into_bounded_chunks() {
        // 100 words of 30 chars + spaces easily exceeds a 2000 char chunk.
        let text = vec!["word"; 1000].join(" ");
        let chunks = chunk_text(text);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(
                chunk.chars().count() <= 2000,
                "chunk too long: {}",
                chunk.chars().count()
            );
        }
    }

    #[test]
    fn words_are_never_split_across_chunks() {
        // A single 5000-char word is placed on its own chunk untouched, not cut.
        let long_word = "w".repeat(5000);
        let text = format!("abc {} def", long_word);
        let chunks = chunk_text(text);
        assert!(chunks.iter().any(|chunk| chunk == &long_word));
        // The other words must remain intact too.
        let all: Vec<&str> = chunks.iter().map(|c| c.as_str()).collect();
        assert!(all.iter().any(|c| c.contains("abc")));
        assert!(all.iter().any(|c| c.contains("def")));
    }

    #[test]
    fn empty_text_yields_no_chunks() {
        assert!(chunk_text(String::new()).is_empty());
        assert!(chunk_text("   \n\n  ".to_string()).is_empty());
    }
}
