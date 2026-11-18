use std::{collections::HashMap, error::Error, fs};

use kiln_framework::configuration::{ConfigurationError, ServerConfig, environment_from};

#[test]
fn external_variables_override_dotenv_and_server_settings_are_typed() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join(".env"), "SERVER_PORT=9000\nSERVER_HOST=localhost\n")?;
    let values = environment_from(directory.path(), HashMap::from([("SERVER_PORT".into(), "8080".into())]))?;
    let config = ServerConfig::from_environment(values)?;
    assert_eq!(config.port, 8080);
    assert_eq!(config.host, "localhost");
    Ok(())
}

#[test]
fn missing_environment_file_uses_laravel_development_server_defaults() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let values = environment_from(directory.path(), HashMap::new())?;
    let config = ServerConfig::from_environment(values)?;
    assert_eq!(config.port, 8000);
    assert_eq!(config.host, "127.0.0.1");
    Ok(())
}

#[test]
fn external_app_env_selects_an_environment_specific_file() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join(".env"), "SERVER_PORT=9000\n")?;
    fs::write(directory.path().join(".env.testing"), "SERVER_PORT=9001\n")?;
    let external = HashMap::from([("APP_ENV".into(), "testing".into())]);
    let values = environment_from(directory.path(), external)?;
    assert_eq!(ServerConfig::from_environment(values)?.port, 9001);
    Ok(())
}

#[test]
fn absent_environment_specific_file_falls_back_to_dotenv() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join(".env"), "SERVER_PORT=9000\n")?;
    let external = HashMap::from([("APP_ENV".into(), "testing".into())]);
    let values = environment_from(directory.path(), external)?;
    assert_eq!(ServerConfig::from_environment(values)?.port, 9000);
    Ok(())
}

#[test]
fn dotenv_app_env_does_not_change_the_selected_file() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join(".env"), "APP_ENV=testing\nSERVER_PORT=9000\n")?;
    fs::write(directory.path().join(".env.testing"), "SERVER_PORT=9001\n")?;
    let values = environment_from(directory.path(), HashMap::new())?;
    assert_eq!(ServerConfig::from_environment(values)?.port, 9000);
    Ok(())
}

#[test]
fn malformed_dotenv_fails_without_echoing_its_contents() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join(".env"), "PASSWORD=\"secret-without-closing-quote")?;
    let result = environment_from(directory.path(), HashMap::new());
    assert!(matches!(result, Err(ConfigurationError::Syntax(_))));
    if let Err(error) = result {
        assert!(!format!("{error:?}").contains("secret-without-closing-quote"));
    }
    Ok(())
}

#[test]
fn invalid_or_out_of_range_server_port_is_rejected() {
    for port in ["invalid", "65536", "-1"] {
        let values = HashMap::from([("SERVER_PORT".into(), port.into())]);
        assert!(ServerConfig::from_environment(values).is_err(), "port: {port}");
    }
}
