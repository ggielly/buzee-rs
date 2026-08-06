use super::schema::{body, document};
use crate::domain::types::{DBStat, DateLimit, QuerySegments};
use crate::domain::AppError;
use crate::infrastructure::browser_readers::{search_arc, search_chrome, search_firefox};
use crate::infrastructure::database::models::{DocumentSearchResult, MetadataFTSSearchResult};
use crate::infrastructure::database::establish_connection;
use crate::infrastructure::indexing::all_allowed_filetypes;
use crate::infrastructure::tantivy_index::{
    acquire_searcher_from_reader, parse_query_and_get_top_docs, return_document_search_results,
};
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};
use serde_json;
use tantivy::{Index, IndexReader, Searcher};

fn sanitize_fts_input(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\'' | '"' | '(' | ')' | ':' | '{' | '}' | '^' | '+' | '-' | '*' | '~' | '\\'
            )
        })
        .collect();
    cleaned.replace('\'', "''")
}

fn validate_file_type(ft: &str) -> Option<String> {
    let trimmed = ft.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        Some(trimmed.to_string())
    } else {
        log::warn!("Rejected suspicious file_type value: {:?}", trimmed);
        None
    }
}

fn parse_stringified_query_segments(json_string: &str) -> QuerySegments {
    let parsed_json = serde_json::from_str(json_string);
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

fn create_tantivy_query_statement(
    query_segments: &QuerySegments,
    file_type_string: String,
) -> String {
    let mut tantivy_query_string: String = String::new();

    if query_segments.quoted_segments.len() > 0 {
        tantivy_query_string = format!(
            "{}",
            query_segments
                .quoted_segments
                .iter()
                .map(|segment| {
                    let clean = segment.replace("^^", "");
                    let safe: String = clean
                        .chars()
                        .filter(|c| {
                            !matches!(
                                c,
                                '\'' | '(' | ')' | ':' | '{' | '}' | '^' | '+' | '~' | '\\'
                            )
                        })
                        .collect();
                    format!("\"{}\"", safe)
                })
                .collect::<Vec<String>>()
                .join(" ")
        );
    }
    if query_segments.greedy_segments.len() > 0 {
        tantivy_query_string = format!(
            "{} {}*",
            tantivy_query_string,
            query_segments
                .greedy_segments
                .iter()
                .map(|s| {
                    let safe: String = s
                        .chars()
                        .filter(|c| {
                            !matches!(
                                c,
                                '\'' | '"'
                                    | '('
                                    | ')'
                                    | ':'
                                    | '{'
                                    | '}'
                                    | '^'
                                    | '+'
                                    | '-'
                                    | '~'
                                    | '\\'
                            )
                        })
                        .collect();
                    safe
                })
                .collect::<Vec<String>>()
                .join("* ")
        );
    }
    if query_segments.not_segments.len() > 0 {
        tantivy_query_string = format!(
            "{} -{}",
            tantivy_query_string,
            query_segments
                .not_segments
                .iter()
                .map(|segment| {
                    let clean = segment.replace("^^", "");
                    let safe: String = clean
                        .chars()
                        .filter(|c| {
                            !matches!(
                                c,
                                '\'' | '"' | '(' | ')' | ':' | '{' | '}' | '^' | '+' | '~' | '\\'
                            )
                        })
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
    tantivy_query_string = tantivy_query_string.trim().to_string();

    if file_type_string.is_empty() {
        return tantivy_query_string;
    }
    if !tantivy_query_string.is_empty() {
        let safe_types: Vec<String> = file_type_string
            .split(',')
            .filter_map(|t| validate_file_type(t))
            .collect();
        if !safe_types.is_empty() {
            if safe_types.len() == 1 {
                tantivy_query_string =
                    format!("{} AND file_type:{}", tantivy_query_string, safe_types[0]);
            } else {
                let ft_query = safe_types
                    .iter()
                    .map(|t| format!("file_type:{}", t))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                tantivy_query_string = format!("{} AND ({})", tantivy_query_string, ft_query);
            }
        }
    }

    tantivy_query_string
}

pub fn search_fts_index(
    query: String,
    page: i32,
    limit: i32,
    file_type: Option<String>,
    date_limit: Option<DateLimit>,
    conn: PooledConnection<ConnectionManager<SqliteConnection>>,
    tantivy_reader: &IndexReader,
    tantivy_index: &Index,
) -> Result<Vec<DocumentSearchResult>, diesel::result::Error> {
    log::debug!(
        "search_fts_index: query: {}, page: {}, limit: {}, file_type: {:?}, date_limit: {:?}",
        query,
        page,
        limit,
        file_type,
        date_limit
    );

    let query_segments: QuerySegments = parse_stringified_query_segments(&query);
    log::debug!("query_segments: {:?}", query_segments);

    let mut search_results: Vec<DocumentSearchResult>;
    if query_segments.quoted_segments.is_empty()
        && query_segments.greedy_segments.is_empty()
        && !query_segments.not_segments.is_empty()
    {
        search_results = handle_special_case(query, page, limit, file_type, conn)?;
    } else {
        let tantivy_string =
            create_tantivy_query_statement(&query_segments, file_type.unwrap_or("".to_string()));
        log::debug!("tantivy_string: {}", tantivy_string);

        let searcher = acquire_searcher_from_reader(tantivy_reader);
        search_results = get_search_results_from_tantivy_index(
            &tantivy_string,
            limit,
            page,
            &searcher,
            tantivy_index,
            conn,
        )
        .unwrap_or(Vec::new());
    }
    search_results.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    search_results.dedup_by(|a, b| a.id == b.id);
    if let Some(date_limit) = date_limit {
        let start_date = date_limit.start.parse::<i64>().unwrap_or(0);
        let end_date = date_limit.end.parse::<i64>().unwrap_or(0);
        if start_date > 0 || end_date > 0 {
            search_results.retain(|result| {
                result.last_modified >= start_date && result.last_modified <= end_date
            });
        }
    }

    Ok(search_results)
}

fn get_search_results_from_tantivy_index(
    query: &String,
    limit: i32,
    page: i32,
    searcher: &Searcher,
    tantivy_index: &Index,
    mut conn: PooledConnection<ConnectionManager<SqliteConnection>>,
) -> Result<Vec<DocumentSearchResult>, AppError> {
    let top_docs = parse_query_and_get_top_docs(
        tantivy_index,
        &searcher,
        query.to_string(),
        limit,
        page * limit,
    )
    .unwrap_or(Vec::new());
    if top_docs.len() > 0 {
        let search_results =
            return_document_search_results(tantivy_index, &searcher, top_docs).unwrap_or(vec![]);
        let document_ids: Vec<i32> = search_results
            .iter()
            .map(|result| result.id as i32)
            .collect();

        let search_results_to_return = document::table
            .filter(document::id.eq_any(document_ids))
            .load::<DocumentSearchResult>(&mut conn)
            .unwrap_or(Vec::new());

        Ok(search_results_to_return)
    } else {
        Ok(Vec::new())
    }
}

pub fn get_recently_opened_docs(
    page: i32,
    limit: i32,
    file_type: Option<String>,
    mut conn: PooledConnection<ConnectionManager<SqliteConnection>>,
) -> Result<Vec<DocumentSearchResult>, diesel::result::Error> {
    let where_file_type = if let Some(file_type) = file_type {
        let safe_types: Vec<String> = file_type
            .split(',')
            .filter_map(|t| validate_file_type(t))
            .collect();
        if safe_types.is_empty() {
            "".to_string()
        } else {
            format!(r#" WHERE file_type IN ('{}')"#, safe_types.join("','"))
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

    let counts: Vec<FileTypeCount> =
        diesel::sql_query("SELECT file_type, COUNT(*) AS count FROM document GROUP BY file_type")
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

pub fn get_total_document_count(conn: &mut SqliteConnection) -> Result<i64, diesel::result::Error> {
    use crate::infrastructure::database::schema::document::dsl::*;
    document.count().get_result(conn)
}

pub fn get_file_parsed_count(
    mut conn: PooledConnection<ConnectionManager<SqliteConnection>>,
) -> Result<i64, diesel::result::Error> {
    use crate::infrastructure::database::schema::document::dsl::*;
    let parsed_files = document
        .filter(last_parsed.gt(0))
        .filter(file_type.ne("folder"))
        .count()
        .get_result(&mut conn)?;
    Ok(parsed_files)
}

fn handle_special_case(
    query: String,
    page: i32,
    limit: i32,
    file_type: Option<String>,
    conn: PooledConnection<ConnectionManager<SqliteConnection>>,
) -> Result<Vec<DocumentSearchResult>, diesel::result::Error> {
    let query_segments: QuerySegments = parse_stringified_query_segments(&query);
    log::debug!("query_segments: {:?}", query_segments);
    let outer_search_results = get_recently_opened_docs(page, limit * 2, file_type, conn)?;
    let mut search_results: Vec<DocumentSearchResult> = Vec::new();
    for result in outer_search_results {
        let mut found = false;
        for not_segment in &query_segments.not_segments {
            if result.name.contains(not_segment)
                || result.path.contains(not_segment)
                || result.file_type.contains(not_segment)
            {
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

pub fn get_metadata_title_matches(
    query: String,
    conn: &mut SqliteConnection,
) -> Result<Vec<String>, diesel::result::Error> {
    log::debug!("getting suggestions for: {}!", query);
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
    let keyword_suggestions: Vec<MetadataFTSSearchResult> =
        diesel::sql_query(inner_query).load::<MetadataFTSSearchResult>(conn)?;
    let mut suggestions: Vec<String> = keyword_suggestions
        .iter()
        .map(|suggestion| suggestion.title.clone())
        .collect();

    suggestions = suggestions
        .iter()
        .map(|s| s.trim().to_lowercase())
        .collect();
    suggestions.retain(|suggestion| suggestion.contains(&query));
    suggestions.retain(|suggestion| {
        suggestion
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ')
    });

    suggestions.dedup();

    Ok(suggestions)
}

pub fn get_parsed_text_for_file(
    document_id: i32,
    conn: &mut SqliteConnection,
) -> Result<Vec<String>, diesel::result::Error> {
    let parsed_text_rows = body::table
        .filter(body::source_id.eq(document_id))
        .select(body::text)
        .order_by(body::id.asc())
        .load::<String>(conn)?;

    Ok(parsed_text_rows)
}

pub fn get_file_id_from_path(
    file_path: &String,
    conn: &mut SqliteConnection,
) -> Result<i32, diesel::result::Error> {
    use crate::infrastructure::database::schema::document::dsl::*;
    let file_id = document
        .select(id)
        .filter(path.eq(file_path))
        .first::<i32>(conn);

    if file_id.is_ok() {
        return file_id;
    } else {
        return Ok(0);
    }
}

pub fn search_browser_history(
    user_profile: String,
    user_query: String,
    limit: i32,
    page: i32,
) -> Result<Vec<DocumentSearchResult>, AppError> {
    let chrome_search_results = search_chrome(
        user_profile.clone(),
        user_query.clone(),
        i64::from(limit),
        i64::from(page),
    )
    .unwrap_or(vec![]);
    let firefox_search_results =
        search_firefox(user_query.clone(), i64::from(limit), i64::from(page)).unwrap_or(vec![]);
    let arc_search_results =
        search_arc(user_profile, user_query, i64::from(limit), i64::from(page)).unwrap_or(vec![]);

    log::debug!(
        "got {} results from chrome and {} results from firefox and {} results from arc",
        chrome_search_results.len(),
        firefox_search_results.len(),
        arc_search_results.len()
    );
    let mut search_results: Vec<DocumentSearchResult> = chrome_search_results
        .into_iter()
        .chain(firefox_search_results.into_iter())
        .collect();
    search_results = search_results
        .into_iter()
        .chain(arc_search_results.into_iter())
        .collect();

    search_results.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));

    Ok(search_results)
}