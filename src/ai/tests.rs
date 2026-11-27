use std::path::Path;

use super::backend::AiError;
use super::claude_cli::{args, parse_output, parse_stream_output, program, stream_input, transcript};
use super::openai::{parse_response, request_body};
use super::prompt::{system_prompt, PromptContext};
use super::protocol::{format_sets, parse_reply, segments, Parsed, MAX_TOTAL};
use crate::domain::{AiConfig, AiImage, AiTool, ChatMsg, DbKind, DbRead, ResultSet, Role, Row, Segment};

#[test]
fn plain_reply_is_an_answer() {
    assert_eq!(parse_reply("Pakai index."), Parsed::Answer("Pakai index.".into()));
}

#[test]
fn dbz_block_is_a_tool_call() {
    let r = "Cek dulu.\n```dbz\n{\"tool\":\"describe_table\",\"table\":\"orders\"}\n```";
    assert_eq!(parse_reply(r), Parsed::Call { text: "Cek dulu.".into(), tool: AiTool::Db(DbRead::DescribeTable { table: "orders".into() }) });
}

#[test]
fn propose_change_view_is_optional() {
    let r = "```dbz\n{\"tool\":\"propose_change\",\"title\":\"Index\",\"sql\":\"CREATE INDEX i ON t(a)\"}\n```";
    assert!(matches!(parse_reply(r), Parsed::Call { tool: AiTool::ProposeChange { view: None, .. }, .. }));
}

#[test]
fn fenced_json_is_not_a_tool_call() {
    assert!(matches!(parse_reply("```json\n{\"tool\":\"list_objects\"}\n```"), Parsed::Answer(_)));
}

#[test]
fn bad_or_double_calls_are_invalid() {
    assert!(matches!(parse_reply("```dbz\n{\"tool\":\"drop_all\"}\n```"), Parsed::Invalid { .. }));
    assert!(matches!(parse_reply("```dbz\nnot json\n```"), Parsed::Invalid { .. }));
    let two = "```dbz\n{\"tool\":\"list_objects\"}\n```\n```dbz\n{\"tool\":\"active_editor\"}\n```";
    assert!(matches!(parse_reply(two), Parsed::Invalid { error, .. } if error.contains("one tool call")));
}

#[test]
fn segments_split_sql_fences() {
    assert_eq!(
        segments("Coba:\n```sql\nSELECT 1;\n```\nSelesai."),
        vec![Segment::Text("Coba:".into()), Segment::Sql("SELECT 1;".into()), Segment::Text("Selesai.".into())]
    );
    assert_eq!(segments("```json\n{}\n```"), vec![Segment::Text("```json\n{}\n```".into())]);
}

#[test]
fn format_sets_truncates_rows_cells_and_total() {
    let rows: Vec<Row> = (0..10_000).map(|i| vec![Some(i.to_string()), Some("x".repeat(5_000))]).collect();
    let set = ResultSet { columns: vec!["id".into(), "big".into()], rows, has_rows: true, ..Default::default() };
    let out = format_sets(&[set]);
    assert!(out.len() <= MAX_TOTAL + 100);
    assert!(out.contains("(truncated)"));
    assert!(!out.contains(&"x".repeat(2_001)));
}

#[test]
fn prompt_names_server_db_tools_and_scope() {
    let tables: Vec<String> = (0..400).map(|i| format!("t{i}")).collect();
    let p = system_prompt(&PromptContext { kind: DbKind::PostgreSql, version: "16.2", db: "shop", schema: "public", tables: &tables });
    for s in ["PostgreSQL", "16.2", "shop", "public", "```dbz", "run_select", "propose_change", "t299", "and 100 more"] {
        assert!(p.contains(s), "missing {s}");
    }
    assert!(!p.contains("t300,"));
}

#[test]
fn old_settings_without_ai_fields_load_defaults() {
    assert_eq!(serde_json::from_str::<AiConfig>("{}").expect("valid"), AiConfig::default());
}

