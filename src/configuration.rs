use std::convert::{TryFrom, TryInto};
use config::{Config, File};


#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

// Added to help handle environment in which app is run (docker vs local)
#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: String,
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password,
            self.host,
            self.port,
            self.database_name
        )
    }

    pub fn connection_string_without_db(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}",
            self.username, self.password, self.host, self.port
        )
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("configuration");

    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");
    
    let builder = Config::builder()
        // set default values
        .set_default("application.port", 8000)?
        .set_default("application.host", "127.0.0.1")?
        // Add sources
        .add_source(File::from(configuration_directory.join("base")).required(true))
        .add_source(File::from(configuration_directory.join(environment.as_str())).required(true))
        .add_source(config::Environment::with_prefix("APP").separator("__"));

    let config = builder.build()?;
    config.try_deserialize::<Settings>()
        .map_err(|e| {
            eprintln!("Failed to read config: {}", e);
            e
        })


    }

    // The possible runtime environment for our application.
    pub enum Environment {
        Local,
        Production,
    }

    impl Environment {
        pub fn as_str(&self) -> &'static str {
            match self {
                Environment::Local => "local",
                Environment::Production => "production",
            }
        }
    }

    impl TryFrom<String> for Environment {
        type Error = String;

        fn try_from(s: String) -> Result<Self, Self::Error> {
            match s.to_lowercase().as_str() {
                "local" => Ok(Self::Local),
                "production" => Ok(Self::Production),
                other => Err(format!(
                    "{} is not a supported environment. Use either `local` or `production`.",
                    other
                )),
            }
        }
    }
