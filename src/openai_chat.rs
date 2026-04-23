use crate::adapter;
use crate::coding_agent;
use crate::models::{CodingAgentConfig, RepoRecord};
use crate::{executor, overview};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::env;

pub struct OpenAiChatResponse {
    pub assistant_text: String,
    pub response_id: Option<String>,
}

pub fn respond(
    user_message: &str,
    repos: &[RepoRecord],
    active_repo: Option<usize>,
    previous_response_id: Option<&str>,
    coding_agent_config: &CodingAgentConfig,
) -> Result<OpenAiChatResponse> {
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => anyhow::bail!("OPENAI_API_KEY is not configured"),
    };
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4".to_string());
    let client = reqwest::blocking::Client::builder()
        .build()
        .context("failed to build OpenAI client")?;

    let instructions = build_instructions(repos, active_repo);
    let tools = build_tools();
    let mut response = create_response(
        &client,
        &api_key,
        &model,
        Some(json!([{ "role": "user", "content": user_message }])),
        &instructions,
        previous_response_id,
        Some(&tools),
    )?;

    loop {
        let function_calls = extract_function_calls(&response);
        if function_calls.is_empty() {
            let assistant_text = extract_output_text(&response);
            return Ok(OpenAiChatResponse {
                assistant_text,
                response_id: response
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            });
        }

        let mut tool_outputs = Vec::new();
        for call in function_calls {
            let name = call.get("name").and_then(Value::as_str).unwrap_or_default();
            let call_id = call
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let args = call
                .get("arguments")
                .and_then(Value::as_str)
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .unwrap_or(Value::Null);
            let output = handle_tool_call(name, args, repos, coding_agent_config)?;
            tool_outputs.push(json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": serde_json::to_string(&output)?,
            }));
        }

        response = create_response(
            &client,
            &api_key,
            &model,
            Some(Value::Array(tool_outputs)),
            &instructions,
            response
                .get("id")
                .and_then(Value::as_str)
                .or(previous_response_id),
            None,
        )?;
    }
}

fn create_response(
    client: &reqwest::blocking::Client,
    api_key: &str,
    model: &str,
    input: Option<Value>,
    instructions: &str,
    previous_response_id: Option<&str>,
    tools: Option<&[Value]>,
) -> Result<Value> {
    let mut body = json!({
        "model": model,
        "instructions": instructions,
        "parallel_tool_calls": false,
        "max_output_tokens": 2000,
    });
    if let Some(input) = input {
        body["input"] = input;
    }
    if let Some(previous_response_id) = previous_response_id {
        body["previous_response_id"] = Value::String(previous_response_id.to_string());
    }
    if let Some(tools) = tools {
        body["tools"] = Value::Array(tools.to_vec());
    }

    let response = client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .context("failed to call OpenAI Responses API")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        anyhow::bail!("OpenAI Responses API error {}: {}", status, body);
    }

    response
        .json::<Value>()
        .context("failed to parse OpenAI response JSON")
}