#[test]
fn openai_body_maps_roles() {
    let msgs = [ChatMsg::new(Role::User, "hi"), ChatMsg::new(Role::Assistant, "a"), ChatMsg::new(Role::ToolResult, "rows")];
    let v = request_body("SYS", &msgs, "gpt-x");
    assert_eq!(v["model"], "gpt-x");
    assert_eq!(v["messages"][0]["role"], "system");
    assert_eq!(v["messages"][3]["role"], "user");
    assert_eq!(v["messages"][3]["content"], "[tool result]\nrows");
    assert!(v.get("tools").is_none() && v.get("stream").is_none());
}

#[test]
fn openai_response_and_error() {
    assert_eq!(parse_response(r#"{"choices":[{"message":{"content":"ok"}}]}"#).ok().as_deref(), Some("ok"));
    assert!(matches!(parse_response(r#"{"error":{"message":"bad key"}}"#), Err(AiError::BadResponse(m)) if m.contains("bad key")));
}

#[test]
fn transcript_keeps_fences_and_unicode() {
    let t = transcript(&[ChatMsg::new(Role::User, "kenapa lambat? ```sql\nSELECT 'é'\n```")]);
    assert!(t.contains("```sql\nSELECT 'é'\n```"));
    assert!(t.trim_end().ends_with("Reply as the assistant to the last message."));
}

#[test]
fn cli_args_disable_tools_and_pass_model() {
    let a = args("sonnet", Path::new("C:/x/system.txt"), false);
    assert!(a.contains(&"-p".to_string()) && a.contains(&"--model".to_string()) && a.contains(&"sonnet".to_string()));
    assert!(a.windows(2).any(|w| w[0] == "--tools" && w[1].is_empty()));
    assert!(!args("", Path::new("s"), false).contains(&"--model".to_string()));
}

#[test]
fn cli_output_parsing() {
    assert_eq!(parse_output(r#"{"type":"result","result":"hello","is_error":false}"#).ok().as_deref(), Some("hello"));
    assert!(parse_output(r#"{"result":"limit","is_error":true}"#).is_err());
    assert!(parse_output("not json").is_err());
}

#[test]
fn a_cli_folder_means_the_claude_inside_it() {
    let dir = std::env::temp_dir().join(format!("dbz_ai_cli_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join(if cfg!(windows) { "claude.exe" } else { "claude" });
    std::fs::write(&file, "").expect("fake cli");
    let found = program(dir.to_str().expect("utf-8 path"));
    std::fs::remove_dir_all(&dir).expect("clean up");
    assert_eq!(found, file);
}

fn with_image(text: &str) -> ChatMsg {
    let image = AiImage { name: "shot.png".into(), media_type: "image/png".into(), data: vec![1, 2, 3] };
    ChatMsg { role: Role::User, text: text.into(), images: vec![image] }
}

#[test]
fn only_the_latest_user_message_sends_its_images() {
    let msgs = [with_image("lama"), ChatMsg::new(Role::Assistant, "ok"), with_image("baru")];
    let v = request_body("SYS", &msgs, "m");
    assert!(v["messages"][1]["content"].is_string());
    assert!(v["messages"][1]["content"].as_str().is_some_and(|t| t.contains("not shown again")));
    assert_eq!(v["messages"][3]["content"][1]["image_url"]["url"], "data:image/png;base64,AQID");
}

#[test]
fn cli_streams_images_as_json_lines() {
    let a = args("", Path::new("s"), true);
    assert!(a.windows(2).any(|w| w[0] == "--input-format" && w[1] == "stream-json"));
    assert!(a.contains(&"--verbose".to_string()));
    let msgs = [with_image("lihat")];
    let line: serde_json::Value = serde_json::from_str(stream_input(&transcript(&msgs), &msgs[0].images).trim()).expect("one JSON line");
    assert_eq!(line["type"], "user");
    assert_eq!(line["message"]["content"][1]["source"]["data"], "AQID");
    assert!(line["message"]["content"][0]["text"].as_str().is_some_and(|t| t.contains("shot.png")));
}

#[test]
fn cli_stream_output_reads_the_result_line() {
    let out = "{\"type\":\"system\"}
{\"type\":\"assistant\"}
{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"hai\"}
";
    assert_eq!(parse_stream_output(out).ok().as_deref(), Some("hai"));
    assert!(parse_stream_output("{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"result\":\"\"}").is_err());
}
