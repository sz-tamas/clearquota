mod gcp;

use async_trait::async_trait;
use secrecy::SecretString;

pub use gcp::{
    GcpSecretManagerResolver, begin_authentication, check_application_default_credentials,
};

#[async_trait]
pub trait SecretResolver: Send + Sync {
    /// Resolves a credential only for the duration of a collection request.
    async fn resolve(&self, reference: &str) -> Result<SecretString, SecretError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret reference must use projects/<project>/secrets/<secret>/versions/<version>")]
    InvalidReference,
    #[error("Application Default Credentials could not produce an access token")]
    AuthenticationFailed,
    #[error("Google Secret Manager could not access this secret reference")]
    AccessFailed,
    #[error("Google Secret Manager returned an invalid secret payload")]
    InvalidPayload,
}
