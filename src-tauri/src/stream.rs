/// 流式 SSE 处理模块
/// 将上游 Chat Completions SSE 流转换为 Responses API SSE 流
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use futures::stream::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

/// HTTP 错误响应（供流式模块使用）
fn error_response(status: StatusCode, msg: &str) -> axum::response::Response {
    (
        status,
        [("content-type", "application/json")],
        json!({ "error": { "message": msg } }).to_string(),
    )
        .into_response()
}

/// 流式处理中的工具调用累积状态
struct StreamToolCalls {
    /// (chat_index, output_index, id, name, arguments)
    calls: Vec<(usize, usize, String, String, String)>,
    /// 下一个 output_index（0 是文本消息，工具调用从 1 开始）
    next_output_idx: usize,
}

impl StreamToolCalls {
    fn new() -> Self {
        Self { calls: Vec::new(), next_output_idx: 1 }
    }

    /// 处理一个 tool_call delta，返回需要发射的 Responses API 事件
    fn process_delta(
        &mut self,
        tc_index: usize,
        tc_id: &str,
        tc_name: &str,
        tc_args: &str,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let pos = self.calls.iter().position(|(idx, _, _, _, _)| *idx == tc_index);

        match pos {
            Some(p) => {
                // 已有工具调用，追加 arguments
                self.calls[p].4.push_str(tc_args);
                if !tc_args.is_empty() {
                    events.push(Event::default().data(json!({
                        "type": "response.function_call_arguments.delta",
                        "item_id": self.calls[p].2,
                        "output_index": self.calls[p].1,
                        "call_id": self.calls[p].2,
                        "delta": tc_args,
                    }).to_string()));
                }
            }
            None => {
                let output_idx = self.next_output_idx;
                self.next_output_idx += 1;

                // 发射 output_item.added（新工具调用）
                events.push(Event::default().data(json!({
                    "type": "response.output_item.added",
                    "output_index": output_idx,
                    "item": {
                        "id": tc_id,
                        "type": "function_call",
                        "status": "in_progress",
                        "name": tc_name,
                        "call_id": tc_id,
                    }
                }).to_string()));

                // 发射 arguments delta
                if !tc_args.is_empty() {
                    events.push(Event::default().data(json!({
                        "type": "response.function_call_arguments.delta",
                        "item_id": tc_id,
                        "output_index": output_idx,
                        "call_id": tc_id,
                        "delta": tc_args,
                    }).to_string()));
                }

                self.calls.push((tc_index, output_idx, tc_id.to_string(), tc_name.to_string(), tc_args.to_string()));
            }
        }
        events
    }
}

