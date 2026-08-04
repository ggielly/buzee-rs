use crate::arc_read::search_arc;
use crate::chrome_read::search_chrome;
use crate::firefox_read::search_firefox;
use crate::custom_types::{Error, DBStat, DateLimit, QuerySegments};
use crate::database::establish_connection;
use crate::database::models::{DocumentSearchResult, MetadataFTSSearchResult};
use crate::indexing::all_allowed_filetypes;
use crate::tantivy_index::{acquire_searcher_from_reader, create_tantivy_schema, get_tantivy_index, parse_query_and_get_top_docs, return_document_search_results};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};
use diesel::r2d2::{PooledConnection, ConnectionManager};
use serde_json;
use super::schema::{body, document};
use tantivy::{Searcher, Index};

/// Sanitize a string for safe interpolation into FTS5 MATCH queries.
/// Strips all FTS5 operator characters and escapes single quotes.
fn sanitize_fts_input(input: &str) -> String {
    // Strip FTS5 special characters that could alter query semantics
    let cleaned: String = input
        .chars()
        .filter(|c| !matches!(c, '\'' | '"' | '(' | ')' | ':' | '{' | '}' | '^' | '+' | '-' | '*' | '~' | '\\'))
        .collect();
    cleaned.replace('\'', "''")
}

/// Validate that a file_type value is an alphanumeric extension (plus underscore/hyphen).
/// Returns None if the value contains suspicious characters.
fn validate_file_type(ft: &str) -> Option<String> {
    let trimmed = ft.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Only allow alphanumeric, underscore, hyphen — no quotes, no parens, no semicolons
    if trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        Some(trimmed.to_string())
    } else {
        log::warn!("Rejected suspicious file_type value: {:?}", trimmed);
        None
    }
}

fn parse_stringified_query_segments(json_string: &str) -> QuerySegments {
    let parsed_json = serde_json::from_str(json_string);
    // convert to QuerySegments
    let query_segments: QuerySegments = match parsed_json {
        Ok(value) => value,
        Err(_) => QuerySegments {
            quoted_segments: Vec::new(),
            greedy_segments: Vec::new(),
            not_segments: Vec::new(),
        },
    };
    query_segments
}

fn create_tantivy_query_statement(query_segments: &QuerySegments, file_type_string: String) -> String {
    let mut tantivy_query_string: String = String::new();

    // If there are quoted segments, join them with double quotes
    if query_segments.quoted_segments.len() > 0 {
      tantivy_query_string = format!("{}",
        query_segments.quoted_segments
            .iter()
            .map(|segment| {
                let clean = segment.replace("^^", "");
                // Strip characters that could break Tantivy query syntax
                let safe: String = clean.chars()
                    .filter(|c| !matches!(c, '\'' | '(' | ')' | ':' | '{' | '}' | '^' | '+' | '~' | '\\'))
                    .collect();
                format!("\"{}\"", safe)
            })
            .collect::<Vec<String>>()
            .join(" ")
        );
    }
    // If there are greedy segments, join them with an asterisk space
    if query_segments.greedy_segments.len() > 0 {
      tantivy_query_string = format!(
            "{} {}*",
            tantivy_query_string,
            query_segments.greedy_segments.iter().map(|s| {
                let safe: String = s.chars()
                    .filter(|c| !matches!(c, '\'' | '"' | '(' | ')' | ':' | '{' | '}' | '^' | '+' | '-' | '~' | '\\'))
                    .collect();
                safe
            }).collect::<Vec<String>>().join("* ")
        );
    }
    // If there are NOT segments, place a - in front of each segment
    if query_segments.not_segments.len() > 0 {
      tantivy_query_string = format!(
          "{} -{}",
          tantivy_query_string,
          query_segments.not_segments.iter().map(|segment| {
              let clean = segment.replace("^^", "");
              let safe: String = clean.chars()
                  .filter(|c| !matches!(c, '\'' | '"' | '(' | ')' | ':' | '{' | '}' | '^' | '+' | '~' | '\\'))
                  .collect();
              if safe.contains(' ') {
                  format!("\"{}\"", safe)
              } else {
                  safe
              }
          })
          .collect::<Vec<String>>()
          .join(" -")
        );
    }
    // remove the trailing `OR )`
    tantivy_query_string = tantivy_query_string.trim().to_string();

    if file_type_string.is_empty() {
        return tantivy_query_string;
    }
    if !tantivy_query_string.is_empty() {
      // add file_type to query — validate each type
      let safe_types: Vec<String> = file_type_string
          .split(',')
          .filter_map(|t| validate_file_type(t))
          .collect();
      if !safe_types.is_empty() {
        if safe_types.len() == 1 {
          tantivy_query_string = format!("{} AND file_type:{}", tantivy_query_string, safe_types[0]);
        } else {
          let ft_query = safe_types.iter().map(|t| format!("file_type:{}", t)).collect::<Vec<_>>().join(" OR ");
          tantivy_query_string = format!("{} AND ({})", tantivy_query_string, ft_query);
        }
      }
    }

    tantivy_query_string
}

