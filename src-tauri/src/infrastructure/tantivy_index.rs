use crate::domain::types::{
    TantivyBookmarkSearchResult, TantivyDocumentItem, TantivyDocumentSearchResult,
};
use crate::domain::AppError;
use crate::infrastructure::housekeeping::get_app_directory;
use crate::infrastructure::utils::norm;
use std::path::PathBuf;
use std::sync::Mutex;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, TantivyError};
use tantivy::{schema::*, DocAddress};

use std::sync::OnceLock;
fn cached_schema() -> &'static Schema {
    static INSTANCE: OnceLock<Schema> = OnceLock::new();
    INSTANCE.get_or_init(|| create_tantivy_schema())
}

pub fn create_tantivy_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    schema_builder.add_i64_field("id", INDEXED | STORED);
    schema_builder.add_text_field("source_table", STRING);
    schema_builder.add_text_field("source_domain", STRING);
    schema_builder.add_text_field("comment", TEXT);

    schema_builder.add_text_field("title", TEXT);
    schema_builder.add_text_field("body", TEXT);
    schema_builder.add_text_field("file_type", STRING);
    schema_builder.add_i64_field("last_modified", INDEXED | STORED);

    schema_builder.add_text_field("url", STRING);

    schema_builder.add_text_field("tags", TEXT);

    schema_builder.add_text_field("sender", STRING);
    schema_builder.add_text_field("recipient", STRING);
    schema_builder.add_text_field("cc", STRING);
    schema_builder.add_text_field("bcc", STRING);
    schema_builder.add_text_field("subject", TEXT);
    schema_builder.add_text_field("attachments", TEXT);

    schema_builder.build()
}

pub fn get_tantivy_index(schema: Schema) -> tantivy::Result<Index> {
    let index_path = PathBuf::from(norm(
        format!("{}/{}", get_app_directory(), "buzee_tantivy_index").as_str(),
    ));
    let meta_file_path = PathBuf::from(norm(
        format!(
            "{}/{}/meta.json",
            get_app_directory(),
            "buzee_tantivy_index"
        )
        .as_str(),
    ));
    if index_path.exists() && meta_file_path.exists() {
        return Index::open_in_dir(&index_path);
    } else {
        return Index::create_in_dir(index_path, schema);
    }
}

pub fn get_tantivy_index_cached() -> tantivy::Result<Index> {
    get_tantivy_index(cached_schema().clone())
}

pub fn get_reader_for_index(index: &Index) -> tantivy::Result<IndexReader> {
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;
    Ok(reader)
}

pub fn acquire_searcher_from_reader(reader: &IndexReader) -> Searcher {
    reader.searcher()
}

pub fn delete_docs_from_index_with_ids(
    writer: &Mutex<IndexWriter>,
    ids_to_delete: &Vec<i32>,
) -> tantivy::Result<()> {
    let index = get_tantivy_index_cached().unwrap();
    let mut state = writer.lock().unwrap();
    let writer = &mut *state;

    let id = index.schema().get_field("id").unwrap();
    for del_id in ids_to_delete {
        writer.delete_term(Term::from_field_i64(id, i64::from(*del_id)));
    }

    let commit_stamp = writer.commit()?;

    if commit_stamp > 0 {
        return Ok(());
    } else {
        return Err(tantivy::TantivyError::SystemError(
            "Failed to commit changes to the index".to_string(),
        ));
    }
}

pub fn delete_and_add_docs_to_index(
    writer: &Mutex<IndexWriter>,
    ids_to_delete: &Vec<i32>,
    docs_to_add: &Vec<TantivyDocumentItem>,
) -> tantivy::Result<()> {
    let index = get_tantivy_index_cached().unwrap();
    let mut state = writer.lock().unwrap();
    let writer = &mut *state;

    let id = index.schema().get_field("id").unwrap();
    for del_id in ids_to_delete {
        writer.delete_term(Term::from_field_i64(id, i64::from(*del_id)));
    }

    let source_table = index.schema().get_field("source_table").unwrap();
    let source_domain = index.schema().get_field("source_domain").unwrap();
    let title = index.schema().get_field("title").unwrap();
    let body = index.schema().get_field("body").unwrap();
    let url = index.schema().get_field("url").unwrap();
    let file_type = index.schema().get_field("file_type").unwrap();
    let last_modified = index.schema().get_field("last_modified").unwrap();
    let comment = index.schema().get_field("comment").unwrap();

    for doc in docs_to_add {
        writer.add_document(doc!(
          id => doc.source_id,
          source_table => doc.source_table.as_str(),
          source_domain => doc.source_domain.as_str(),
          title => doc.name.as_str(),
          body => doc.body.as_str(),
          url => doc.url.as_str(),
          file_type => doc.file_type.as_str(),
          last_modified => doc.last_modified,
          comment => doc.comment.as_str(),
        ))?;
    }

    let commit_stamp = writer.commit()?;
    if commit_stamp > 0 {
        Ok(())
    } else {
        Err(tantivy::TantivyError::SystemError(
            "Failed to commit changes to the index".to_string(),
        ))
    }
}

pub fn delete_all_docs_from_index(writer: &Mutex<IndexWriter>) -> tantivy::Result<()> {
    log::warn!("WARNING: Deleting all documents from the index");
    let mut state = writer.lock().unwrap();
    let writer = &mut *state;

    let _ = writer.delete_all_documents()?;

    let commit_stamp = writer.commit()?;

    if commit_stamp > 0 {
        return Ok(());
    } else {
        return Err(tantivy::TantivyError::SystemError(
            "Failed to commit changes to the index".to_string(),
        ));
    }
}

