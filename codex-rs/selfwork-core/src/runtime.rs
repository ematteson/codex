//! Embedded Codex runtime wiring for selfwork sessions.
//!
//! Step 3a adds the runtime session wrapper on top of the app-server startup
//! wiring. Selfwork keeps Codex's in-process app-server as the execution
//! boundary, but owns thread lifecycle, turn submission, and response
//! collection here.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::EnvironmentManager;
use codex_app_server_client::EnvironmentManagerArgs;
use codex_app_server_client::ExecServerRuntimePaths;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudRequirementsLoader;
use codex_config::LoaderOverrides;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionSource;
use toml::Value as TomlValue;

use crate::Mode;
use crate::ModeBundle;

const SELFWORK_CLIENT_NAME: &str = "selfwork";
const SELFWORK_SESSION_SOURCE: &str = "selfwork";
const UNSUPPORTED_SERVER_REQUEST_CODE: i64 = -32000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfworkTurn {
    pub user_message: String,
    pub assistant_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneShotOutput {
    pub assistant_text: String,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug)]
struct RequestIdSequencer {
    next: i64,
}

impl RequestIdSequencer {
    fn new() -> Self {
        Self { next: 1 }
    }

    fn next(&mut self) -> RequestId {
        let id = self.next;
        self.next += 1;
        RequestId::Integer(id)
    }
}

/// Builds the in-process app-server startup configuration used by selfwork.
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
    developer_instructions: Option<String>,
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
            developer_instructions: None,
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

    pub fn with_developer_instructions(
        mut self,
        developer_instructions: impl Into<String>,
    ) -> Self {
        self.developer_instructions = Some(developer_instructions.into());
        self
    }

    pub async fn start_session(self) -> io::Result<SelfworkSession> {
        SelfworkSession::start(self).await
    }

    pub async fn run_one_shot(self, user_message: impl Into<String>) -> io::Result<OneShotOutput> {
        let mut session = self.start_session().await?;
        let response = session.send_user_message(user_message.into()).await;
        let shutdown = session.shutdown().await;
        let response = response?;
        shutdown?;
        Ok(response)
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
            approval_policy: Some(AskForApproval::Never),
            base_instructions: Some(mode_bundle.render_system_prompt()),
            developer_instructions: self.developer_instructions,
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

        let mut arg0_paths = self.arg0_paths;
        if arg0_paths.codex_self_exe.is_none() {
            arg0_paths.codex_self_exe = Some(std::env::current_exe()?);
        }

        let runtime_paths = ExecServerRuntimePaths::from_optional_paths(
            arg0_paths.codex_self_exe.clone(),
            arg0_paths.codex_linux_sandbox_exe.clone(),
        )?;
        let environment_manager =
            Arc::new(EnvironmentManager::new(EnvironmentManagerArgs::new(runtime_paths)).await);

        Ok(InProcessClientStartArgs {
            arg0_paths,
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

pub struct SelfworkSession {
    mode: Mode,
    client: InProcessAppServerClient,
    request_ids: RequestIdSequencer,
    thread_id: String,
    cwd: PathBuf,
    approval_policy: codex_app_server_protocol::AskForApproval,
    reasoning_effort: Option<ReasoningEffort>,
    turns: Vec<SelfworkTurn>,
}

impl SelfworkSession {
    pub async fn start(builder: CodexRuntimeBuilder) -> io::Result<Self> {
        let mode = builder.mode;
        let start_args = builder.build_start_args().await?;
        let config = Arc::clone(&start_args.config);
        let cwd = config.cwd.to_path_buf();
        let approval_policy: codex_app_server_protocol::AskForApproval =
            config.permissions.approval_policy.value().into();
        let reasoning_effort = config.model_reasoning_effort;
        let mut request_ids = RequestIdSequencer::new();
        let client = InProcessAppServerClient::start(start_args).await?;

        let response: ThreadStartResponse = send_request_with_response(
            &client,
            ClientRequest::ThreadStart {
                request_id: request_ids.next(),
                params: ThreadStartParams {
                    model: config.model.clone(),
                    model_provider: Some(config.model_provider_id.clone()),
                    cwd: Some(cwd.to_string_lossy().to_string()),
                    approval_policy: Some(approval_policy),
                    approvals_reviewer: Some(config.approvals_reviewer.into()),
                    base_instructions: config.base_instructions.clone(),
                    developer_instructions: config.developer_instructions.clone(),
                    ephemeral: Some(config.ephemeral),
                    ..ThreadStartParams::default()
                },
            },
            "thread/start",
        )
        .await?;

        Ok(Self {
            mode,
            client,
            request_ids,
            thread_id: response.thread.id,
            cwd,
            approval_policy,
            reasoning_effort,
            turns: Vec::new(),
        })
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn thread_id(&self) -> &str {
        &self.thread_id
    }

    pub fn turns(&self) -> &[SelfworkTurn] {
        &self.turns
    }

    pub async fn send_user_message(
        &mut self,
        user_message: impl Into<String>,
    ) -> io::Result<OneShotOutput> {
        let user_message = user_message.into();
        let response: TurnStartResponse = send_request_with_response(
            &self.client,
            ClientRequest::TurnStart {
                request_id: self.request_ids.next(),
                params: TurnStartParams {
                    thread_id: self.thread_id.clone(),
                    input: vec![UserInput::Text {
                        text: user_message.clone(),
                        text_elements: Vec::new(),
                    }],
                    responsesapi_client_metadata: None,
                    environments: None,
                    cwd: Some(self.cwd.clone()),
                    approval_policy: Some(self.approval_policy),
                    approvals_reviewer: None,
                    sandbox_policy: None,
                    permissions: None,
                    model: None,
                    service_tier: None,
                    effort: self.reasoning_effort,
                    summary: None,
                    personality: None,
                    output_schema: None,
                    collaboration_mode: None,
                },
            },
            "turn/start",
        )
        .await?;
        let turn_id = response.turn.id;
        let assistant_text = self.collect_assistant_text(&turn_id).await?;
        self.turns.push(SelfworkTurn {
            user_message,
            assistant_text: assistant_text.clone(),
        });
        Ok(OneShotOutput {
            assistant_text,
            thread_id: self.thread_id.clone(),
            turn_id,
        })
    }

    pub async fn shutdown(self) -> io::Result<()> {
        self.client.shutdown().await
    }

    async fn collect_assistant_text(&mut self, turn_id: &str) -> io::Result<String> {
        let mut collector = AssistantTextCollector::default();
        loop {
            let Some(event) = self.client.next_event().await else {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "app-server event stream closed before the turn completed",
                ));
            };
            match event {
                InProcessServerEvent::Lagged { .. } => {}
                InProcessServerEvent::ServerRequest(request) => {
                    reject_server_request(&self.client, request).await?;
                }
                InProcessServerEvent::ServerNotification(notification) => {
                    if let Some(text) =
                        collector.process_notification(notification, &self.thread_id, turn_id)?
                    {
                        return Ok(text);
                    }
                }
            }
        }
    }
}

#[derive(Default)]
struct AssistantTextCollector {
    delta_text: String,
    completed_text: Option<String>,
}

impl AssistantTextCollector {
    fn process_notification(
        &mut self,
        notification: ServerNotification,
        thread_id: &str,
        turn_id: &str,
    ) -> io::Result<Option<String>> {
        match notification {
            ServerNotification::AgentMessageDelta(payload)
                if payload.thread_id == thread_id && payload.turn_id == turn_id =>
            {
                self.delta_text.push_str(&payload.delta);
            }
            ServerNotification::ItemCompleted(payload)
                if payload.thread_id == thread_id && payload.turn_id == turn_id =>
            {
                if let ThreadItem::AgentMessage { text, .. } = payload.item {
                    self.completed_text = Some(text);
                }
            }
            ServerNotification::Error(payload)
                if payload.thread_id == thread_id
                    && payload.turn_id == turn_id
                    && !payload.will_retry =>
            {
                return Err(io::Error::other(payload.error.to_string()));
            }
            ServerNotification::TurnCompleted(payload)
                if payload.thread_id == thread_id && payload.turn.id == turn_id =>
            {
                return match payload.turn.status {
                    TurnStatus::Completed => {
                        let text = self
                            .completed_text
                            .clone()
                            .unwrap_or_else(|| self.delta_text.clone());
                        Ok(Some(text))
                    }
                    TurnStatus::Failed => {
                        let message = payload
                            .turn
                            .error
                            .map(|err| err.to_string())
                            .unwrap_or_else(|| "turn failed".to_string());
                        Err(io::Error::other(message))
                    }
                    TurnStatus::Interrupted => Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "turn interrupted",
                    )),
                    TurnStatus::InProgress => Ok(None),
                };
            }
            _ => {}
        }
        Ok(None)
    }
}

