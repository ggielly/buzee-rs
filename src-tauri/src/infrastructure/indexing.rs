use crate::domain::types::{DbPool, TantivyDocumentItem, UserPreferencesState};
use crate::infrastructure::database::models::{
    AllowList, DocumentItem, FileTypes, IgnoreList,
};
use crate::infrastructure::database::schema::{
    allow_list, body, document, file_types, ignore_list, metadata, metadata_fts,
};
use crate::infrastructure::tantivy_index;
use crate::infrastructure::text_extraction::Extractor;
use crate::infrastructure::utils::{get_metadata, norm};
use diesel::connection::Connection;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection,
};
use ignore::{Walk, WalkBuilder};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanSkipReason {
    Disappeared,
    PermissionDenied,
    MetadataUnavailable,
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
    let filetypes = match file_types::table
        .select((
            file_types::file_type,
            file_types::file_type_category,
            file_types::file_type_allowed,
            file_types::added_by_user,
        ))
        .load::<FileTypes>(connection)
    {
        Ok(filetypes) => filetypes,
        Err(e) => {
            log::error!("Could not load allowed file types: {}", e);
            return Vec::new();
        }
    };

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
    use crate::infrastructure::housekeeping::get_home_directory;
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
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                return true;
            }
            let curr_path = norm(entry.path().to_str().unwrap_or(""));
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

    if extension.is_none() || !allowed_extensions.contains(&extension.unwrap().to_string()) {
        return Err(ScanSkipReason::NotIndexable);
    }
    if filename.starts_with(".") || filename.starts_with("~$") {
        return Err(ScanSkipReason::NotIndexable);
    }

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
    if metadata.file_type().is_symlink() {
        return Err(ScanSkipReason::NotIndexable);
    }

    if !metadata.is_file() {
        return Err(ScanSkipReason::NotIndexable);
    }
    let filesize = metadata.len();

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

pub fn add_file_metadata_to_database(
    files_array: &Vec<DocumentItem>,
    connection: &mut SqliteConnection,
) {
    let file_paths: Vec<_> = files_array.iter().map(|file| &file.path).collect();

    let existing_files = match document::table
        .select((
            document::path,
            document::last_modified,
            document::last_opened,
            document::size,
        ))
        .filter(document::path.eq_any(&file_paths))
        .load::<(String, i64, i64, Option<f64>)>(connection)
    {
        Ok(files) => files,
        Err(e) => {
            log::error!("Could not load existing files for update: {}", e);
            return;
        }
    };

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

    if !files_to_add.is_empty() {
        if let Err(e) = connection.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::insert_into(document::table)
                .values(files_to_add)
                .execute(connection)
        }) {
            log::error!("Could not insert new files: {}", e);
        }
    }

    if !files_to_update.is_empty() {
        log::info!(">>> Updating {} existing files", files_to_update.len());
        for file in files_to_update {
            if let Err(e) = diesel::update(document::table.filter(document::path.eq(&file.path)))
                .set((
                    document::last_modified.eq(&file.last_modified),
                    document::last_opened.eq(&file.last_opened),
                    document::size.eq(&file.size),
                ))
                .execute(connection)
            {
                log::error!("Could not update file {:?}: {}", file.path, e);
            }
        }
    }
}

pub fn add_body_to_database_public(
    body_items: &Vec<crate::infrastructure::database::models::BodyItem>,
    connection: &mut SqliteConnection,
) {
    add_body_to_database(body_items, connection);
}

fn add_body_to_database(body_items: &Vec<crate::infrastructure::database::models::BodyItem>, connection: &mut SqliteConnection) {
    if body_items.is_empty() {
        return;
    }

    let metadata_ids: Vec<i32> = body_items.iter().map(|item| item.metadata_id).collect();
    let existing_metadata_ids: HashSet<i32> = match body::table
        .select(body::metadata_id)
        .filter(body::metadata_id.eq_any(&metadata_ids))
        .load::<i32>(connection)
    {
        Ok(ids) => ids.into_iter().collect(),
        Err(e) => {
            log::error!("Could not load existing body rows: {}", e);
            return;
        }
    };

    let new_body_items: Vec<&crate::infrastructure::database::models::BodyItem> = body_items
        .iter()
        .filter(|item| !existing_metadata_ids.contains(&item.metadata_id))
        .collect();

    if !new_body_items.is_empty() {
        if let Err(e) = connection.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::insert_into(body::table)
                .values(new_body_items)
                .execute(connection)
        }) {
            log::error!("Could not insert body rows: {}", e);
        }
    }

    let existing_body_items: Vec<&crate::infrastructure::database::models::BodyItem> = body_items
        .iter()
        .filter(|item| existing_metadata_ids.contains(&item.metadata_id))
        .collect();
    if let Err(e) = connection.transaction::<_, diesel::result::Error, _>(|connection| {
        for item in existing_body_items {
            diesel::update(body::table.filter(body::metadata_id.eq(item.metadata_id)))
                .set((
                    body::text.eq(&item.text),
                    body::last_parsed.eq(item.last_parsed),
                ))
                .execute(connection)?;
        }
        Ok(())
    }) {
        log::error!("Could not update body rows: {}", e);
    }
}

