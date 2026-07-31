use std::collections::HashMap;

use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigLayerSource;
use codex_config::LoaderOverrides;
use codex_config::McpServerTransportConfig;
use codex_config::build_cli_overrides_layer;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use toml::Value as TomlValue;

use super::ConfigManager;
use super::merge_session_overrides;

fn merged_session_layer(
    cli_overrides: Vec<(String, TomlValue)>,
    request_overrides: HashMap<String, serde_json::Value>,
) -> TomlValue {
    build_cli_overrides_layer(&merge_session_overrides(
        &cli_overrides,
        request_overrides,
    ))
}

#[test]
fn request_table_preserves_unaddressed_process_cli_leaves() {
    let cli_overrides = vec![
        (
            "mcp_servers.node_repl.env.HTTP_PROXY".to_string(),
            TomlValue::String("http://proxy.example:2407".to_string()),
        ),
        (
            "mcp_servers.node_repl.env.NODE_USE_ENV_PROXY".to_string(),
            TomlValue::String("1".to_string()),
        ),
        (
            "mcp_servers.node_repl.args".to_string(),
            TomlValue::Array(vec![TomlValue::String("old.js".to_string())]),
        ),
    ];
    let request_overrides = HashMap::from([(
        "mcp_servers.node_repl".to_string(),
        serde_json::json!({
            "command": "node",
            "args": ["node_repl.js"],
            "env": {
                "NODE_REPL_NODE_PATH": "/opt/node"
            }
        }),
    )]);

    let merged = merged_session_layer(cli_overrides, request_overrides);
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.node_repl]
command = "node"
args = ["node_repl.js"]

[mcp_servers.node_repl.env]
HTTP_PROXY = "http://proxy.example:2407"
NODE_USE_ENV_PROXY = "1"
NODE_REPL_NODE_PATH = "/opt/node"
"#,
    )
    .expect("expected config should be valid TOML");

    assert_eq!(merged, expected);
}

#[test]
fn explicit_request_leaf_overrides_process_cli_leaf() {
    let cli_overrides = vec![(
        "mcp_servers.node_repl.env.HTTP_PROXY".to_string(),
        TomlValue::String("http://process-proxy.example:2407".to_string()),
    )];
    let request_overrides = HashMap::from([(
        "mcp_servers.node_repl.env.HTTP_PROXY".to_string(),
        serde_json::json!("http://request-proxy.example:2407"),
    )]);

    let merged = merged_session_layer(cli_overrides, request_overrides);
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.node_repl.env]
HTTP_PROXY = "http://request-proxy.example:2407"
"#,
    )
    .expect("expected config should be valid TOML");

    assert_eq!(merged, expected);
}

#[test]
fn request_table_env_leaf_overrides_process_cli_leaf() {
    let cli_overrides = vec![(
        "mcp_servers.node_repl.env.HTTP_PROXY".to_string(),
        TomlValue::String("http://process-proxy.example:2407".to_string()),
    )];
    let request_overrides = HashMap::from([(
        "mcp_servers.node_repl".to_string(),
        serde_json::json!({
            "command": "node",
            "args": [],
            "env": {
                "HTTP_PROXY": "http://request-proxy.example:2407"
            }
        }),
    )]);

    let merged = merged_session_layer(cli_overrides, request_overrides);
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.node_repl]
command = "node"
args = []

[mcp_servers.node_repl.env]
HTTP_PROXY = "http://request-proxy.example:2407"
"#,
    )
    .expect("expected config should be valid TOML");

    assert_eq!(merged, expected);
}

#[test]
fn request_table_replaces_http_mcp_server_with_stdio() {
    let cli_overrides = vec![(
        "mcp_servers.sample.url".to_string(),
        TomlValue::String("https://example.test/mcp".to_string()),
    )];
    let request_overrides = HashMap::from([(
        "mcp_servers.sample".to_string(),
        serde_json::json!({
            "command": "echo",
            "args": []
        }),
    )]);

    let merged = merged_session_layer(cli_overrides, request_overrides);
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.sample]
command = "echo"
args = []
"#,
    )
    .expect("expected config should be valid TOML");

    assert_eq!(merged, expected);
}

#[test]
fn request_table_replaces_stdio_mcp_server_with_http() {
    let cli_overrides = vec![
        (
            "mcp_servers.sample.command".to_string(),
            TomlValue::String("echo".to_string()),
        ),
        (
            "mcp_servers.sample.args".to_string(),
            TomlValue::Array(Vec::new()),
        ),
        (
            "mcp_servers.sample.env.HTTP_PROXY".to_string(),
            TomlValue::String("http://proxy.example:2407".to_string()),
        ),
    ];
    let request_overrides = HashMap::from([(
        "mcp_servers.sample".to_string(),
        serde_json::json!({
            "url": "https://example.test/mcp"
        }),
    )]);

    let merged = merged_session_layer(cli_overrides, request_overrides);
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.sample]
url = "https://example.test/mcp"
"#,
    )
    .expect("expected config should be valid TOML");

    assert_eq!(merged, expected);
}

