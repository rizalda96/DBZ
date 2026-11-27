use super::{AiChat, ChatItem, Step, MAX_ROUNDS};
use crate::domain::{AiImage, AiTool, DbRead, Role};

#[test]
fn answer_ends_turn() {
    let mut c = AiChat::default();
    let t = c.begin_turn("hai".into());
    assert_eq!(c.accept(t, Ok("Halo".into())), Step::Done);
    assert!(c.busy.is_none());
    assert!(matches!(c.items.last(), Some(ChatItem::Assistant(_))));
}

#[test]
fn tool_call_keeps_turn_busy_with_its_label() {
    let mut c = AiChat::default();
    let t = c.begin_turn("x".into());
    let step = c.accept(t, Ok("```dbz\n{\"tool\":\"explain\",\"sql\":\"SELECT 1\"}\n```".into()));
    assert!(matches!(step, Step::Run(AiTool::Db(DbRead::Explain { .. }))));
    assert_eq!(c.busy.as_ref().map(|b| b.label.as_str()), Some("Running EXPLAIN…"));
}

#[test]
fn stale_reply_is_ignored() {
    let mut c = AiChat::default();
    let t = c.begin_turn("x".into());
    c.stop();
    assert_eq!(c.accept(t, Ok("late".into())), Step::Stale);
    let t2 = c.begin_turn("y".into());
    c.reset();
    assert_eq!(c.accept(t2, Ok("late".into())), Step::Stale);
}

#[test]
fn rounds_are_limited() {
    let mut c = AiChat::default();
    let t = c.begin_turn("x".into());
    let call = || Ok("```dbz\n{\"tool\":\"list_objects\"}\n```".to_string());
    for _ in 0..MAX_ROUNDS {
        assert!(matches!(c.accept(t, call()), Step::Run(_)));
    }
    assert_eq!(c.accept(t, call()), Step::LimitReached);
    assert!(c.busy.is_none());
}

#[test]
fn declined_change_continues_turn() {
    let mut c = AiChat::default();
    c.begin_turn("buat index".into());
    c.tool_result("Proposed change".into(), Err("user rejected the change".into()));
    assert_eq!(c.history().last().map(|m| m.role), Some(Role::ToolResult));
    assert!(c.history().last().is_some_and(|m| m.text.contains("rejected")));
    assert!(c.busy.is_some());
}

#[test]
fn invalid_call_is_reported_back_to_ai() {
    let mut c = AiChat::default();
    let t = c.begin_turn("x".into());
    assert_eq!(c.accept(t, Ok("```dbz\nnot json\n```".into())), Step::Retry);
    assert!(c.history().last().is_some_and(|m| m.role == Role::ToolResult && m.text.starts_with("invalid tool call:")));
}

#[test]
fn ai_error_ends_turn_with_message() {
    let mut c = AiChat::default();
    let t = c.begin_turn("x".into());
    assert_eq!(c.accept(t, Err(crate::ai::AiError::Timeout)), Step::Done);
    assert!(c.busy.is_none());
    assert!(matches!(c.items.last(), Some(ChatItem::Error(_))));
}

fn png(name: &str) -> AiImage {
    AiImage { name: name.into(), media_type: "image/png".into(), data: vec![1, 2, 3] }
}

#[test]
fn attachments_go_with_the_next_message_only() {
    let mut c = AiChat::default();
    c.attach(png("a.png")).expect("room");
    c.begin_turn("lihat ini".into());
    assert_eq!(c.history()[0].images.len(), 1);
    assert!(c.attachments.is_empty());
    assert!(matches!(&c.items[0], ChatItem::User { images, .. } if images == &["a.png".to_string()]));
}

#[test]
fn attachments_are_limited() {
    let mut c = AiChat::default();
    for i in 0..super::MAX_IMAGES {
        c.attach(png(&format!("{i}.png"))).expect("room");
    }
    assert!(c.attach(png("one-too-many.png")).is_err());
}
