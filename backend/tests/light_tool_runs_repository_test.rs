use async_trait::async_trait;
use backend::{
    agent_core::runner::{
        anonymous_light_tool_tools, run_agent_loop, AgentFailureMessages, AgentLoopInput,
        AgentPrincipal, AgentRunError,
    },
    pg_models::NewPgMessageHistory,
    pg_schema::{light_tool_devices, light_tool_runs},
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_runs_repository::{
            AccountRunCreation, AnonymousTrialRunCreation, LightToolRunsRepository,
        },
    },
    services::{
        light_tool_agent_responder::{
            build_account_agent_input, build_anonymous_chat_request, LightToolAgentResponder,
        },
        light_tool_bootstrap::{
            LightToolBootstrapService, TRIAL_DURATION_SECONDS, TRIAL_MESSAGE_LIMIT,
        },
        light_tool_identity::hash_installation_id,
        light_tool_run_dispatcher::{
            dispatch_light_tool_run, LightToolConversationTurn, LightToolDispatchOutcome,
            LightToolResponder, LightToolRunPrincipal,
        },
        light_tool_run_execution::{
            LightToolRunExecutionError, LightToolRunExecutionService, MAX_RUN_ACTIVITY_CHARACTERS,
        },
        light_tool_run_supervisor::supervise_light_tool_runs_once,
    },
    test_utils::{
        create_test_pg_pool, create_test_state, create_test_user, MockLlmResponse, TestUserParams,
    },
    utils::encryption::decrypt,
};
use diesel::prelude::*;
use std::sync::{Arc, Barrier};
use tokio::sync::mpsc;

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const NOW: i32 = 1_700_000_000;
const TEST_ENCRYPTION_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

#[test]
fn anonymous_ai_request_contains_only_system_and_user_messages() {
    let history = vec![LightToolConversationTurn {
        user_message: "My name is Ada".to_string(),
        assistant_message: "Nice to meet you, Ada.".to_string(),
    }];
    let request = build_anonymous_chat_request(&history, "What is my name?");

    assert_eq!(request.messages.len(), 4);
    assert_eq!(
        request.messages[0].role,
        openai_api_rs::v1::chat_completion::MessageRole::system
    );
    assert_eq!(
        request.messages[3].role,
        openai_api_rs::v1::chat_completion::MessageRole::user
    );
    assert!(request.tools.is_none());

    let system_prompt = match &request.messages[0].content {
        openai_api_rs::v1::chat_completion::Content::Text(text) => text,
        _ => panic!("expected a text system prompt"),
    };
    assert!(system_prompt.contains("anonymous trial"));
    assert!(system_prompt.contains("weather and web-search tools"));
    assert!(system_prompt.contains("cannot send messages"));

    assert_eq!(
        request.messages[1].role,
        openai_api_rs::v1::chat_completion::MessageRole::user
    );
    assert_eq!(
        request.messages[2].role,
        openai_api_rs::v1::chat_completion::MessageRole::assistant
    );

    let user_message = match &request.messages[3].content {
        openai_api_rs::v1::chat_completion::Content::Text(text) => text,
        _ => panic!("expected a text user message"),
    };
    assert_eq!(user_message, "What is my name?");
}

#[test]
fn anonymous_ai_exposes_only_public_read_tools() {
    let names = anonymous_light_tool_tools()
        .into_iter()
        .map(|tool| tool.function.name)
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["get_weather", "search_firecrawl"]);
}

#[tokio::test]
async fn anonymous_agent_uses_the_shared_runner_for_direct_responses() {
    let ai_config = backend::AiConfig::default_for_tests();
    let tools = anonymous_light_tool_tools();
    let request = build_anonymous_chat_request(&[], "Hello");
    let mut mock_llm_response =
        Some(MockLlmResponse::with_direct_response("Hello from the shared agent").to_response());
    let mock_tool_responses = None;
    let reasoning_tx = None;

    let output = run_agent_loop(AgentLoopInput {
        principal: AgentPrincipal::AnonymousLightTool {
            device_id: 42,
            ai_config: &ai_config,
        },
        model_purpose: backend::ModelPurpose::Default,
        user_given_info: "",
        image_url: None,
        tools: &tools,
        completion_messages: request.messages,
        skip_sms: true,
        reasoning_tx: &reasoning_tx,
        status_tx: None,
        mock_llm_response: &mut mock_llm_response,
        mock_tool_responses: &mock_tool_responses,
        current_time: NOW,
        failure_messages: AgentFailureMessages::anonymous_trial(),
    })
    .await
    .unwrap();

    assert_eq!(output.final_response, "Hello from the shared agent");
}

