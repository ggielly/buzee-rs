pub mod browser_readers;
pub mod dashboard;
pub mod database;
pub mod housekeeping;
pub mod indexing;
pub mod platform;
pub mod statistics;
pub mod tantivy_index;
pub mod text_extraction;
pub mod user_prefs;
pub mod utils;

pub use database::{
    establish_connection, establish_connection_without_app, establish_direct_connection_to_db,
    get_connection_pool,
};
pub use housekeeping::{get_app_directory, initialize as housekeeping_initialize};
pub use platform::{DefaultPlatformService, PlatformService};