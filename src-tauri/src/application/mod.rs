pub mod app_services;
pub mod commands;
pub mod events;
pub mod requests;
pub mod sync_ops;
pub mod workers;

pub use app_services::{AppServices, BuildServices, OcrController, SyncController};
pub use events::UiEvent;
pub use requests::WorkerRequest;
use std::sync::Arc;

/// Assemble the application from raw resources, start the worker and return the
/// two handles the UI needs (services + request sender). The UI owns the event
/// receiver created from the same channel passed into `app_services::build`.
pub fn launch(
    services: Arc<AppServices>,
    event_tx: crossbeam_channel::Sender<UiEvent>,
) -> (Arc<AppServices>, crossbeam_channel::Sender<WorkerRequest>) {
    let request_tx = workers::spawn(services.clone(), event_tx);
    (services, request_tx)
}