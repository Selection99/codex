use crate::certs::ManagedMitmCaTrustBundle;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

pub(crate) fn prepare_mitm_ca_trust_bundle_env<F>(
    mitm_ca_trust_bundle: &ManagedMitmCaTrustBundle,
    env: &mut HashMap<String, String>,
    cwd: &Path,
    startup_ca_env_keys_present_in_child: &[&'static str],
    can_read_path: F,
) -> Vec<AbsolutePathBuf>
where
    F: Fn(&Path) -> bool,
{
    let ssl_cert_dir_contents = read_child_ca_dir_contents(
        mitm_ca_trust_bundle,
        env,
        cwd,
        startup_ca_env_keys_present_in_child,
        &can_read_path,
    );
    for key in crate::certs::CUSTOM_CA_ENV_KEYS {
        let Some(value) = env.get(key).filter(|value| !value.is_empty()) else {
            continue;
        };
        let mut custom_ca_bundle = read_child_ca_bundle_contents(
            mitm_ca_trust_bundle,
            key,
            value,
            cwd,
            startup_ca_env_keys_present_in_child,
            &can_read_path,
        )
        .unwrap_or_default();
        if key == "SSL_CERT_FILE"
            && let Some(ssl_cert_dir_contents) = ssl_cert_dir_contents.as_deref()
        {
            crate::certs::append_pem_contents(&mut custom_ca_bundle, ssl_cert_dir_contents);
        }
        if custom_ca_bundle.is_empty() {
            continue;
        }

        match crate::certs::materialize_ca_trust_bundle_with_custom_ca(
            mitm_ca_trust_bundle,
            &custom_ca_bundle,
        ) {
            Ok(path) => {
                env.insert(key.to_string(), path.to_string_lossy().into_owned());
            }
            Err(err) => {
                warn!(
                    ca_env_key = key,
                    "failed to materialize child MITM CA bundle; leaving current value unchanged: {err}"
                );
            }
        }
    }

    managed_mitm_ca_trust_bundle_paths_for_env(mitm_ca_trust_bundle, env)
}

fn resolve_ca_bundle_path(path: &str, cwd: &Path) -> std::path::PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn read_child_ca_bundle_contents<F>(
    mitm_ca_trust_bundle: &ManagedMitmCaTrustBundle,
    key: &'static str,
    value: &str,
    cwd: &Path,
    startup_ca_env_keys_present_in_child: &[&'static str],
    can_read_path: &F,
) -> Option<String>
where
    F: Fn(&Path) -> bool,
{
    let value_path = Path::new(value);
    let custom_ca_bundle_path = if value_path == mitm_ca_trust_bundle.path {
        if !startup_ca_env_keys_present_in_child.contains(&key) {
            return None;
        }
        let startup_value = mitm_ca_trust_bundle.startup_env_values.get(key)?;
        resolve_ca_bundle_path(startup_value, &mitm_ca_trust_bundle.startup_cwd)
    } else if crate::certs::is_generated_trust_bundle_path(value_path, mitm_ca_trust_bundle) {
        return None;
    } else {
        resolve_ca_bundle_path(value, cwd)
    };
    match crate::certs::read_custom_ca_bundle(&custom_ca_bundle_path, can_read_path) {
        Ok(contents) => Some(contents),
        Err(err) => {
            warn!(
                ca_env_key = key,
                ca_bundle_path = %custom_ca_bundle_path.display(),
                "failed to read child MITM CA bundle; leaving current value unchanged: {err}"
            );
            None
        }
    }
}

fn read_child_ca_dir_contents<F>(
    mitm_ca_trust_bundle: &ManagedMitmCaTrustBundle,
    env: &HashMap<String, String>,
    cwd: &Path,
    startup_ca_env_keys_present_in_child: &[&'static str],
    can_read_path: &F,
) -> Option<String>
where
    F: Fn(&Path) -> bool,
{
    let value = env
        .get(crate::certs::SSL_CERT_DIR_ENV_KEY)
        .filter(|value| !value.is_empty())?;
    let ca_dir_cwd =
        if startup_ca_env_keys_present_in_child.contains(&crate::certs::SSL_CERT_DIR_ENV_KEY) {
            &mitm_ca_trust_bundle.startup_cwd
        } else {
            cwd
        };
    let mut trust_bundle = String::new();
    for ca_dir_path in std::env::split_paths(value).map(|path| {
        if path.is_absolute() {
            path
        } else {
            ca_dir_cwd.join(path)
        }
    }) {
        match crate::certs::read_custom_ca_dir(&ca_dir_path, can_read_path) {
            Ok(contents) if !contents.is_empty() => {
                crate::certs::append_pem_contents(&mut trust_bundle, &contents);
            }
            Ok(_) => {}
            Err(err) => {
                warn!(
                    ca_bundle_path = %ca_dir_path.display(),
                    "failed to read child MITM CA directory; leaving current value unchanged: {err}"
                );
            }
        }
    }
    if trust_bundle.is_empty() {
        None
    } else {
        Some(trust_bundle)
    }
}

fn managed_mitm_ca_trust_bundle_paths_for_env(
    mitm_ca_trust_bundle: &ManagedMitmCaTrustBundle,
    env: &HashMap<String, String>,
) -> Vec<AbsolutePathBuf> {
    let mut paths = crate::certs::CUSTOM_CA_ENV_KEYS
        .into_iter()
        .filter_map(|key| env.get(key))
        .map(Path::new)
        .filter(|path| {
            *path == mitm_ca_trust_bundle.path
                || crate::certs::is_generated_trust_bundle_path(path, mitm_ca_trust_bundle)
        })
        .filter_map(|path| AbsolutePathBuf::from_absolute_path(path).ok())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
#[path = "child_ca_tests.rs"]
mod tests;