#[tokio::test]
async fn anonymous_agent_rejects_an_unexposed_tool_without_executing_it() {
    let ai_config = backend::AiConfig::default_for_tests();
    let tools = anonymous_light_tool_tools();
    let request = build_anonymous_chat_request(&[], "Do an account action");
    let mut mock_llm_response = Some(MockLlmResponse::with_invalid_tool_call().to_response());
    let mock_tool_responses = None;
    let reasoning_tx = None;

    let result = run_agent_loop(AgentLoopInput {
        principal: AgentPrincipal::AnonymousLightTool {
            device_id: 42,
            ai_config: &ai_config,
        },
        model_purpose: backend::ModelPurpose::Default,
        user_given_info: "",
        image_url: None,
        tools: &tools,
        completion_messages: request.messages,
        skip_sms: true,
        reasoning_tx: &reasoning_tx,
        status_tx: None,
        mock_llm_response: &mut mock_llm_response,
        mock_tool_responses: &mock_tool_responses,
        current_time: NOW,
        failure_messages: AgentFailureMessages::anonymous_trial(),
    })
    .await;

    assert!(matches!(
        result,
        Err(AgentRunError::EarlyReturn {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ..
        })
    ));
}

#[tokio::test]
async fn shared_agent_returns_an_error_instead_of_panicking_on_empty_choices() {
    let ai_config = backend::AiConfig::default_for_tests();
    let tools = anonymous_light_tool_tools();
    let request = build_anonymous_chat_request(&[], "Hello");
    let mut response = MockLlmResponse::with_direct_response("unused").to_response();
    response.choices.clear();
    let mut mock_llm_response = Some(response);
    let mock_tool_responses = None;
    let reasoning_tx = None;

    let result = run_agent_loop(AgentLoopInput {
        principal: AgentPrincipal::AnonymousLightTool {
            device_id: 42,
            ai_config: &ai_config,
        },
        model_purpose: backend::ModelPurpose::Default,
        user_given_info: "",
        image_url: None,
        tools: &tools,
        completion_messages: request.messages,
        skip_sms: true,
        reasoning_tx: &reasoning_tx,
        status_tx: None,
        mock_llm_response: &mut mock_llm_response,
        mock_tool_responses: &mock_tool_responses,
        current_time: NOW,
        failure_messages: AgentFailureMessages::anonymous_trial(),
    })
    .await;

    assert!(matches!(result, Err(AgentRunError::System { .. })));
}

#[test]
fn anonymous_ai_request_bounds_each_historical_message() {
    let history = vec![LightToolConversationTurn {
        user_message: "a".repeat(2_100),
        assistant_message: "b".repeat(2_100),
    }];
    let request = build_anonymous_chat_request(&history, "Continue");

    for message in &request.messages[1..=2] {
        let openai_api_rs::v1::chat_completion::Content::Text(text) = &message.content else {
            panic!("expected text history");
        };
        assert_eq!(text.chars().count(), 2_000);
        assert!(text.ends_with(" [truncated]"));
    }
}

#[tokio::test]
#[serial_test::serial]
async fn account_agent_input_uses_shared_history_and_authorized_tools() {
    set_test_encryption_key();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    state
        .user_repository
        .create_message_history(&NewPgMessageHistory {
            user_id: user.id,
            role: "user".to_string(),
            encrypted_content: "Earlier cross-channel question".to_string(),
            tool_name: None,
            tool_call_id: None,
            tool_calls_json: None,
            created_at: chrono::Utc::now().timestamp() as i32 - 60,
            conversation_id: "".to_string(),
        })
        .unwrap();

    let input = build_account_agent_input(&state, &user, "Current Light Tool question")
        .await
        .unwrap();
    let system_prompt = match &input.completion_messages[0].content {
        openai_api_rs::v1::chat_completion::Content::Text(text) => text,
        _ => panic!("expected a text system prompt"),
    };
    assert!(system_prompt.contains("Lightfriend tool on a Light Phone"));
    assert!(input.completion_messages.iter().any(|message| {
        matches!(
            &message.content,
            openai_api_rs::v1::chat_completion::Content::Text(text)
                if text.contains("Earlier cross-channel question")
        )
    }));
    let current_messages = input
        .completion_messages
        .iter()
        .filter(|message| {
            matches!(
                &message.content,
                openai_api_rs::v1::chat_completion::Content::Text(text)
                    if text == "Current Light Tool question"
            )
        })
        .count();
    assert_eq!(current_messages, 1);

    let tool_names = input
        .tools
        .iter()
        .map(|tool| tool.function.name.as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"send_chat_message"));
    assert!(tool_names.contains(&"query_message"));
}