#[test]
fn request_descendant_deterministically_overrides_request_ancestor() {
    let cli_overrides = vec![(
        "mcp_servers.node_repl.env.HTTP_PROXY".to_string(),
        TomlValue::String("http://process-proxy.example:2407".to_string()),
    )];
    let request_overrides = HashMap::from([
        (
            "mcp_servers.node_repl.env.HTTP_PROXY".to_string(),
            serde_json::json!("http://request-proxy.example:2407"),
        ),
        (
            "mcp_servers.node_repl".to_string(),
            serde_json::json!({
                "command": "node",
                "args": []
            }),
        ),
    ]);

    let merged = merged_session_layer(cli_overrides, request_overrides);
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.node_repl]
command = "node"
args = []

[mcp_servers.node_repl.env]
HTTP_PROXY = "http://request-proxy.example:2407"
"#,
    )
    .expect("expected config should be valid TOML");

    assert_eq!(merged, expected);
}

#[test]
fn request_table_keeps_model_provider_variant_atomic() {
    let cli_overrides = vec![(
        "model_providers.sample.env_key".to_string(),
        TomlValue::String("SAMPLE_API_KEY".to_string()),
    )];
    let request_overrides = HashMap::from([(
        "model_providers.sample".to_string(),
        serde_json::json!({
            "name": "Sample",
            "base_url": "https://example.test/v1",
            "wire_api": "responses",
            "auth": {
                "type": "oauth"
            }
        }),
    )]);

    let merged = merged_session_layer(cli_overrides, request_overrides);
    let provider = merged
        .get("model_providers")
        .and_then(TomlValue::as_table)
        .and_then(|providers| providers.get("sample"))
        .and_then(TomlValue::as_table)
        .expect("sample provider should remain a table");

    assert!(!provider.contains_key("env_key"));
    assert!(provider.contains_key("auth"));
}

#[tokio::test]
async fn config_manager_deserializes_merged_mcp_server() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    std::fs::write(
        codex_home.path().join(codex_config::CONFIG_TOML_FILE),
        r#"
[mcp_servers.node_repl]
command = "node"
args = ["user.js"]
"#,
    )?;
    let manager = ConfigManager::new_for_tests(
        codex_home.path().to_path_buf(),
        vec![
            (
                "mcp_servers.node_repl.env.HTTP_PROXY".to_string(),
                TomlValue::String("http://proxy.example:2407".to_string()),
            ),
            (
                "mcp_servers.node_repl.env.NODE_USE_ENV_PROXY".to_string(),
                TomlValue::String("1".to_string()),
            ),
        ],
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleLoader::default(),
    );
    let request_overrides = HashMap::from([(
        "mcp_servers.node_repl".to_string(),
        serde_json::json!({
            "command": "node",
            "args": ["request.js"],
            "env": {
                "NODE_REPL_NODE_PATH": "/opt/node"
            }
        }),
    )]);

    let config = manager
        .load_with_overrides(
            Some(request_overrides),
            codex_core::config::ConfigOverrides::default(),
        )
        .await?;
    let server = config
        .mcp_servers
        .get()
        .get("node_repl")
        .expect("node_repl should be configured");
    let origins = config.config_layer_stack.origins();

    assert_eq!(
        server.transport,
        McpServerTransportConfig::Stdio {
            command: "node".to_string(),
            args: vec!["request.js".to_string()],
            env: Some(HashMap::from([
                (
                    "HTTP_PROXY".to_string(),
                    "http://proxy.example:2407".to_string(),
                ),
                ("NODE_USE_ENV_PROXY".to_string(), "1".to_string()),
                ("NODE_REPL_NODE_PATH".to_string(), "/opt/node".to_string()),
            ])),
            env_vars: Vec::new(),
            cwd: None,
        }
    );
    assert_eq!(
        origins
            .get("mcp_servers.node_repl.env.HTTP_PROXY")
            .map(|metadata| &metadata.name),
        Some(&ConfigLayerSource::SessionFlags)
    );
    assert_eq!(
        origins
            .get("mcp_servers.node_repl.env.NODE_REPL_NODE_PATH")
            .map(|metadata| &metadata.name),
        Some(&ConfigLayerSource::SessionFlags)
    );
    Ok(())
}
