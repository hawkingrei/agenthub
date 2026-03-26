use std::collections::HashMap;
use std::fmt;

use pyroscope::PyroscopeAgent;
use pyroscope::backend::{BackendConfig, PprofConfig, pprof_backend};
use pyroscope::pyroscope::{PyroscopeAgentBuilder, PyroscopeAgentRunning};

pub const PYROSCOPE_BASIC_AUTH_PASSWORD_ENV: &str = "PYROSCOPE_BASIC_AUTH_PASSWORD";
pub const PYROSCOPE_BASIC_AUTH_USER_ENV: &str = "PYROSCOPE_BASIC_AUTH_USER";
pub const PYROSCOPE_SERVER_ADDRESS_ENV: &str = "PYROSCOPE_SERVER_ADDRESS";

const PYROSCOPE_SAMPLE_RATE: u32 = 100;
const PYROSCOPE_SPY_NAME: &str = "pyroscope-rs";
const REQUIRED_ENV_KEYS: [&str; 3] = [
    PYROSCOPE_BASIC_AUTH_PASSWORD_ENV,
    PYROSCOPE_BASIC_AUTH_USER_ENV,
    PYROSCOPE_SERVER_ADDRESS_ENV,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyroscopeBootstrapOptions<'a> {
    pub application_name: &'a str,
    pub application_version: &'a str,
}

#[derive(Clone, PartialEq, Eq)]
struct PyroscopeEnvConfig {
    server_address: String,
    basic_auth_user: String,
    basic_auth_password: String,
}

impl fmt::Debug for PyroscopeEnvConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PyroscopeEnvConfig")
            .field("server_address", &self.server_address)
            .field("basic_auth_user", &self.basic_auth_user)
            .field("basic_auth_password", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PyroscopeEnvState {
    Disabled,
    Incomplete {
        present: Vec<&'static str>,
        missing: Vec<&'static str>,
    },
    Ready(PyroscopeEnvConfig),
}

pub struct ActivePyroscopeGuard {
    agent: Option<PyroscopeAgent<PyroscopeAgentRunning>>,
}

impl Drop for ActivePyroscopeGuard {
    fn drop(&mut self) {
        let Some(agent) = self.agent.take() else {
            return;
        };
        match agent.stop() {
            Ok(agent_ready) => {
                agent_ready.shutdown();
                tracing::info!("pyroscope profiler stopped");
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "pyroscope profiler stop failed during shutdown"
                );
            }
        }
    }
}

impl ActivePyroscopeGuard {
    fn start(
        options: PyroscopeBootstrapOptions<'_>,
        config: &PyroscopeEnvConfig,
    ) -> Result<Self, String> {
        let backend = pprof_backend(
            PprofConfig {
                sample_rate: PYROSCOPE_SAMPLE_RATE,
            },
            BackendConfig::default(),
        );
        let agent = PyroscopeAgentBuilder::new(
            &config.server_address,
            options.application_name,
            PYROSCOPE_SAMPLE_RATE,
            PYROSCOPE_SPY_NAME,
            options.application_version,
            backend,
        )
        .basic_auth(&config.basic_auth_user, &config.basic_auth_password)
        .tags(vec![
            ("service", "agenthub"),
            ("version", options.application_version),
        ])
        .build()
        .map_err(|err| err.to_string())?
        .start()
        .map_err(|err| err.to_string())?;
        Ok(Self { agent: Some(agent) })
    }
}

pub fn maybe_start_from_env(
    options: PyroscopeBootstrapOptions<'_>,
) -> Option<ActivePyroscopeGuard> {
    match load_env_state(std::env::vars()) {
        PyroscopeEnvState::Disabled => {
            tracing::info!("pyroscope profiler disabled: required environment variables not set");
            None
        }
        PyroscopeEnvState::Incomplete { present, missing } => {
            tracing::warn!(
                present = ?present,
                missing = ?missing,
                "pyroscope profiler environment is incomplete; skipping startup"
            );
            None
        }
        PyroscopeEnvState::Ready(config) => {
            tracing::info!(
                server_address = %config.server_address,
                basic_auth_user = %config.basic_auth_user,
                application_name = options.application_name,
                "starting pyroscope profiler"
            );
            match ActivePyroscopeGuard::start(options, &config) {
                Ok(guard) => {
                    tracing::info!(
                        server_address = %config.server_address,
                        application_name = options.application_name,
                        "pyroscope profiler started"
                    );
                    Some(guard)
                }
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        server_address = %config.server_address,
                        "pyroscope profiler startup failed; continuing without profiler"
                    );
                    None
                }
            }
        }
    }
}