pub fn parse_query_and_get_top_docs(
    index: &Index,
    searcher: &Searcher,
    user_query: String,
    result_limit: i32,
    result_offset: i32,
) -> Result<Vec<(f32, DocAddress)>, TantivyError> {
    let comment = index.schema().get_field("comment").unwrap();
    let title = index.schema().get_field("title").unwrap();
    let body = index.schema().get_field("body").unwrap();
    let file_type = index.schema().get_field("file_type").unwrap();
    let url = index.schema().get_field("url").unwrap();
    let tags = index.schema().get_field("tags").unwrap();
    let sender = index.schema().get_field("sender").unwrap();
    let recipient = index.schema().get_field("recipient").unwrap();
    let cc = index.schema().get_field("cc").unwrap();
    let bcc = index.schema().get_field("bcc").unwrap();
    let subject = index.schema().get_field("subject").unwrap();
    let attachments = index.schema().get_field("attachments").unwrap();

    let mut query_parser = QueryParser::for_index(
        &index,
        vec![
            comment, title, body, file_type, url, tags, sender, recipient, cc, bcc, subject,
            attachments,
        ],
    );
    query_parser.set_conjunction_by_default();
    let query = query_parser.parse_query(&user_query)?;

    let limit: usize = result_limit.clamp(1, 10_000) as usize;
    let offset: usize = result_offset.max(0) as usize;
    let top_docs = searcher.search(
        &query,
        &TopDocs::with_limit(limit).and_offset(offset),
    )?;

    Ok(top_docs)
}

pub fn return_document_search_results(
    index: &Index,
    searcher: &Searcher,
    top_docs: Vec<(f32, DocAddress)>,
) -> Result<Vec<TantivyDocumentSearchResult>, TantivyError> {
    let id = index.schema().get_field("id").unwrap();
    let last_modified = index.schema().get_field("last_modified").unwrap();

    let mut search_results = Vec::new();
    for (_score, doc_address) in top_docs {
        let Ok(retrieved_doc) = searcher.doc::<TantivyDocument>(doc_address) else {
            continue;
        };
        let result: TantivyDocumentSearchResult = {
            TantivyDocumentSearchResult {
                id: retrieved_doc
                    .get_first(id)
                    .and_then(|value| value.as_i64())
                    .unwrap_or_else(|| {
                        return 0_i64;
                    }),
                last_modified: retrieved_doc
                    .get_first(last_modified)
                    .and_then(|value| value.as_i64())
                    .unwrap_or_else(|| {
                        return 0_i64;
                    }),
            }
        };
        search_results.push(result);
    }

    Ok(search_results)
}

pub fn return_bookmark_search_results(
    index: &Index,
    searcher: &Searcher,
    top_docs: Vec<(f32, DocAddress)>,
) -> Result<Vec<TantivyBookmarkSearchResult>, TantivyError> {
    let id = index.schema().get_field("id").unwrap();
    let source_table = index.schema().get_field("source_table").unwrap();
    let source_domain = index.schema().get_field("source_domain").unwrap();

    let comment = index.schema().get_field("comment").unwrap();
    let title = index.schema().get_field("title").unwrap();
    let body = index.schema().get_field("body").unwrap();
    let url = index.schema().get_field("url").unwrap();
    let tags = index.schema().get_field("tags").unwrap();

    let mut search_results = Vec::new();
    for (_score, doc_address) in top_docs {
        let Ok(retrieved_doc) = searcher.doc::<TantivyDocument>(doc_address) else {
            continue;
        };
        let result: TantivyBookmarkSearchResult = {
            TantivyBookmarkSearchResult {
                id: retrieved_doc
                    .get_first(id)
                    .and_then(|value| value.as_i64())
                    .unwrap_or_else(|| 0_i64),
                source_table: retrieved_doc
                    .get_first(source_table)
                    .and_then(|value| value.as_str())
                    .unwrap_or("null")
                    .to_string(),
                source_domain: retrieved_doc
                    .get_first(source_domain)
                    .and_then(|value| value.as_str())
                    .unwrap_or("null")
                    .to_string(),
                is_pinned: None,
                comment: retrieved_doc
                    .get_first(comment)
                    .and_then(|value| value.as_str().map(|s| s.to_string())),
                title: retrieved_doc
                    .get_first(title)
                    .and_then(|value| value.as_str().map(|s| s.to_string())),
                body: retrieved_doc
                    .get_first(body)
                    .and_then(|value| value.as_str().map(|s| s.to_string())),
                url: retrieved_doc
                    .get_first(url)
                    .and_then(|value| value.as_str().map(|s| s.to_string())),
                saved_at: None,
                last_opened: None,
                word_count: None,
                is_favorite: None,
                is_archived: None,
                is_read: None,
                tags: retrieved_doc
                    .get_first(tags)
                    .and_then(|value| value.as_str().map(|s| s.to_string())),
                frecency_rank: None,
                frecency_last_accessed: None,
            }
        };

        search_results.push(result);
    }

    Ok(search_results)
}

pub fn internal_test_create_csv_dump_from_index(searcher: &Searcher) -> crate::domain::AppError {
    let _ = searcher;
    crate::domain::AppError::new("internal_test_create_csv_dump_from_index not migrated")
}