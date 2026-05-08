//! Embedded Codex runtime wiring for selfwork sessions.
//!
//! Step 3a-i stops at constructing the app-server startup arguments. It does
//! not start a thread or submit user turns yet; later steps own the session
//! runner. Keeping this boundary explicit lets the runtime configuration get
//! reviewed before selfwork starts talking to a model.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::EnvironmentManager;
use codex_app_server_client::EnvironmentManagerArgs;
use codex_app_server_client::ExecServerRuntimePaths;
use codex_app_server_client::InProcessClientStartArgs;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudRequirementsLoader;
use codex_config::LoaderOverrides;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use toml::Value as TomlValue;

use crate::Mode;
use crate::ModeBundle;

const SELFWORK_CLIENT_NAME: &str = "selfwork";
const SELFWORK_SESSION_SOURCE: &str = "selfwork";

/// Builds the in-process app-server startup configuration used by selfwork.
///
/// The builder deliberately stops before starting [`InProcessAppServerClient`].
/// That keeps Step 3a-i limited to dependency/configuration wiring; actual
/// thread lifecycle and turn handling land in the session-runner steps.
#[derive(Debug, Clone)]
pub struct CodexRuntimeBuilder {
    mode: Mode,
    arg0_paths: Arg0DispatchPaths,
    cli_overrides: Vec<(String, TomlValue)>,
    loader_overrides: LoaderOverrides,
    cwd: Option<PathBuf>,
    codex_home: Option<PathBuf>,
    enable_codex_api_key_env: bool,
    client_version: String,
    channel_capacity: usize,
}

impl CodexRuntimeBuilder {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            arg0_paths: Arg0DispatchPaths::default(),
            cli_overrides: Vec::new(),
            loader_overrides: LoaderOverrides::default(),
            cwd: None,
            codex_home: None,
            enable_codex_api_key_env: false,
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
        }
    }

    pub fn with_arg0_paths(mut self, arg0_paths: Arg0DispatchPaths) -> Self {
        self.arg0_paths = arg0_paths;
        self
    }

    pub fn with_cli_overrides(mut self, cli_overrides: Vec<(String, TomlValue)>) -> Self {
        self.cli_overrides = cli_overrides;
        self
    }

    pub fn with_loader_overrides(mut self, loader_overrides: LoaderOverrides) -> Self {
        self.loader_overrides = loader_overrides;
        self
    }

    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    pub fn with_codex_home(mut self, codex_home: PathBuf) -> Self {
        self.codex_home = Some(codex_home);
        self
    }

    pub fn with_enable_codex_api_key_env(mut self, enabled: bool) -> Self {
        self.enable_codex_api_key_env = enabled;
        self
    }

    pub fn with_client_version(mut self, client_version: impl Into<String>) -> Self {
        self.client_version = client_version.into();
        self
    }

    pub fn with_channel_capacity(mut self, channel_capacity: usize) -> Self {
        self.channel_capacity = channel_capacity;
        self
    }

    pub async fn build_start_args(self) -> io::Result<InProcessClientStartArgs> {
        let Some(mode_bundle) = ModeBundle::for_mode(self.mode) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mode `{}` is not implemented; cannot build Codex runtime",
                    self.mode.display_name()
                ),
            ));
        };

        let mut cli_overrides = self.cli_overrides;
        force_memories_off(&mut cli_overrides);

        let harness_overrides = ConfigOverrides {
            cwd: self.cwd,
            base_instructions: Some(mode_bundle.render_system_prompt()),
            ..Default::default()
        };

        let mut config_builder = ConfigBuilder::default()
            .cli_overrides(cli_overrides.clone())
            .harness_overrides(harness_overrides)
            .loader_overrides(self.loader_overrides.clone())
            .cloud_requirements(CloudRequirementsLoader::default());
        if let Some(codex_home) = self.codex_home {
            config_builder = config_builder.codex_home(codex_home);
        }

        let mut config = config_builder.build().await?;
        // Keep this explicit even though the CLI overrides above should also
        // resolve to false. Selfwork has its own longitudinal state model.
        config.memories.generate_memories = false;
        config.memories.use_memories = false;

        let runtime_paths = ExecServerRuntimePaths::from_optional_paths(
            self.arg0_paths.codex_self_exe.clone(),
            self.arg0_paths.codex_linux_sandbox_exe.clone(),
        )?;
        let environment_manager =
            Arc::new(EnvironmentManager::new(EnvironmentManagerArgs::new(runtime_paths)).await);

        Ok(InProcessClientStartArgs {
            arg0_paths: self.arg0_paths,
            config: Arc::new(config),
            cli_overrides,
            loader_overrides: self.loader_overrides,
            cloud_requirements: CloudRequirementsLoader::default(),
            feedback: CodexFeedback::new(),
            log_db: None,
            environment_manager,
            config_warnings: Vec::new(),
            session_source: SessionSource::Custom(SELFWORK_SESSION_SOURCE.to_string()),
            enable_codex_api_key_env: self.enable_codex_api_key_env,
            client_name: SELFWORK_CLIENT_NAME.to_string(),
            client_version: self.client_version,
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: self.channel_capacity,
        })
    }
}