pub fn update_last_parsed_in_document_table(
    conn: &mut SqliteConnection,
    body_tantivy_source_ids: Vec<i32>,
) {
    let last_parsed_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if let Err(e) = conn.transaction::<_, diesel::result::Error, _>(|connection| {
        diesel::update(document::table.filter(document::id.eq_any(body_tantivy_source_ids)))
            .set(document::last_parsed.eq(last_parsed_timestamp))
            .execute(connection)
    }) {
        log::error!("Could not update last_parsed timestamps: {}", e);
    }
}

pub async fn extract_text_from_path(
    path: String,
    file_type: String,
    pool: &DbPool,
    preferences: &Arc<RwLock<UserPreferencesState>>,
) -> String {
    let extractor: Extractor = Extractor::new();
    let extracted_text = extractor
        .extract_text_from_file(path, file_type, pool, preferences)
        .await;
    match extracted_text {
        Ok(text) => text,
        Err(e) => {
            log::error!("Error extracting text: {}", e);
            String::new()
        }
    }
}

pub async fn extract_text_from_path_with_error(
    path: String,
    file_type: String,
    pool: &DbPool,
    preferences: &Arc<RwLock<UserPreferencesState>>,
) -> (String, Option<String>) {
    let extractor: Extractor = Extractor::new();
    let extracted_text = extractor
        .extract_text_from_file(path, file_type, pool, preferences)
        .await;
    match extracted_text {
        Ok(text) => (text, None),
        Err(e) => {
            log::error!("Error extracting text: {}", e);
            (String::new(), Some(e.to_string()))
        }
    }
}

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

pub fn chunk_text(text: String) -> Vec<String> {
    const MAX_CHUNK_CHARS: usize = 2000;
    let mut chunks: Vec<String> = vec![];
    let mut chunk = String::new();

    for word in text.split_whitespace() {
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
                chunks.push(word.to_string());
            }
        }
    }

    if !chunk.is_empty() {
        chunks.push(chunk);
    }

    chunks
}

pub fn remove_nonexistent_and_ignored_files(
    conn: &mut SqliteConnection,
    ignore_allow_cache: &crate::domain::IgnoreAllowCacheState,
    tantivy_writer: &Mutex<tantivy::IndexWriter>,
) {
    let all_file_paths = match document::table.select(document::path).load::<String>(conn) {
        Ok(paths) => paths,
        Err(e) => {
            log::error!("Could not load file paths for cleanup: {}", e);
            return;
        }
    };

    log::info!("All files: {}", &all_file_paths.len());

    let mut files_to_remove: Vec<String> = vec![];
    let mut files_to_remove_from_index_only: Vec<String> = vec![];
    for path in &all_file_paths {
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
        if ignore_allow_cache.is_allowed(path) {
            continue;
        }
        if ignore_allow_cache.is_ignored(path) {
            files_to_remove.push(path.clone());
            continue;
        }
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
        let mut chunked_files_to_remove: Vec<Vec<String>> = vec![];
        let mut chunk: Vec<String> = vec![];
        for file in files_to_remove {
            if chunk.len() == 500 {
                chunked_files_to_remove.push(std::mem::take(&mut chunk));
            }
            chunk.push(file);
        }
        chunked_files_to_remove.push(chunk);
        for chunks_of_files_to_remove in chunked_files_to_remove {
            remove_vector_of_file_paths_from_db(&chunks_of_files_to_remove, conn, false, tantivy_writer);
        }
    }

    if files_to_remove_from_index_only.len() > 0 {
        let mut chunked_files_to_remove: Vec<Vec<String>> = vec![];
        let mut chunk: Vec<String> = vec![];
        for file in files_to_remove_from_index_only {
            if chunk.len() == 500 {
                chunked_files_to_remove.push(std::mem::take(&mut chunk));
            }
            chunk.push(file);
        }
        chunked_files_to_remove.push(chunk);
        for chunks_of_files_to_remove in chunked_files_to_remove {
            remove_vector_of_file_paths_from_db(&chunks_of_files_to_remove, conn, true, tantivy_writer);
        }
    }
}

