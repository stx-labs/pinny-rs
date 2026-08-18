use crate::config::*;
use serial_test::serial;
use std::{fs::File, io::Write};
use tempfile::{tempdir, TempDir};

fn create_cargo_toml(content: &str) -> TempDir {
    let tmp_dir = tempdir().unwrap();
    let dir_path = tmp_dir.path();
    let cargo_path = dir_path.join("Cargo.toml");
    let mut cargo =
        File::create(cargo_path).expect("failed to create Cargo.toml file");
    cargo.write_all(content.as_bytes()).expect("cannot write");
    tmp_dir
}

#[test]
#[serial]
fn test_create_config_with_3_tags_ok() {
    let content = r#"
[Package]
name = "consumer"
version = "0.0.1"

[package.metadata.pinny]
allowed = ["tag1", "tag2", "tag3"]
"#;

    let tmp_dir = create_cargo_toml(content);
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_ok());

    let config = config_result.unwrap();

    assert_eq!(3, config.allowed_tags.len());
    assert_eq!("tag1", config.allowed_tags[0]);
    assert_eq!("tag2", config.allowed_tags[1]);
    assert_eq!("tag3", config.allowed_tags[2]);
}

#[test]
#[serial]
fn test_get_instance_caches_config_per_manifest_dir_ok() {
    let content_a = r#"
[Package]
name = "consumer_a"
version = "0.0.1"

[package.metadata.pinny]
allowed = ["tag_a"]
"#;
    let content_b = r#"
[Package]
name = "consumer_b"
version = "0.0.1"

[package.metadata.pinny]
allowed = ["tag_b"]
"#;

    let tmp_dir_a = create_cargo_toml(content_a);
    let tmp_dir_b = create_cargo_toml(content_b);

    // Simulate a single long-lived process (e.g. rust-analyzer's proc-macro
    // server) expanding macros for two crates with different allowed tags.
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir_a.path());
    let config_a = Config::get_instance().unwrap();
    assert_eq!(vec!["tag_a".to_string()], config_a.allowed_tags);

    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir_b.path());
    let config_b = Config::get_instance().unwrap();
    assert_eq!(vec!["tag_b".to_string()], config_b.allowed_tags);

    // The first crate's config must still be served on a cache hit.
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir_a.path());
    let config_a_again = Config::get_instance().unwrap();
    assert_eq!(vec!["tag_a".to_string()], config_a_again.allowed_tags);
}

#[test]
#[serial]
fn test_create_config_with_missing_file_ko() {
    let tmp_dir = tempdir().unwrap();
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_err());
    assert!(matches!(
        config_result.err().unwrap(),
        ConfigError::ReadError(_)
    ));
}

#[test]
#[serial]
fn test_create_config_with_invalid_toml_ko() {
    let content = r#"
[Pa
name = "consumer"
version = "0.0.1"

[package.metadata.pinny]
"#;

    let tmp_dir = create_cargo_toml(content);
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_err());
    assert!(matches!(
        config_result.err().unwrap(),
        ConfigError::ParseError(_)
    ));
}

#[test]
#[serial]
fn test_create_config_with_missing_allowed_tags_ko() {
    let content = r#"
[Package]
name = "consumer"
version = "0.0.1"

[package.metadata.pinny]
"#;

    let tmp_dir = create_cargo_toml(content);
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_err());
    assert_eq!(ConfigError::MissingTags, config_result.err().unwrap());
}

#[test]
#[serial]
fn test_create_config_with_missing_metadata_ko() {
    let content = r#"
[Package]
name = "consumer"
version = "0.0.1"
"#;

    let tmp_dir = create_cargo_toml(content);
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_err());
    assert_eq!(ConfigError::MissingTags, config_result.err().unwrap());
}

#[test]
#[serial]
fn test_create_config_with_duplicated_tag_ko() {
    let content = r#"
[Package]
name = "consumer"
version = "0.0.1"

[package.metadata.pinny]
allowed = ["tag1", "tag1", "tag3"]
"#;

    let tmp_dir = create_cargo_toml(content);
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_err());
    assert_eq!(
        ConfigError::DuplicateTag("tag1".into()),
        config_result.err().unwrap()
    );
}

#[test]
#[serial]
fn test_create_config_with_invalid_tag_format_ko() {
    let content = r#"
[Package]
name = "consumer"
version = "0.0.1"

[package.metadata.pinny]
allowed = ["?invalid", "tag2", "tag3"]
"#;

    let tmp_dir = create_cargo_toml(content);
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_err());
    assert_eq!(
        ConfigError::InvalidTagFormat("?invalid".into()),
        config_result.err().unwrap()
    );
}

#[test]
#[serial]
fn test_create_config_with_invalid_tags_array_ko() {
    let content = r#"
[Package]
name = "consumer"
version = "0.0.1"

[package.metadata.pinny]
allowed = "tag1"
"#;

    let tmp_dir = create_cargo_toml(content);
    std::env::set_var("CARGO_MANIFEST_DIR", tmp_dir.path());

    let config_result = ConfigFactory::create();
    assert!(config_result.is_err());
    assert_eq!(
        ConfigError::InvalidArrayFormat,
        config_result.err().unwrap()
    );
}