// Return documents from the metadata_fts index that match the given search query (name and type)
// bm25(document_fts, 10) is the ranking function which gives 10x weight to the file name (first column)
pub fn search_fts_index(
    query: String,
    page: i32,
    limit: i32,
    file_type: Option<String>,
    date_limit: Option<DateLimit>,
    conn: PooledConnection<ConnectionManager<SqliteConnection>>,
    app: &tauri::AppHandle
) -> Result<Vec<DocumentSearchResult>, diesel::result::Error> {
    log::debug!(
        "search_fts_index: query: {}, page: {}, limit: {}, file_type: {:?}, date_limit: {:?}",
        query, page, limit, file_type, date_limit
    );

    let query_segments: QuerySegments = parse_stringified_query_segments(&query);
    log::debug!("query_segments: {:?}", query_segments);

    let mut search_results: Vec<DocumentSearchResult>;
    // if there is only a NOT query, pass it to `handle_special_case` function
    if query_segments.quoted_segments.is_empty() && query_segments.greedy_segments.is_empty() && !query_segments.not_segments.is_empty() {
      search_results = handle_special_case(query, page, limit, file_type, conn)?;
    }
    // otherwise run the Tantivy search query
    else {
      let tantivy_string = create_tantivy_query_statement(&query_segments, file_type.unwrap_or("".to_string()));
      log::debug!("tantivy_string: {}", tantivy_string);

      match get_tantivy_index(create_tantivy_schema()) {
        Ok(tantivy_index) => {
          match acquire_searcher_from_reader(&app) {
            Ok(searcher) => {
              let new_conn = establish_connection(&app);
              search_results = get_search_results_from_tantivy_index(&tantivy_string, limit, page, &searcher, &tantivy_index, new_conn).unwrap_or(Vec::new());
            }
            Err(e) => {
              log::error!("Failed to acquire tantivy searcher: {}", e);
              search_results = Vec::new();
            }
          }
        }
        Err(e) => {
          log::error!("Failed to open tantivy index: {}", e);
          search_results = Vec::new();
        }
      }
    }
    // and order them by last_modified
    // TODO: change this to frecency rank
    search_results.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    // remove duplicates by checking if the id is the same
    search_results.dedup_by(|a, b| a.id == b.id);
    if let Some(date_limit) = date_limit {
      let start_date = date_limit.start.parse::<i64>().unwrap_or(0);
      let end_date = date_limit.end.parse::<i64>().unwrap_or(0);
      if start_date > 0 || end_date > 0 {
        // remove results that don't match the date limit
        search_results.retain(|result| {
          result.last_modified >= start_date && result.last_modified <= end_date
        });
      }
    }

    Ok(search_results)
}

fn get_search_results_from_tantivy_index(query: &String, limit: i32, page: i32, searcher: &Searcher, tantivy_index: &Index, mut conn:  PooledConnection<ConnectionManager<SqliteConnection>>,) -> Result<Vec<DocumentSearchResult>, Error> {
  let top_docs = parse_query_and_get_top_docs(&tantivy_index, &searcher, query.to_string(), limit, page*limit).unwrap_or(Vec::new());
  if top_docs.len() > 0 {
    let search_results = return_document_search_results(&tantivy_index, &searcher, top_docs).unwrap_or(vec![]);
    let document_ids: Vec<i32> = search_results.iter().map(|result| result.id as i32).collect();

    let search_results_to_return = document::table
      .filter(document::id.eq_any(document_ids))
      .load::<DocumentSearchResult>(&mut conn)
      .unwrap_or(Vec::new());

    Ok(search_results_to_return)
  } else {
    Ok(Vec::new())
  }
}