fn remove_vector_of_file_paths_from_db(
    file_paths: &Vec<String>,
    conn: &mut SqliteConnection,
    remove_from_index_only: bool,
    tantivy_writer: &Mutex<tantivy::IndexWriter>,
) {
    if file_paths.is_empty() {
        return;
    }

    let metadata_ids = match document::table
        .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
        .filter(document::path.eq_any(file_paths))
        .select(metadata::id)
        .load::<i32>(conn)
    {
        Ok(ids) => ids,
        Err(e) => {
            log::error!("Could not load metadata ids for removal: {}", e);
            return;
        }
    };

    if metadata_ids.is_empty() {
        return;
    }

    if let Err(e) = conn.transaction::<_, diesel::result::Error, _>(|connection| {
        diesel::delete(body::table.filter(body::metadata_id.eq_any(metadata_ids.clone())))
            .execute(connection)
    }) {
        log::error!("Could not delete body rows: {}", e);
    }

    if !remove_from_index_only {
        if let Err(e) = conn.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::delete(
                metadata_fts::table.filter(metadata_fts::id.eq_any(metadata_ids.clone())),
            )
            .execute(connection)
        }) {
            log::error!("Could not delete metadata_fts rows: {}", e);
        }
        if let Err(e) = conn.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::delete(metadata::table.filter(metadata::id.eq_any(metadata_ids)))
                .execute(connection)
        }) {
            log::error!("Could not delete metadata rows: {}", e);
        }
        if let Err(e) = conn.transaction::<_, diesel::result::Error, _>(|connection| {
            diesel::delete(document::table.filter(document::path.eq_any(file_paths)))
                .execute(connection)
        }) {
            log::error!("Could not delete document rows: {}", e);
        }
    }

    let document_ids = match document::table
        .filter(document::path.eq_any(file_paths))
        .filter(document::last_parsed.gt(0))
        .select(document::id)
        .load::<i32>(conn)
    {
        Ok(ids) => ids,
        Err(e) => {
            log::error!("Could not load document ids for index removal: {}", e);
            return;
        }
    };
    if !document_ids.is_empty() {
        let indexing_commit_response =
            tantivy_index::delete_docs_from_index_with_ids(tantivy_writer, &document_ids);
        if indexing_commit_response.is_err() {
            log::error!(
                "Error deleting files from Tantivy Index: {:?}",
                indexing_commit_response
            );
        }
    }
}

pub fn add_folders_to_db(conn: &mut SqliteConnection) {
    let all_files = match document::table
        .select(document::path)
        .filter(document::file_type.ne("folder"))
        .load::<String>(conn)
    {
        Ok(files) => files,
        Err(e) => {
            log::error!("Could not load files for folder scan: {}", e);
            return;
        }
    };

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
    let existing_folders: HashSet<String> = match document::table
        .select(document::path)
        .filter(document::file_type.eq("folder"))
        .load::<String>(conn)
    {
        Ok(folders) => folders.into_iter().collect(),
        Err(e) => {
            log::error!("Could not load existing folders: {}", e);
            return;
        }
    };

    let mut unique_folders: HashSet<String> = HashSet::new();
    for folder in &all_folders {
        if !existing_folders.contains(folder) && unique_folders.insert(folder.clone()) {
        }
    }
    log::info!("Unique folders: {}", unique_folders.len());

    if unique_folders.is_empty() {
        return;
    }
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

    if let Err(e) = diesel::insert_into(document::table)
        .values(folder_items)
        .execute(conn)
    {
        log::error!("Could not insert folder rows: {}", e);
    }
}

pub fn add_path_to_ignore_list(
    path: String,
    is_folder: bool,
    ignore_indexing: bool,
    conn: &mut SqliteConnection,
) -> Result<usize, diesel::result::Error> {
    let _ =
        diesel::delete(allow_list::table.filter(allow_list::path.eq(path.clone()))).execute(conn);
    diesel::insert_into(ignore_list::table)
        .values(IgnoreList {
            path,
            is_folder,
            ignore_indexing,
        })
        .execute(conn)
}

pub fn get_all_ignored_paths(conn: &mut SqliteConnection) -> Vec<IgnoreList> {
    match ignore_list::table
        .select((
            ignore_list::path,
            ignore_list::is_folder,
            ignore_list::ignore_indexing,
        ))
        .load::<IgnoreList>(conn)
    {
        Ok(paths) => paths,
        Err(e) => {
            log::error!("Could not load ignored paths: {}", e);
            Vec::new()
        }
    }
}

pub fn remove_paths_from_ignore_list(
    paths: Vec<String>,
    conn: &mut SqliteConnection,
) -> Result<usize, diesel::result::Error> {
    log::info!("Removing {} paths from ignore_list", paths.len());
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
    let _ =
        diesel::delete(ignore_list::table.filter(ignore_list::path.eq(path.clone()))).execute(conn);
    diesel::insert_into(allow_list::table)
        .values(AllowList { path, is_folder })
        .execute(conn)
}

pub fn get_all_allowed_paths(conn: &mut SqliteConnection) -> Vec<AllowList> {
    match allow_list::table
        .select((allow_list::path, allow_list::is_folder))
        .load::<AllowList>(conn)
    {
        Ok(paths) => paths,
        Err(e) => {
            log::error!("Could not load allowed paths: {}", e);
            Vec::new()
        }
    }
}

pub fn clear_last_parsed_dates_from_db(conn: &mut SqliteConnection) {
    if let Err(e) = diesel::update(document::table)
        .set(document::last_parsed.eq(0))
        .execute(conn)
    {
        log::error!("Could not reset last_parsed on documents: {}", e);
    }
    if let Err(e) = diesel::update(body::table)
        .set(body::last_parsed.eq(0))
        .execute(conn)
    {
        log::error!("Could not reset last_parsed on body: {}", e);
    }
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
        let long_word = "w".repeat(5000);
        let text = format!("abc {} def", long_word);
        let chunks = chunk_text(text);
        assert!(chunks.iter().any(|chunk| chunk == &long_word));
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