fn force_memories_off(cli_overrides: &mut Vec<(String, TomlValue)>) {
    cli_overrides.push((
        "memories.use_memories".to_string(),
        TomlValue::Boolean(false),
    ));
    cli_overrides.push((
        "memories.generate_memories".to_string(),
        TomlValue::Boolean(false),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn test_arg0_paths() -> Arg0DispatchPaths {
        Arg0DispatchPaths {
            codex_self_exe: Some(std::env::current_exe().expect("current test binary")),
            ..Default::default()
        }
    }

    #[test]
    fn build_start_args_sets_selfwork_identity_and_prompt() {
        let codex_home = TempDir::new().expect("codex home");
        let cwd = TempDir::new().expect("cwd");

        let start_args = tokio_test::block_on(
            CodexRuntimeBuilder::new(Mode::Explore)
                .with_arg0_paths(test_arg0_paths())
                .with_codex_home(codex_home.path().to_path_buf())
                .with_cwd(cwd.path().to_path_buf())
                .build_start_args(),
        )
        .expect("start args");

        assert_eq!(start_args.client_name, "selfwork");
        assert_eq!(
            start_args.session_source,
            SessionSource::Custom("selfwork".to_string())
        );
        assert!(!start_args.enable_codex_api_key_env);
        assert!(start_args.experimental_api);
        assert_eq!(
            start_args.channel_capacity,
            DEFAULT_IN_PROCESS_CHANNEL_CAPACITY
        );

        let base_instructions = start_args
            .config
            .base_instructions
            .as_deref()
            .expect("selfwork base instructions");
        assert!(base_instructions.contains("# Base Invariants"));
        assert!(base_instructions.contains("# Mode: Explore"));
    }

    #[test]
    fn build_start_args_disables_codex_memories() {
        let codex_home = TempDir::new().expect("codex home");
        let cwd = TempDir::new().expect("cwd");

        let start_args = tokio_test::block_on(
            CodexRuntimeBuilder::new(Mode::Explore)
                .with_arg0_paths(test_arg0_paths())
                .with_codex_home(codex_home.path().to_path_buf())
                .with_cwd(cwd.path().to_path_buf())
                .build_start_args(),
        )
        .expect("start args");

        assert!(!start_args.config.memories.use_memories);
        assert!(!start_args.config.memories.generate_memories);
        assert!(start_args.cli_overrides.iter().any(|(key, value)| {
            key == "memories.use_memories" && value == &TomlValue::Boolean(false)
        }));
        assert!(start_args.cli_overrides.iter().any(|(key, value)| {
            key == "memories.generate_memories" && value == &TomlValue::Boolean(false)
        }));
    }

    #[test]
    fn build_start_args_rejects_unimplemented_modes() {
        let codex_home = TempDir::new().expect("codex home");

        let result = tokio_test::block_on(
            CodexRuntimeBuilder::new(Mode::Plan)
                .with_codex_home(codex_home.path().to_path_buf())
                .build_start_args(),
        );
        let err = match result {
            Ok(_) => panic!("Plan is not implemented yet"),
            Err(err) => err,
        };

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("Plan"),
            "unexpected error message: {err}"
        );
    }
}
