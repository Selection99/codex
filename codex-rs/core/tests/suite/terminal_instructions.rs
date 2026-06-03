use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
const TERMINAL_INSTRUCTIONS_START: &str = "- This surface is a terminal.";
const TERMINAL_INSTRUCTIONS_END: &str = "- Use only ASCII characters in visuals.";

async fn request_for_session_source(source: SessionSource) -> Result<ResponsesRequest> {
    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let thread = test
        .thread_manager
        .start_thread_with_options(StartThreadOptions {
            config: test.config.clone(),
            initial_history: InitialHistory::New,
            session_source: Some(source),
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
        })
        .await?;

    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.cwd_path());
    thread
        .thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                cwd: Some(test.config.cwd.to_path_buf()),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                model: Some(thread.session_configured.model.clone()),
                ..Default::default()
            },
        })
        .await?;
    wait_for_event(&thread.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(request.single_request())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_and_exec_sessions_include_terminal_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    for source in [SessionSource::Cli, SessionSource::Exec] {
        let request = request_for_session_source(source).await?;
        assert!(request.body_contains_text(TERMINAL_INSTRUCTIONS_START));
        assert!(request.body_contains_text(TERMINAL_INSTRUCTIONS_END));
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_terminal_sessions_omit_terminal_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let request = request_for_session_source(SessionSource::VSCode).await?;
    assert!(!request.body_contains_text(TERMINAL_INSTRUCTIONS_START));
    assert!(!request.body_contains_text(TERMINAL_INSTRUCTIONS_END));

    Ok(())
}