async fn send_request_with_response<T>(
    client: &InProcessAppServerClient,
    request: ClientRequest,
    method: &str,
) -> io::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    client
        .request_typed(request)
        .await
        .map_err(|err| io::Error::other(format!("{method}: {err}")))
}

async fn reject_server_request(
    client: &InProcessAppServerClient,
    request: ServerRequest,
) -> io::Result<()> {
    let method = server_request_method_name(&request);
    client
        .reject_server_request(
            request.id().clone(),
            JSONRPCErrorError {
                code: UNSUPPORTED_SERVER_REQUEST_CODE,
                message: format!(
                    "selfwork does not support `{}` server requests yet",
                    method.unwrap_or("unknown")
                ),
                data: None,
            },
        )
        .await
}

fn server_request_method_name(request: &ServerRequest) -> Option<&'static str> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { .. } => {
            Some("item/commandExecution/requestApproval")
        }
        ServerRequest::FileChangeRequestApproval { .. } => Some("item/fileChange/requestApproval"),
        ServerRequest::ToolRequestUserInput { .. } => Some("item/tool/requestUserInput"),
        ServerRequest::ApplyPatchApproval { .. } => Some("applyPatchApproval"),
        ServerRequest::ExecCommandApproval { .. } => Some("execCommandApproval"),
        ServerRequest::McpServerElicitationRequest { .. } => Some("mcpServer/elicitation/request"),
        ServerRequest::PermissionsRequestApproval { .. } => {
            Some("item/permissions/requestApproval")
        }
        ServerRequest::DynamicToolCall { .. } => Some("item/tool/call"),
        ServerRequest::ChatgptAuthTokensRefresh { .. } => Some("account/chatgptAuthTokens/refresh"),
    }
}

pub async fn run_one_shot(mode: Mode, user_message: impl Into<String>) -> io::Result<String> {
    Ok(CodexRuntimeBuilder::new(mode)
        .run_one_shot(user_message)
        .await?
        .assistant_text)
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
            CodexRuntimeBuilder::new(Mode::Reflect)
                .with_codex_home(codex_home.path().to_path_buf())
                .build_start_args(),
        );
        let err = match result {
            Ok(_) => panic!("Reflect is not implemented yet"),
            Err(err) => err,
        };

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("Reflect"),
            "unexpected error message: {err}"
        );
    }
}
