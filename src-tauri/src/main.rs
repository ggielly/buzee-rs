use buzee::application::{app_services, BuildServices, UiEvent, launch};
use buzee::domain::IgnoreAllowCacheState;
use buzee::domain::types::UserPreferencesState;
use buzee::infrastructure::database;
use buzee::infrastructure::tantivy_index;
use buzee::infrastructure::user_prefs;
use buzee::infrastructure::{DefaultPlatformService, housekeeping_initialize};
use buzee::ui::{AppFlags, BuzeeApp};
use std::sync::{Arc, Mutex, RwLock};

fn main() -> iced::Result {
    housekeeping_initialize();

    let db_pool = database::get_connection_pool().expect("Failed to build the database pool");

    let preferences: Arc<RwLock<UserPreferencesState>> =
        Arc::new(RwLock::new(UserPreferencesState::default()));
    user_prefs::set_user_preferences_state_from_db_value(&preferences, &db_pool);

    let theme: buzee::ui::theme::Theme =
        serde_json::from_str(&user_prefs::get_app_theme(&db_pool)).unwrap_or_default();

    let index = tantivy_index::get_tantivy_index_cached().expect("Failed to open the Tantivy index");
    let tantivy_reader = Arc::new(tantivy_index::get_reader_for_index(&index).expect("Failed to open the index reader"));
    let tantivy_writer =
        Arc::new(Mutex::new(index.writer(50_000_000).expect("Failed to open the index writer")));
    let tantivy_index = Arc::new(index);

    let mut direct_conn = database::establish_direct_connection_to_db()
        .expect("Failed to create a direct DB connection");
    let ignore_allow_cache = Arc::new(RwLock::new(IgnoreAllowCacheState::from_db(&mut direct_conn)));

    let platform: Arc<dyn buzee::infrastructure::PlatformService> =
        Arc::new(DefaultPlatformService::new());

    let (event_tx, event_rx) = crossbeam_channel::unbounded::<UiEvent>();

    let services = app_services::build(
        BuildServices {
            db_pool,
            tantivy_reader,
            tantivy_writer,
            tantivy_index,
            preferences,
            ignore_allow_cache,
            platform,
        },
        event_tx.clone(),
    );

    let (services, request_tx) = launch(services, event_tx);

    let flags = Arc::new(Mutex::new(Some(AppFlags {
        services,
        request_tx,
        event_rx,
        theme,
    })));

    iced::application(
        move || {
            let flags = flags.lock().unwrap().take().expect("boot called more than once");
            let (app, task) = BuzeeApp::new(flags);
            (app, task)
        },
        BuzeeApp::update,
        BuzeeApp::view,
    )
    .title("Buzee")
    .subscription(BuzeeApp::subscription)
    .theme(BuzeeApp::theme)
    .scale_factor(|_| 1.0)
    .default_font(buzee::ui::fonts::BODY)
    .font(buzee::ui::fonts::BODY_BYTES)
    .font(buzee::ui::fonts::ICONS_BYTES)
    .run()
}