fn build_instructions(repos: &[RepoRecord], active_repo: Option<usize>) -> String {
    let repo_lines = repos
        .iter()
        .enumerate()
        .map(|(index, repo)| {
            let marker = if Some(index) == active_repo {
                "active"
            } else {
                "available"
            };
            let commands = repo
                .commands
                .iter()
                .take(12)
                .map(|command| command.command_name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "- {} ({}, status={}): {}",
                repo.repo_id,
                marker,
                repo.status.label(),
                commands
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are youbot, a repo orchestration assistant. Prefer function tools over freeform guessing. Use run_repo_command for existing justfile capabilities. Use run_adapter_change when the user wants to change how a repo is presented inside youbot. Use run_code_change only when the user is asking to change child-repo code, logic, data model, or repo-owned outputs. If uncertain about whether a change belongs to the adapter or the child repo, ask a clarification question instead of making up commands.\n\nRegistered repos:\n{repo_lines}"
    )
}

fn build_tools() -> Vec<Value> {
    vec![
        function_tool(
            "list_repos",
            "List registered repos and their statuses.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        function_tool(
            "list_commands",
            "List available just commands for a repo.",
            json!({
                "type":"object",
                "properties":{"repo_id":{"type":"string"}},
                "required":["repo_id"],
                "additionalProperties":false
            }),
        ),
        function_tool(
            "run_repo_command",
            "Run a just command in a repo.",
            json!({
                "type":"object",
                "properties":{
                    "repo_id":{"type":"string"},
                    "command_name":{"type":"string"},
                    "arguments":{"type":"array","items":{"type":"string"}}
                },
                "required":["repo_id","command_name"],
                "additionalProperties":false
            }),
        ),
        function_tool(
            "run_code_change",
            "Invoke the configured coding-agent backend in a repo for a code change request.",
            json!({
                "type":"object",
                "properties":{
                    "repo_id":{"type":"string"},
                    "request":{"type":"string"}
                },
                "required":["repo_id","request"],
                "additionalProperties":false
            }),
        ),
        function_tool(
            "run_adapter_change",
            "Update a youbot-owned adapter for how a repo is presented in the TUI.",
            json!({
                "type":"object",
                "properties":{
                    "repo_id":{"type":"string"},
                    "request":{"type":"string"}
                },
                "required":["repo_id","request"],
                "additionalProperties":false
            }),
        ),
    ]
}

fn function_tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({
        "type":"function",
        "name":name,
        "description":description,
        "parameters":parameters
    })
}

fn extract_function_calls(response: &Value) -> Vec<Value> {
    response
        .get("output")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn extract_output_text(response: &Value) -> String {
    if let Some(text) = response.get("output_text").and_then(Value::as_str) {
        return text.to_string();
    }

    response
        .get("output")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if item.get("type").and_then(Value::as_str) == Some("message") {
                        item.get("content")
                            .and_then(Value::as_array)
                            .map(|content| {
                                content
                                    .iter()
                                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "No assistant output returned.".to_string())
}

fn handle_tool_call(
    name: &str,
    args: Value,
    repos: &[RepoRecord],
    coding_agent_config: &CodingAgentConfig,
) -> Result<Value> {
    match name {
        "list_repos" => Ok(json!(
            repos
                .iter()
                .map(|repo| {
                    json!({
                        "repo_id": repo.repo_id,
                        "name": repo.name,
                        "status": repo.status.label(),
                        "commands": repo.commands.len(),
                    })
                })
                .collect::<Vec<_>>()
        )),
        "list_commands" => {
            let repo = find_repo(repos, args.get("repo_id").and_then(Value::as_str))?;
            Ok(json!(
                repo.commands
                    .iter()
                    .map(|command| {
                        json!({
                            "command_name": command.command_name,
                            "description": command.description,
                        })
                    })
                    .collect::<Vec<_>>()
            ))
        }
        "run_repo_command" => {
            let repo = find_repo(repos, args.get("repo_id").and_then(Value::as_str))?;
            let command_name = args
                .get("command_name")
                .and_then(Value::as_str)
                .context("missing command_name")?;
            let arguments = args
                .get("arguments")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(ToString::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let result = executor::run(repo, command_name, &arguments)?;
            Ok(json!({
                "repo_id": repo.repo_id,
                "command_name": command_name,
                "exit_code": result.exit_code,
                "summary": overview::summarize_execution(repo, &result),
                "stdout": result.stdout,
                "stderr": result.stderr,
            }))
        }
        "run_code_change" => {
            let repo = find_repo(repos, args.get("repo_id").and_then(Value::as_str))?;
            let request = args
                .get("request")
                .and_then(Value::as_str)
                .context("missing request")?;
            let result = coding_agent::run_code_change(repo, request, coding_agent_config)?;
            Ok(json!({
                "repo_id": repo.repo_id,
                "backend_name": result.backend_name,
                "session_id": result.session_id,
                "exit_code": result.exit_code,
                "summary": result.summary,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }))
        }
        "run_adapter_change" => {
            let repo = find_repo(repos, args.get("repo_id").and_then(Value::as_str))?;
            let request = args
                .get("request")
                .and_then(Value::as_str)
                .context("missing request")?;
            let summary = adapter::apply_change(repo, request)?;
            Ok(json!({
                "repo_id": repo.repo_id,
                "summary": summary,
            }))
        }
        _ => Ok(json!({"error": format!("unknown tool {name}")})),
    }
}

fn find_repo<'a>(repos: &'a [RepoRecord], repo_id: Option<&str>) -> Result<&'a RepoRecord> {
    let repo_id = repo_id.context("missing repo_id")?;
    repos
        .iter()
        .find(|repo| repo.repo_id == repo_id)
        .with_context(|| format!("unknown repo_id {repo_id}"))
}