fn load_env_state<K, V, I>(vars: I) -> PyroscopeEnvState
where
    K: AsRef<str>,
    V: AsRef<str>,
    I: IntoIterator<Item = (K, V)>,
{
    let env_map = vars
        .into_iter()
        .map(|(key, value)| (key.as_ref().to_string(), value.as_ref().to_string()))
        .collect::<HashMap<_, _>>();

    let present = REQUIRED_ENV_KEYS
        .iter()
        .copied()
        .filter(|key| read_trimmed_env_value(&env_map, key).is_some())
        .collect::<Vec<_>>();
    if present.is_empty() {
        return PyroscopeEnvState::Disabled;
    }

    let missing = REQUIRED_ENV_KEYS
        .iter()
        .copied()
        .filter(|key| read_trimmed_env_value(&env_map, key).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return PyroscopeEnvState::Incomplete { present, missing };
    }

    PyroscopeEnvState::Ready(PyroscopeEnvConfig {
        basic_auth_password: read_trimmed_env_value(&env_map, PYROSCOPE_BASIC_AUTH_PASSWORD_ENV)
            .expect("password present after missing check"),
        basic_auth_user: read_trimmed_env_value(&env_map, PYROSCOPE_BASIC_AUTH_USER_ENV)
            .expect("user present after missing check"),
        server_address: read_trimmed_env_value(&env_map, PYROSCOPE_SERVER_ADDRESS_ENV)
            .expect("server address present after missing check"),
    })
}

fn read_trimmed_env_value(env_map: &HashMap<String, String>, key: &str) -> Option<String> {
    env_map
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{
        PYROSCOPE_BASIC_AUTH_PASSWORD_ENV, PYROSCOPE_BASIC_AUTH_USER_ENV,
        PYROSCOPE_SERVER_ADDRESS_ENV, PyroscopeEnvConfig, PyroscopeEnvState, load_env_state,
    };

    #[test]
    fn load_env_state_disables_profiler_when_all_keys_are_missing() {
        assert_eq!(
            load_env_state(std::iter::empty::<(&str, &str)>()),
            PyroscopeEnvState::Disabled
        );
    }

    #[test]
    fn load_env_state_requires_all_three_keys() {
        let state = load_env_state([
            (PYROSCOPE_BASIC_AUTH_USER_ENV, "ops"),
            (
                PYROSCOPE_SERVER_ADDRESS_ENV,
                "https://pyroscope.example.com",
            ),
        ]);
        assert_eq!(
            state,
            PyroscopeEnvState::Incomplete {
                present: vec![PYROSCOPE_BASIC_AUTH_USER_ENV, PYROSCOPE_SERVER_ADDRESS_ENV,],
                missing: vec![PYROSCOPE_BASIC_AUTH_PASSWORD_ENV],
            }
        );
    }

    #[test]
    fn load_env_state_treats_blank_values_as_missing() {
        let state = load_env_state([
            (PYROSCOPE_BASIC_AUTH_PASSWORD_ENV, "  "),
            (PYROSCOPE_BASIC_AUTH_USER_ENV, " ops "),
            (
                PYROSCOPE_SERVER_ADDRESS_ENV,
                " https://pyroscope.example.com ",
            ),
        ]);
        assert_eq!(
            state,
            PyroscopeEnvState::Incomplete {
                present: vec![PYROSCOPE_BASIC_AUTH_USER_ENV, PYROSCOPE_SERVER_ADDRESS_ENV,],
                missing: vec![PYROSCOPE_BASIC_AUTH_PASSWORD_ENV],
            }
        );
    }

    #[test]
    fn load_env_state_accepts_trimmed_complete_configuration() {
        let state = load_env_state([
            (PYROSCOPE_BASIC_AUTH_PASSWORD_ENV, " secret "),
            (PYROSCOPE_BASIC_AUTH_USER_ENV, " ops "),
            (
                PYROSCOPE_SERVER_ADDRESS_ENV,
                " https://pyroscope.example.com ",
            ),
        ]);
        assert_eq!(
            state,
            PyroscopeEnvState::Ready(PyroscopeEnvConfig {
                basic_auth_password: "secret".to_string(),
                basic_auth_user: "ops".to_string(),
                server_address: "https://pyroscope.example.com".to_string(),
            })
        );
    }
}
