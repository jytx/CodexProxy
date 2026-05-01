use crate::types::*;
use serde_json::{json, Value};

/// 将 Responses API 请求转换为 Chat Completions API 请求
/// model_override: 非空时替换请求中的模型名
pub fn responses_to_chat(resp_req: &ResponsesRequest, model_override: &str) -> Value {
    let model_name = if model_override.is_empty() {
        resp_req.model.clone()
    } else {
        model_override.to_string()
    };

    let mut chat_req = json!({
        "model": model_name,
        "stream": resp_req.stream.unwrap_or(false),
    });

    let mut messages = Vec::new();

    // instructions → 作为 user 消息（部分国内模型不支持 system 角色）
    if let Some(ref instructions) = resp_req.instructions {
        if !instructions.is_empty() {
            messages.push(json!({
                "role": "user",
                "content": format!("[System Instructions]\n{}", instructions),
            }));
        }
    }

    // 解析 input
    if let Some(input_str) = resp_req.input.as_str() {
        messages.push(json!({ "role": "user", "content": input_str }));
    } else if let Some(input_arr) = resp_req.input.as_array() {
        for raw_msg in input_arr {
            let im: ResponsesInputMessage = match serde_json::from_value(raw_msg.clone()) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let msg_type = im.msg_type.as_deref().unwrap_or("");

            match msg_type {
                "function_call_output" => {
                    messages.push(json!({
                        "role": "tool",
                        "content": im.output.unwrap_or_default(),
                        "tool_call_id": im.call_id.unwrap_or_default(),
                    }));
                }
                "function_call" => {
                    messages.push(json!({
                        "role": "assistant",
                        "tool_calls": [{
                            "id": im.call_id.clone().unwrap_or_default(),
                            "type": "function",
                            "function": {
                                "name": im.name.clone().unwrap_or_default(),
                                "arguments": im.arguments.clone().unwrap_or_default(),
                            }
                        }]
                    }));
                }
                _ => {
                    // 根据 role 处理 message 类型
                    // Codex Responses API 使用 "developer" 角色，国内模型不支持，映射为 "system"
                    let raw_role = im.role.as_deref().unwrap_or("user");
                    let role = match raw_role {
                        "developer" | "system" => "user",
                        other => other,
                    };
                    let content = convert_content_to_chat(im.content.as_ref());
                    let mut msg = json!({ "role": role });
                    if let Some(c) = content {
                        msg["content"] = c;
                    }
                    messages.push(msg);
                }
            }
        }
    }

    chat_req["messages"] = json!(messages);

    // 参数映射
    if let Some(v) = resp_req.max_output_tokens {
        chat_req["max_completion_tokens"] = json!(v);
    }
    if let Some(v) = resp_req.temperature {
        chat_req["temperature"] = json!(v);
    }
    if let Some(v) = resp_req.top_p {
        chat_req["top_p"] = json!(v);
    }
    if let Some(v) = resp_req.frequency_penalty {
        chat_req["frequency_penalty"] = json!(v);
    }
    if let Some(v) = resp_req.presence_penalty {
        chat_req["presence_penalty"] = json!(v);
    }
    if let Some(ref v) = resp_req.reasoning {
        if let Some(effort) = v.get("effort").and_then(|e| e.as_str()) {
            chat_req["reasoning_effort"] = json!(effort);
        }
    }
    if let Some(ref v) = resp_req.text {
        if let Some(format) = v.get("format") {
            chat_req["response_format"] = convert_text_to_response_format(format);
        }
    }
    if let Some(v) = resp_req.parallel_tool_calls {
        chat_req["parallel_tool_calls"] = json!(v);
    }
    if let Some(ref v) = resp_req.tools {
        chat_req["tools"] = convert_responses_tools(v);
    }
    if let Some(ref v) = resp_req.tool_choice {
        chat_req["tool_choice"] = v.clone();
    }
    if let Some(ref v) = resp_req.user {
        chat_req["user"] = json!(v);
    }
    if resp_req.stream.unwrap_or(false) {
        chat_req["stream_options"] = json!({ "include_usage": true });
    }

    chat_req
}

