use crate::infrastructure::database::{create_tables_if_not_exists, establish_direct_connection_to_db};
use crate::infrastructure::user_prefs::{
    get_enable_logs_flag, set_default_app_data, set_default_file_types, set_default_user_prefs,
};
use crate::infrastructure::utils::norm;
use diesel::RunQueryDsl;
use dirs::document_dir;
use log::info;
use log::LevelFilter;

pub const APP_DIRECTORY: &str = r#"buzee-tauri"#;

pub fn get_documents_directory() -> Option<String> {
    if let Some(documents_dir) = document_dir() {
        Some(documents_dir.to_string_lossy().to_string())
    } else {
        None
    }
}

pub fn get_home_directory() -> Option<String> {
    if let Some(home_dir) = dirs::home_dir() {
        Some(home_dir.to_string_lossy().to_string())
    } else {
        None
    }
}

pub fn create_app_directory_if_not_exists() -> Result<(), std::io::Error> {
    let documents_dir = get_documents_directory().unwrap();
    let app_dir_path = format!("{}/{}", documents_dir, APP_DIRECTORY);
    let app_dir_path = norm(&app_dir_path);
    log::info!("creating app dir at:{}", &app_dir_path);
    std::fs::create_dir_all(app_dir_path)
}

pub fn get_app_directory() -> String {
    let app_dir_path = format!("{}/{}", get_documents_directory().unwrap(), APP_DIRECTORY);
    let app_dir_path = norm(&app_dir_path);
    app_dir_path
}

pub fn create_tantivy_index_directory_if_not_exists() -> Result<(), std::io::Error> {
    let app_dir_path = get_app_directory();
    let index_dir_path = format!("{}/{}", app_dir_path, "buzee_tantivy_index");
    let index_dir_path = norm(&index_dir_path);
    log::info!("creating tantivy index dir at:{}", &index_dir_path);
    std::fs::create_dir_all(index_dir_path)
}

pub fn get_log_file_path() -> String {
    let app_dir_path = get_app_directory();
    let logging_file_path = format!("{}/{}", app_dir_path, "buzee.log");
    norm(&logging_file_path)
}

pub fn setup_file_logging(enabled: bool) {
    use log4rs::append::file::FileAppender;
    use log4rs::config::{Appender, Root};
    use log4rs::encode::pattern::PatternEncoder;

    if let Err(err) = std::fs::create_dir_all(get_app_directory()) {
        eprintln!("failed to create app directory for logging: {}", err);
    }

    let appender = FileAppender::builder()
        .append(true)
        .encoder(Box::new(PatternEncoder::new("{d} {l} {t} - {m}{n}")))
        .build(get_log_file_path())
        .expect("could not create log appender");

    let level = if enabled {
        LevelFilter::Info
    } else {
        LevelFilter::Off
    };

    let config = log4rs::Config::builder()
        .appender(Appender::builder().build("file", Box::new(appender)))
        .build(Root::builder().appender("file").build(level))
        .expect("could not build log configuration");

    let _ = log4rs::init_config(config);
    info!("Logger enabled; writing to {}", get_log_file_path());
}

pub fn initialize() -> () {
    log::info!("Initializing app directory");
    create_app_directory_if_not_exists().unwrap();
    create_tantivy_index_directory_if_not_exists().unwrap();

    let mut conn = establish_direct_connection_to_db()
        .expect("Could not create a direct DB connection at startup");
    log::info!("Initializing database");
    create_tables_if_not_exists(&mut conn).unwrap();

    diesel::sql_query("PRAGMA foreign_keys = ON;")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("PRAGMA busy_timeout = 5000;")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("PRAGMA journal_mode = WAL;")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("PRAGMA synchronous = NORMAL;")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("PRAGMA cache_size = -64000;")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("PRAGMA auto_vacuum = FULL;")
        .execute(&mut conn)
        .unwrap();

    set_default_app_data(&mut conn);
    set_default_user_prefs(&mut conn, false);
    set_default_file_types(&mut conn);

    let enable_logs = get_enable_logs_flag(&mut conn);
    setup_file_logging(enable_logs);
    info!("Logger initialized");

    #[cfg(all(target_os = "windows", feature = "ocr"))]
    {
        use crate::infrastructure::text_extraction::ocr_cache;
        ocr_cache::prune_ocr_caches(&mut conn, 90);
    }
}