#[tokio::test]
#[serial_test::serial]
async fn account_responder_returns_subscription_guidance_without_calling_the_model() {
    set_test_encryption_key();
    let state = create_test_state();
    let mut params = TestUserParams::us_user(10.0, 5.0);
    params.sub_tier = None;
    let user = create_test_user(&state, &params);
    let responder = LightToolAgentResponder::new(state.ai_config.clone(), Arc::downgrade(&state));
    let (activity_tx, _activity_rx) = mpsc::channel(8);

    let response = responder
        .respond(
            LightToolRunPrincipal::Account {
                device_id: 42,
                user_id: user.id,
            },
            &[],
            "Check my email",
            activity_tx,
        )
        .await
        .unwrap();

    assert!(response.contains("active subscription"));
}

fn set_test_encryption_key() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
}

fn create_device() -> (backend::PgDbPool, i32) {
    let pool = create_test_pg_pool();
    let bootstrap = LightToolBootstrapService::new(LightToolDevicesRepository::new(pool.clone()));
    bootstrap.bootstrap(INSTALLATION_ID, None, NOW).unwrap();
    let installation_hash = hash_installation_id(INSTALLATION_ID).unwrap();
    let device = LightToolDevicesRepository::new(pool.clone())
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    (pool, device.id)
}

#[test]
#[serial_test::serial]
fn creates_encrypted_run_and_reads_plaintext_for_its_device() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());

    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "client-message-1",
            "What is the weather?",
            NOW + 1,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created {
        run,
        messages_remaining,
    } = created
    else {
        panic!("expected a newly created run");
    };

    assert_eq!(messages_remaining, TRIAL_MESSAGE_LIMIT - 1);
    assert_eq!(run.user_message, "What is the weather?");
    assert_eq!(run.status, "queued");
    assert_eq!(run.account_user_id, None);

    let stored_ciphertext = {
        let mut conn = pool.get().unwrap();
        light_tool_runs::table
            .find(&run.id)
            .select(light_tool_runs::encrypted_user_message)
            .first::<String>(&mut conn)
            .unwrap()
    };
    assert_ne!(stored_ciphertext, "What is the weather?");
    assert_eq!(decrypt(&stored_ciphertext).unwrap(), "What is the weather?");

    let found = repository
        .find_by_id_for_device(&run.id, device_id)
        .unwrap()
        .unwrap();
    assert_eq!(found, run);
    assert!(repository
        .find_by_id_for_device(&run.id, device_id + 1)
        .unwrap()
        .is_none());
}

#[test]
#[serial_test::serial]
fn linked_account_run_snapshots_principal_without_spending_trial_quota() {
    set_test_encryption_key();
    let state = create_test_state();
    let bootstrap =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    bootstrap.bootstrap(INSTALLATION_ID, None, NOW).unwrap();
    let installation_hash = hash_installation_id(INSTALLATION_ID).unwrap();
    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::update(light_tool_devices::table.find(device.id))
            .set(light_tool_devices::user_id.eq(Some(user.id)))
            .execute(&mut conn)
            .unwrap();
    }

    let repository = LightToolRunsRepository::new(state.pg_pool.clone());
    let created = repository
        .create_account_run(
            device.id,
            user.id,
            "linked-client-message",
            "Check my email",
            NOW + TRIAL_DURATION_SECONDS,
        )
        .unwrap();
    let AccountRunCreation::Created(run) = created else {
        panic!("expected a newly created account run");
    };
    assert_eq!(run.account_user_id, Some(user.id));

    let replay = repository
        .create_account_run(
            device.id,
            user.id,
            "linked-client-message",
            "Changed retry text",
            NOW + TRIAL_DURATION_SECONDS + 1,
        )
        .unwrap();
    let AccountRunCreation::Existing(replay) = replay else {
        panic!("expected an idempotent account replay");
    };
    assert_eq!(replay.id, run.id);
    assert_eq!(replay.user_message, "Check my email");

    let stored_device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    assert_eq!(stored_device.trial_messages_used, 0);

    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::update(light_tool_devices::table.find(device.id))
            .set(light_tool_devices::user_id.eq::<Option<i32>>(None))
            .execute(&mut conn)
            .unwrap();
    }
    assert_eq!(
        repository
            .create_anonymous_trial_run(
                device.id,
                "linked-client-message",
                "Try to reuse this id anonymously",
                NOW + 1,
                TRIAL_MESSAGE_LIMIT,
            )
            .unwrap(),
        AnonymousTrialRunCreation::IdempotencyConflict
    );
}