// Get recently opened documents
pub fn get_recently_opened_docs(
    page: i32,
    limit: i32,
    file_type: Option<String>,
    mut conn: PooledConnection<ConnectionManager<SqliteConnection>>,
) -> Result<Vec<DocumentSearchResult>, diesel::result::Error> {
    // Add file type(s) — validate each type against a whitelist pattern
    let where_file_type = if let Some(file_type) = file_type {
        let safe_types: Vec<String> = file_type
            .split(',')
            .filter_map(|t| validate_file_type(t))
            .collect();
        if safe_types.is_empty() {
            "".to_string()
        } else {
            format!(
                r#" WHERE file_type IN ('{}')"#,
                safe_types.join("','")
            )
        }
    } else {
        "".to_string()
    };

    let inner_query = format!(
        r#"
        SELECT m.source_domain, m.source_id as id, m.title as name, m.url as path, m.created_at, m.last_modified, m.frecency_rank, m.frecency_last_accessed, d.file_type, d.size, d.is_pinned, d.comment, d.last_opened, d.last_synced, d.last_parsed
        FROM metadata m
        JOIN (
            SELECT id, file_type, size, is_pinned, comment, last_opened, last_synced, last_parsed
            FROM document
            {where_file_type}
        ) d ON m.source_id = d.id
        ORDER BY last_modified DESC
        LIMIT {limit} OFFSET {offset}
    "#,
        where_file_type = if !where_file_type.is_empty() {
            where_file_type
        } else {
            "".to_string()
        },
        limit = limit,
        offset = page * limit
    );
    log::debug!("inner_query: {}", inner_query);
    let search_results = diesel::sql_query(inner_query).load::<DocumentSearchResult>(&mut conn)?;

    if search_results.len() > 0 {
        log::debug!("search_results: {:?}", search_results[0]);
    }
    Ok(search_results)
}

// Get the counts for all file_types from the document table
pub fn get_counts_for_all_filetypes(
    mut conn: PooledConnection<ConnectionManager<SqliteConnection>>,
) -> Result<Vec<DBStat>, diesel::result::Error> {
    #[derive(diesel::QueryableByName)]
    struct FileTypeCount {
        #[diesel(sql_type = diesel::sql_types::Text)]
        file_type: String,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    // A single GROUP BY query replaces the previous per-filetype COUNT loop.
    let counts: Vec<FileTypeCount> = diesel::sql_query(
        "SELECT file_type, COUNT(*) AS count FROM document GROUP BY file_type",
    )
    .load(&mut conn)?;

    let counts_by_type: std::collections::HashMap<String, i64> = counts
        .into_iter()
        .map(|row| (row.file_type, row.count))
        .collect();

    let all_filetypes = all_allowed_filetypes(&mut conn, true);
    let mut stats: Vec<DBStat> = Vec::with_capacity(all_filetypes.len());
    for doctype in all_filetypes {
        stats.push(DBStat {
            file_type: doctype.file_type.clone(),
            count: counts_by_type.get(&doctype.file_type).copied().unwrap_or(0),
        });
    }
    Ok(stats)
}

// Get total number of documents in the database (all indexed files).
pub fn get_total_document_count(
    conn: &mut SqliteConnection,
) -> Result<i64, diesel::result::Error> {
    use crate::database::schema::document::dsl::*;
    document.count().get_result(conn)
}

// Get counts for total files and num files parsed
pub fn get_file_parsed_count(mut conn: PooledConnection<ConnectionManager<SqliteConnection>>) -> Result<i64, diesel::result::Error> {
    use crate::database::schema::document::dsl::*;
    let parsed_files = document
        .filter(last_parsed.gt(0))
        .filter(file_type.ne("folder"))
        .count().get_result(&mut conn)?;
    Ok(parsed_files)
}

// Handle special case with NEGATIVE query only
// Get recently opened docs (which is what the user was seeing when they typed the query)
// Then filter out the results that match the negative query/queries
fn handle_special_case(
    query: String,
    page: i32,
    limit: i32,
    file_type: Option<String>,
    conn: PooledConnection<ConnectionManager<SqliteConnection>>,
) -> Result<Vec<DocumentSearchResult>, diesel::result::Error> {
    let query_segments: QuerySegments = parse_stringified_query_segments(&query);
    log::debug!("query_segments: {:?}", query_segments);
    let outer_search_results = get_recently_opened_docs(page, limit*2, file_type, conn)?;
    let mut search_results: Vec<DocumentSearchResult> = Vec::new();
    // iterate over outer_search_results and remove any item where item.name or item.path contains any of query_segments.not_segments
    for result in outer_search_results {
        let mut found = false;
        for not_segment in &query_segments.not_segments {
            if result.name.contains(not_segment) || result.path.contains(not_segment) || result.file_type.contains(not_segment) {
                found = true;
                break;
            }
        }
        if !found {
            search_results.push(result);
        }
    }
    Ok(search_results)
}

// Get search suggestions
pub fn get_metadata_title_matches(
    query: String,
    conn: &mut SqliteConnection,
) -> Result<Vec<String>, diesel::result::Error> {
    log::debug!("getting suggestions for: {}!", query);
    // Sanitize the query to prevent FTS injection — strip all FTS5 operators
    let safe_query = sanitize_fts_input(&query);
    if safe_query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let inner_query = format!(
        r#"
            SELECT snippet(metadata_fts, 4, '', '', '', 2) as title from metadata_fts WHERE metadata_fts MATCH '{}*' ORDER BY rank LIMIT 10;
        "#,
        safe_query
    );
    let keyword_suggestions: Vec<MetadataFTSSearchResult> = diesel::sql_query(inner_query).load::<MetadataFTSSearchResult>(conn)?;
    let mut suggestions: Vec<String> = keyword_suggestions.iter().map(|suggestion| suggestion.title.clone()).collect();
    
    // convert keywords to lowercase
    suggestions = suggestions.iter().map(|s| s.trim().to_lowercase()).collect();
    // iterate over the suggestions and remove any item that does not contain the query
    suggestions.retain(|suggestion| suggestion.contains(&query));
    // iterate over the suggestions and remove any item that contains any character other than alphanumeric, _, - and space
    suggestions.retain(|suggestion| suggestion.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' '));

    // let file_suggestions = metadata::table
    //     .select(metadata::title)
    //     .filter(metadata::title.like(format!("%{}%", query)))
    //     .order(metadata::last_modified.desc())
    //     .limit(7)
    //     .load::<String>(conn)?;

    // for suggestion in &file_suggestions {
    //     suggestions.push(suggestion.clone());
    // }

    // remove duplicates
    suggestions.dedup();

    Ok(suggestions)
}