/// 流式：协议转换请求 → 上游 SSE → 转回 SSE（支持文本 + 工具调用）
pub async fn handle_responses_stream(
    client: &Client,
    url: &str,
    api_key: &str,
    chat_body: serde_json::Value,
    model: &str,
) -> axum::response::Response {
    let resp = match client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&chat_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_GATEWAY, &format!("上游错误: {}", e));
        }
    };

    let status = resp.status();
    let axum_status = StatusCode::from_u16(status.as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);

    if axum_status != StatusCode::OK {
        let body = resp.bytes().await.unwrap_or_default();
        return (axum_status, [("content-type", "application/json")], body.to_vec()).into_response();
    }

    let response_id = format!("resp_{}", chrono::Utc::now().timestamp_millis());
    let msg_id = format!("msg_{}", chrono::Utc::now().timestamp_millis());
    let created = chrono::Utc::now().timestamp();
    let model = model.to_string();

    // 文本累积状态
    let full_text_shared: Arc<StdMutex<String>> = Arc::new(StdMutex::new(String::new()));
    let full_text_for_scan = full_text_shared.clone();
    let full_text_for_done = full_text_shared.clone();

    // 工具调用累积状态
    let tc_state: Arc<StdMutex<StreamToolCalls>> = Arc::new(StdMutex::new(StreamToolCalls::new()));
    let tc_state_scan = tc_state.clone();
    let tc_state_done = tc_state.clone();

    // === 初始化事件 ===
    let initial_events: Vec<Result<Event, std::convert::Infallible>> = vec![
        Ok(Event::default().data(json!({
            "type": "response.created",
            "response": {
                "id": response_id,
                "object": "response",
                "created_at": created,
                "status": "in_progress",
                "model": model,
                "output": [],
            }
        }).to_string())),
        Ok(Event::default().data(json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": {
                "id": msg_id,
                "type": "message",
                "status": "in_progress",
                "role": "assistant",
                "content": [],
            }
        }).to_string())),
        Ok(Event::default().data(json!({
            "type": "response.content_part.added",
            "item_id": msg_id,
            "output_index": 0,
            "content_index": 0,
            "part": { "type": "output_text", "text": "" }
        }).to_string())),
    ];

    let msg_id_scan = msg_id.clone();
    let msg_id_done = msg_id.clone();
    let response_id_done = response_id.clone();

    // === 处理上游 SSE 数据块 ===
    let stream = resp.bytes_stream();
    let delta_stream = stream
        .scan(full_text_for_scan, move |shared_text, chunk| {
            let chunk = match chunk {
                Ok(c) => c,
                Err(_) => return futures::future::ready(Some(Vec::new())),
            };
            let text = String::from_utf8_lossy(&chunk).to_string();

            let mut events: Vec<Result<Event, std::convert::Infallible>> = Vec::new();
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" { continue; }

                    if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                            for choice in choices {
                                if let Some(delta) = choice.get("delta") {
                                    // 文本内容 delta
                                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                        if let Ok(mut t) = shared_text.lock() {
                                            t.push_str(content);
                                        }
                                        events.push(Ok(Event::default().data(json!({
                                            "type": "response.output_text.delta",
                                            "item_id": msg_id_scan,
                                            "output_index": 0,
                                            "content_index": 0,
                                            "delta": content,
                                        }).to_string())));
                                    }

                                    // 工具调用 delta
                                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                        if let Ok(mut tc) = tc_state_scan.lock() {
                                            for tc_delta in tool_calls {
                                                let idx = tc_delta.get("index")
                                                    .and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                                                let id = tc_delta.get("id")
                                                    .and_then(|i| i.as_str()).unwrap_or("").to_string();
                                                let name = tc_delta.get("function")
                                                    .and_then(|f| f.get("name"))
                                                    .and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                let args = tc_delta.get("function")
                                                    .and_then(|f| f.get("arguments"))
                                                    .and_then(|a| a.as_str()).unwrap_or("").to_string();

                                                let tc_events = tc.process_delta(idx, &id, &name, &args);
                                                events.extend(tc_events.into_iter().map(Ok));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            futures::future::ready(Some(events))
        })
        .flat_map(futures::stream::iter);

    // === 结束事件 ===
    let done_event = futures::stream::once(async move {
        let full_text = full_text_for_done
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        let tool_calls = tc_state_done.lock()
            .map(|g| g.calls.clone())
            .unwrap_or_default();

        let mut events: Vec<Result<Event, std::convert::Infallible>> = vec![
            Ok(Event::default().data(json!({
                "type": "response.output_text.done",
                "item_id": msg_id_done,
                "output_index": 0,
                "content_index": 0,
                "text": full_text,
            }).to_string())),
            Ok(Event::default().data(json!({
                "type": "response.content_part.done",
                "item_id": msg_id_done,
                "output_index": 0,
                "content_index": 0,
                "part": { "type": "output_text", "text": full_text }
            }).to_string())),
            Ok(Event::default().data(json!({
                "type": "response.output_item.done",
                "output_index": 0,
                "item": {
                    "id": msg_id_done,
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": full_text, "annotations": [] }]
                }
            }).to_string())),
        ];

        // 工具调用完成事件
        for (_, output_idx, id, name, args) in &tool_calls {
            events.push(Ok(Event::default().data(json!({
                "type": "response.function_call_arguments.done",
                "item_id": id,
                "output_index": output_idx,
                "call_id": id,
                "arguments": args,
            }).to_string())));
            events.push(Ok(Event::default().data(json!({
                "type": "response.output_item.done",
                "output_index": output_idx,
                "item": {
                    "id": id,
                    "type": "function_call",
                    "status": "completed",
                    "name": name,
                    "call_id": id,
                    "arguments": args,
                }
            }).to_string())));
        }

        // 构造 response.completed
        let mut output = vec![
            json!({
                "id": msg_id_done,
                "type": "message",
                "status": "completed",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": full_text, "annotations": [] }]
            })
        ];
        for (_, _, id, name, args) in &tool_calls {
            output.push(json!({
                "id": id,
                "type": "function_call",
                "status": "completed",
                "name": name,
                "call_id": id,
                "arguments": args,
            }));
        }

        events.push(Ok(Event::default().data(json!({
            "type": "response.completed",
            "response": {
                "id": response_id_done,
                "object": "response",
                "created_at": created,
                "status": "completed",
                "model": model,
                "output": output,
            },
            "sequence_number": 999,
        }).to_string())));

        futures::stream::iter(events)
    })
    .flatten();

    // 拼接：初始化事件 → delta 事件 → 完成事件
    let full_stream = futures::stream::iter(initial_events)
        .chain(delta_stream)
        .chain(done_event);

    Sse::new(full_stream.boxed())
        .keep_alive(KeepAlive::default())
        .into_response()
}
