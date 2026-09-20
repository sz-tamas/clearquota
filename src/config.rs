use std::{env, path::PathBuf};

pub struct Config {
    pub bind_address: String,
    pub database_path: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self, std::io::Error> {
        // The host deliberately cannot be configured to a public interface. The
        // port is configurable so multiple local development services can coexist.
        let port = env::var("USAGE_DASH_PORT")
            .ok()
            .map(|value| {
                value
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port != 0)
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "USAGE_DASH_PORT must be an integer from 1 through 65535",
                        )
                    })
            })
            .transpose()?
            .unwrap_or(3000);
        let database_path = env::var("USAGE_DASH_DATABASE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/usage-dashboard.sqlite3"));
        Ok(Self {
            bind_address: format!("127.0.0.1:{port}"),
            database_path,
        })
    }
}