#[test]
#[serial_test::serial]
fn replay_returns_original_run_without_spending_quota_again() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());

    let first = repository
        .create_anonymous_trial_run(
            device_id,
            "same-client-message",
            "Original text",
            NOW + 1,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let replay = repository
        .create_anonymous_trial_run(
            device_id,
            "same-client-message",
            "Changed retry text",
            NOW + 2,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();

    let AnonymousTrialRunCreation::Created { run: first, .. } = first else {
        panic!("expected a newly created run");
    };
    let AnonymousTrialRunCreation::Existing {
        run: replay,
        messages_remaining,
    } = replay
    else {
        panic!("expected an idempotent replay");
    };
    assert_eq!(replay.id, first.id);
    assert_eq!(replay.user_message, "Original text");
    assert_eq!(messages_remaining, TRIAL_MESSAGE_LIMIT - 1);

    let stored_device = LightToolDevicesRepository::new(pool)
        .find_by_installation_hash(&hash_installation_id(INSTALLATION_ID).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(stored_device.trial_messages_used, 1);
}

#[test]
#[serial_test::serial]
fn concurrent_retries_create_and_charge_only_one_run() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = Arc::new(LightToolRunsRepository::new(pool.clone()));
    let start = Arc::new(Barrier::new(2));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let repository = repository.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                repository
                    .create_anonymous_trial_run(
                        device_id,
                        "concurrent-client-message",
                        "Hello",
                        NOW + 1,
                        TRIAL_MESSAGE_LIMIT,
                    )
                    .unwrap()
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, AnonymousTrialRunCreation::Created { .. }))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, AnonymousTrialRunCreation::Existing { .. }))
            .count(),
        1
    );

    let stored_device = LightToolDevicesRepository::new(pool)
        .find_by_installation_hash(&hash_installation_id(INSTALLATION_ID).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(stored_device.trial_messages_used, 1);
}

#[test]
#[serial_test::serial]
fn unavailable_trial_does_not_create_a_run() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());

    let result = repository
        .create_anonymous_trial_run(
            device_id,
            "expired-client-message",
            "Hello",
            NOW + TRIAL_DURATION_SECONDS,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    assert_eq!(result, AnonymousTrialRunCreation::TrialUnavailable);

    let count = {
        let mut conn = pool.get().unwrap();
        light_tool_runs::table
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap()
    };
    assert_eq!(count, 0);
}

