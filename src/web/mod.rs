pub mod routes;

use std::sync::Arc;

use crate::{database::Database, providers::ProviderRegistry, secrets::SecretResolver};

pub struct AppState {
    pub database: Database,
    pub secret_resolver: Arc<dyn SecretResolver>,
    pub providers: ProviderRegistry,
}