/// 将 Chat Completions 响应转换为 Responses API 响应
pub fn chat_to_responses(chat_resp: &ChatCompletionsResponse, model: &str) -> Value {
    let mut output = Vec::new();
    let mut status = "completed";

    for choice in &chat_resp.choices {
        let msg = match &choice.message {
            Some(m) => m,
            None => continue,
        };

        if let Some(ref fr) = choice.finish_reason {
            if fr == "length" {
                status = "incomplete";
            }
        }

        // 文本消息
        let text = content_to_string(msg.content.as_ref());
        if !text.is_empty() || msg.tool_calls.is_none() {
            let msg_id = format!("msg_{}", chrono::Utc::now().timestamp_millis());
            output.push(json!({
                "id": msg_id,
                "type": "message",
                "status": "completed",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": text,
                    "annotations": [],
                }]
            }));
        }

        // 工具调用
        if let Some(ref tool_calls) = msg.tool_calls {
            for tc in tool_calls {
                let id = tc.id.clone().unwrap_or_default();
                output.push(json!({
                    "id": id,
                    "type": "function_call",
                    "status": "completed",
                    "name": tc.function.name.clone().unwrap_or_default(),
                    "arguments": tc.function.arguments.clone().unwrap_or_default(),
                    "call_id": id,
                }));
            }
        }
    }

    let mut result = json!({
        "id": format!("resp_{}", chrono::Utc::now().timestamp_millis()),
        "object": "response",
        "created_at": chat_resp.created,
        "status": status,
        "model": model,
        "output": output,
    });

    if let Some(ref usage) = chat_resp.usage {
        result["usage"] = json!({
            "input_tokens": usage.prompt_tokens,
            "output_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
        });
    }

    if status == "incomplete" {
        result["incomplete_details"] = json!({ "reason": "max_output_tokens" });
    }

    result
}

/// 将 content 从 Responses API 格式转为 Chat Completions 格式
fn convert_content_to_chat(content: Option<&Value>) -> Option<Value> {
    let content = content?;

    // 字符串直接返回
    if content.is_string() {
        return Some(content.clone());
    }

    // 数组：转换类型
    if let Some(arr) = content.as_array() {
        let mut texts = Vec::new();
        let mut all_text = true;

        for part in arr {
            let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let text = part.get("text").and_then(|t| t.as_str()).unwrap_or("");

            match part_type {
                "input_text" | "output_text" | "text" => {
                    texts.push(text.to_string());
                }
                "input_image" => {
                    all_text = false;
                }
                _ => {
                    all_text = false;
                }
            }
        }

        // 如果只有纯文本，直接拼接返回字符串
        if all_text && !texts.is_empty() {
            return Some(json!(texts.join("")));
        }

        // 有混合类型，返回数组
        if !texts.is_empty() {
            return Some(json!(texts));
        }
    }

    Some(content.clone())
}

/// content 值转字符串
fn content_to_string(content: Option<&Value>) -> String {
    let content = match content {
        Some(c) => c,
        None => return String::new(),
    };

    if let Some(s) = content.as_str() {
        return s.to_string();
    }

    if let Some(arr) = content.as_array() {
        let mut result = String::new();
        for part in arr {
            let t = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if t == "text" || t == "output_text" || t == "input_text" {
                if let Some(s) = part.get("text").and_then(|v| v.as_str()) {
                    result.push_str(s);
                }
            }
        }
        return result;
    }

    content.to_string()
}

/// 将 Responses tools 转为 Chat Completions tools 格式
/// 保留 function 类型工具（转换格式），其他类型工具（如 computer_use_preview）透传
fn convert_responses_tools(tools: &Value) -> Value {
    let Some(arr) = tools.as_array() else {
        return tools.clone();
    };

    let chat_tools: Vec<Value> = arr
        .iter()
        .map(|t| {
            let tool_type = t.get("type").and_then(|v| v.as_str()).unwrap_or("function");

            match tool_type {
                "function" => {
                    // Responses API 的 function 工具 → Chat Completions 格式
                    let mut func = json!({ "name": t["name"] });
                    if let Some(desc) = t.get("description") {
                        func["description"] = desc.clone();
                    }
                    if let Some(params) = t.get("parameters") {
                        func["parameters"] = params.clone();
                    }
                    if let Some(strict) = t.get("strict") {
                        func["strict"] = strict.clone();
                    }
                    json!({ "type": "function", "function": func })
                }
                _ => {
                    // 其他类型（computer_use_preview 等）直接透传
                    t.clone()
                }
            }
        })
        .collect();

    json!(chat_tools)
}

/// 将 text.format 转为 response_format
fn convert_text_to_response_format(format: &Value) -> Value {
    let fmt_type = format.get("type").and_then(|t| t.as_str()).unwrap_or("text");
    match fmt_type {
        "json_object" => json!({ "type": "json_object" }),
        "json_schema" => {
            let mut result = json!({ "type": "json_schema", "json_schema": {} });
            let js = result.get_mut("json_schema").unwrap();
            if let Some(name) = format.get("name") {
                js["name"] = name.clone();
            }
            if let Some(desc) = format.get("description") {
                js["description"] = desc.clone();
            }
            if let Some(schema) = format.get("schema") {
                js["schema"] = schema.clone();
            }
            if let Some(strict) = format.get("strict") {
                js["strict"] = strict.clone();
            }
            result
        }
        _ => json!({ "type": "text" }),
    }
}