#[test]
#[serial_test::serial]
fn execution_transitions_running_progress_and_completion_once() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());
    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "execution-completed",
            "Tell me the weather",
            NOW + 1,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run, .. } = created else {
        panic!("expected a newly created run");
    };
    let execution = LightToolRunExecutionService::new(LightToolRunsRepository::new(pool.clone()));

    let claimed = execution
        .claim(&run.id, "  THINKING...  ", NOW + 2)
        .unwrap()
        .unwrap();
    assert_eq!(claimed.status, "running");
    assert_eq!(claimed.activity_text.as_deref(), Some("THINKING..."));
    assert!(execution
        .claim(&run.id, "THINKING...", NOW + 3)
        .unwrap()
        .is_none());

    let progress = execution
        .update_activity(&run.id, "SEARCHING THE WEB", NOW + 4)
        .unwrap()
        .unwrap();
    assert_eq!(progress.activity_text.as_deref(), Some("SEARCHING THE WEB"));

    let completed = execution
        .complete(&run.id, "  It will be sunny.  ", NOW + 5)
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert_eq!(
        completed.assistant_message.as_deref(),
        Some("It will be sunny.")
    );
    assert_eq!(completed.activity_text, None);
    assert_eq!(completed.completed_at, Some(NOW + 5));
    assert!(execution
        .update_activity(&run.id, "TOO LATE", NOW + 6)
        .unwrap()
        .is_none());
    assert!(execution
        .fail(&run.id, "TOO LATE", NOW + 6)
        .unwrap()
        .is_none());

    let (encrypted_activity, encrypted_assistant): (Option<String>, Option<String>) = {
        let mut conn = pool.get().unwrap();
        light_tool_runs::table
            .find(&run.id)
            .select((
                light_tool_runs::encrypted_activity_text,
                light_tool_runs::encrypted_assistant_message,
            ))
            .first(&mut conn)
            .unwrap()
    };
    assert_eq!(encrypted_activity, None);
    assert_ne!(encrypted_assistant.as_deref(), Some("It will be sunny."));
    assert_eq!(
        decrypt(encrypted_assistant.as_deref().unwrap()).unwrap(),
        "It will be sunny."
    );
}

#[test]
#[serial_test::serial]
fn execution_records_encrypted_failure_and_rejects_invalid_text() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());
    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "execution-failed",
            "Search for something",
            NOW + 1,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run, .. } = created else {
        panic!("expected a newly created run");
    };
    let execution = LightToolRunExecutionService::new(LightToolRunsRepository::new(pool.clone()));

    assert!(matches!(
        execution.claim(&run.id, "   ", NOW + 2),
        Err(LightToolRunExecutionError::BlankText {
            field: "activity_text"
        })
    ));
    assert!(matches!(
        execution.claim(
            &run.id,
            &"x".repeat(MAX_RUN_ACTIVITY_CHARACTERS + 1),
            NOW + 2
        ),
        Err(LightToolRunExecutionError::TextTooLong {
            field: "activity_text",
            ..
        })
    ));

    execution.claim(&run.id, "THINKING...", NOW + 2).unwrap();
    let failed = execution
        .fail(&run.id, "  Search temporarily unavailable  ", NOW + 3)
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(
        failed.error_message.as_deref(),
        Some("Search temporarily unavailable")
    );
    assert_eq!(failed.activity_text, None);
    assert_eq!(failed.completed_at, Some(NOW + 3));
    assert!(execution
        .complete(&run.id, "TOO LATE", NOW + 4)
        .unwrap()
        .is_none());

    let encrypted_error = {
        let mut conn = pool.get().unwrap();
        light_tool_runs::table
            .find(&run.id)
            .select(light_tool_runs::encrypted_error_message)
            .first::<Option<String>>(&mut conn)
            .unwrap()
            .unwrap()
    };
    assert_ne!(encrypted_error, "Search temporarily unavailable");
    assert_eq!(
        decrypt(&encrypted_error).unwrap(),
        "Search temporarily unavailable"
    );
}

#[test]
#[serial_test::serial]
fn concurrent_execution_claims_have_one_winner() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());
    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "execution-concurrent",
            "Hello",
            NOW + 1,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run, .. } = created else {
        panic!("expected a newly created run");
    };
    let execution = Arc::new(LightToolRunExecutionService::new(
        LightToolRunsRepository::new(pool),
    ));
    let start = Arc::new(Barrier::new(2));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let execution = execution.clone();
            let start = start.clone();
            let run_id = run.id.clone();
            std::thread::spawn(move || {
                start.wait();
                execution.claim(&run_id, "THINKING...", NOW + 2).unwrap()
            })
        })
        .collect();
    let claimed_count = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(Option::is_some)
        .count();
    assert_eq!(claimed_count, 1);
}

struct FakeResponder {
    activities: Vec<String>,
    expected_history: Vec<LightToolConversationTurn>,
    response: Result<String, String>,
}