// Get parsed text for file
pub fn get_parsed_text_for_file(document_id: i32, conn: &mut SqliteConnection) -> Result<Vec<String>, diesel::result::Error> {
  // get all items from body where url is equal to file_path
  let parsed_text_rows = body::table
    .filter(body::source_id.eq(document_id))
    .select(body::text)
    .order_by(body::id.asc())
    .load::<String>(conn)?;

  Ok(parsed_text_rows)
}

// Get the file_id from the document table in the database
pub fn get_file_id_from_path(file_path: &String, conn: &mut SqliteConnection) -> Result<i32, diesel::result::Error> {
  use crate::database::schema::document::dsl::*;
  let file_id = document
    .select(id)
    .filter(path.eq(file_path))
    .first::<i32>(conn);

  if file_id.is_ok() {
    return Ok(file_id.unwrap());
  } else {
    return Ok(0);
  }
}

// Get search results from Firefox and Chrome history
pub fn search_browser_history(user_profile: String, user_query: String, limit: i32, page: i32) -> Result<Vec<DocumentSearchResult>, Error> {
  let chrome_search_results = search_chrome(user_profile.clone(), user_query.clone(), i64::from(limit), i64::from(page)).unwrap_or(vec![]);
  let firefox_search_results = search_firefox(user_query.clone(), i64::from(limit), i64::from(page)).unwrap_or(vec![]);
  let arc_search_results = search_arc(user_profile, user_query, i64::from(limit), i64::from(page)).unwrap_or(vec![]);
  
  log::debug!("got {} results from chrome and {} results from firefox and {} results from arc", chrome_search_results.len(), firefox_search_results.len(), arc_search_results.len());
  let mut search_results: Vec<DocumentSearchResult> = chrome_search_results.into_iter().chain(firefox_search_results.into_iter()).collect();
  search_results = search_results.into_iter().chain(arc_search_results.into_iter()).collect();

  // sort the search results by last_opened descending
  search_results.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));

  Ok(search_results)
}