#[async_trait]
impl LightToolResponder for FakeResponder {
    async fn respond(
        &self,
        _principal: LightToolRunPrincipal,
        history: &[LightToolConversationTurn],
        _user_message: &str,
        activity_tx: mpsc::Sender<String>,
    ) -> Result<String, String> {
        assert_eq!(history, self.expected_history);
        for activity in &self.activities {
            activity_tx.send(activity.clone()).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        self.response.clone()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn dispatcher_persists_progress_and_completes_fake_response() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());
    let previous = repository
        .create_anonymous_trial_run(
            device_id,
            "dispatcher-previous",
            "My name is Ada",
            NOW + 1,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run: previous, .. } = previous else {
        panic!("expected a newly created previous run");
    };
    repository
        .claim_queued_run(&previous.id, "THINKING...", NOW + 2)
        .unwrap();
    repository
        .complete_running_run(&previous.id, "Nice to meet you, Ada.", NOW + 3)
        .unwrap();

    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "dispatcher-completed",
            "What is my name?",
            NOW + 4,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run, .. } = created else {
        panic!("expected a newly created run");
    };
    let responder = Arc::new(FakeResponder {
        activities: vec!["SEARCHING THE WEB".to_string()],
        expected_history: vec![LightToolConversationTurn {
            user_message: "My name is Ada".to_string(),
            assistant_message: "Nice to meet you, Ada.".to_string(),
        }],
        response: Ok("Your name is Ada.".to_string()),
    });
    let dispatch = tokio::spawn(dispatch_light_tool_run(
        pool.clone(),
        run.id.clone(),
        responder,
    ));

    let mut saw_progress = false;
    for _ in 0..50 {
        let current = repository
            .find_by_id_for_device(&run.id, device_id)
            .unwrap()
            .unwrap();
        if current.activity_text.as_deref() == Some("SEARCHING THE WEB") {
            saw_progress = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(saw_progress);
    assert_eq!(
        dispatch.await.unwrap().unwrap(),
        LightToolDispatchOutcome::Completed
    );

    let completed = repository
        .find_by_id_for_device(&run.id, device_id)
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert_eq!(
        completed.assistant_message.as_deref(),
        Some("Your name is Ada.")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn dispatcher_sanitizes_fake_provider_failure() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());
    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "dispatcher-failed",
            "Hello",
            NOW + 1,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run, .. } = created else {
        panic!("expected a newly created run");
    };
    let responder = Arc::new(FakeResponder {
        activities: Vec::new(),
        expected_history: Vec::new(),
        response: Err("provider error containing internal details".to_string()),
    });

    assert_eq!(
        dispatch_light_tool_run(pool, run.id.clone(), responder)
            .await
            .unwrap(),
        LightToolDispatchOutcome::Failed
    );
    let failed = repository
        .find_by_id_for_device(&run.id, device_id)
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.error_message.as_deref(), Some("REQUEST FAILED"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn supervisor_dispatches_queued_runs_left_by_a_restart() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());
    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "supervisor-queued",
            "Hello",
            NOW,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run, .. } = created else {
        panic!("expected a newly created run");
    };
    let responder = Arc::new(FakeResponder {
        activities: Vec::new(),
        expected_history: Vec::new(),
        response: Ok("Recovered reply".to_string()),
    });

    supervise_light_tool_runs_once(pool, responder)
        .await
        .unwrap();

    for _ in 0..100 {
        let current = repository
            .find_by_id_for_device(&run.id, device_id)
            .unwrap()
            .unwrap();
        if current.status == "completed" {
            assert_eq!(
                current.assistant_message.as_deref(),
                Some("Recovered reply")
            );
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("supervisor did not complete queued run");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn supervisor_fails_stale_running_runs_without_replaying_them() {
    set_test_encryption_key();
    let (pool, device_id) = create_device();
    let repository = LightToolRunsRepository::new(pool.clone());
    let created = repository
        .create_anonymous_trial_run(
            device_id,
            "supervisor-stale",
            "Send a message once",
            NOW,
            TRIAL_MESSAGE_LIMIT,
        )
        .unwrap();
    let AnonymousTrialRunCreation::Created { run, .. } = created else {
        panic!("expected a newly created run");
    };
    repository
        .claim_queued_run(&run.id, "THINKING...", NOW + 1)
        .unwrap();
    let responder = Arc::new(FakeResponder {
        activities: Vec::new(),
        expected_history: Vec::new(),
        response: Ok("This must not run".to_string()),
    });

    supervise_light_tool_runs_once(pool, responder)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let failed = repository
        .find_by_id_for_device(&run.id, device_id)
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.error_message.as_deref(), Some("REQUEST INTERRUPTED"));
    assert!(failed.assistant_message.is_none());
}
