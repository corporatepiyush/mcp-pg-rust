use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn send_request(stdin: &mut dyn Write, method: &str, params: Option<Value>, id: u64) {
    let request = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params.unwrap_or(json!({})),
        "id": id
    });
    let mut req_str = serde_json::to_string(&request).unwrap();
    req_str.push('\n');
    stdin.write_all(req_str.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

fn send_notification(stdin: &mut dyn Write, method: &str, params: Option<Value>) {
    let mut req = json!({ "jsonrpc": "2.0", "method": method });
    if let Some(p) = params {
        req["params"] = p;
    }
    let mut req_str = serde_json::to_string(&req).unwrap();
    req_str.push('\n');
    stdin.write_all(req_str.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

fn read_response(reader: &mut dyn BufRead) -> Value {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line.trim()).unwrap()
}

fn assert_success(resp: &Value, id: u64) {
    assert!(resp.get("result").is_some(), "Response {id} missing result: {resp}");
    assert!(resp.get("error").is_none() || resp["error"].is_null(), "Response {id} has error: {resp}");
    assert_eq!(resp["id"], id, "Response {id} id mismatch");
}

fn assert_error(resp: &Value, id: u64) {
    assert!(resp.get("error").is_some(), "Response {id} missing error: {resp}");
    let e = &resp["error"];
    assert!(!e.is_null(), "Response {id} has null error");
    assert!(e.get("code").is_some(), "Response {id} error missing code");
    assert!(e.get("message").is_some(), "Response {id} error missing message");
    assert_eq!(resp["id"], id, "Response {id} id mismatch");
}

fn assert_clean(resp: &Value) {
    // No 'error' key when successful, no 'result' key on error
    if resp.get("result").is_some() {
        assert!(resp.get("error").is_none(),
            "Response has both result and error key: {resp}");
    }
    if resp.get("error").is_some() {
        assert!(resp.get("result").is_none(),
            "Response has both error and result key: {resp}");
    }
}

fn spawn_stdio() -> (std::process::Child, Box<dyn Write>, Box<dyn BufRead>) {
    let binary = if cfg!(debug_assertions) {
        "./target/debug/mcp-postgres"
    } else {
        "./target/release/mcp-postgres"
    };

    let mut child = Command::new(binary)
        .arg("--stdio")
        .arg("--database-url")
        .arg("postgresql://piyush@localhost:5432/mcp_test_31")
        .arg("--log-level")
        .arg("error")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn mcp-postgres");

    let stdin: Box<dyn Write> = Box::new(child.stdin.take().unwrap());
    let stdout: Box<dyn BufRead> = Box::new(BufReader::new(child.stdout.take().unwrap()));
    (child, stdin, stdout)
}

#[test]
fn test_stdio_full_workflow() {
    let (mut child, mut stdin, mut stdout) = spawn_stdio();

    // 1. Notification (no response expected)
    send_notification(&mut *stdin, "notifications/initialized", None);

    // 2. Initialize
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 1);
    assert_clean(&resp);
    assert_eq!(resp["result"]["serverInfo"]["name"], "mcp-postgres");

    // 3. Ping
    send_request(&mut *stdin, "ping", None, 2);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 2);
    assert_clean(&resp);
    assert!(resp["result"].is_null());

    // 4. Tools list
    send_request(&mut *stdin, "tools/list", None, 3);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 3);
    assert_clean(&resp);
    assert!(!resp["result"]["tools"].as_array().unwrap().is_empty());

    // 5. Create table
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"create_table","arguments":{"table":"stdio_test","columns":["id SERIAL PRIMARY KEY","name TEXT","val INTEGER"]}})), 4);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 4);
    assert_clean(&resp);

    // 6. Insert
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"execute_insert","arguments":{"sql":"INSERT INTO stdio_test (name, val) VALUES ('hello', 42)"}})), 5);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 5);
    assert_clean(&resp);

    // 7. Query
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"execute_query","arguments":{"sql":"SELECT * FROM stdio_test ORDER BY id"}})), 6);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 6);
    assert_clean(&resp);
    assert_eq!(resp["result"]["rows"].as_array().unwrap().len(), 1);

    // 8. List tables
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"list_tables","arguments":{}})), 7);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 7);
    assert_clean(&resp);

    // 9. Error: bad table
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"execute_query","arguments":{"sql":"SELECT * FROM nonexistent"}})), 8);
    let resp = read_response(&mut *stdout);
    assert_error(&resp, 8);
    assert_clean(&resp);

    // 10. Error: bad tool
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"no_such_tool","arguments":{}})), 9);
    let resp = read_response(&mut *stdout);
    assert_error(&resp, 9);
    assert_clean(&resp);

    // 11. Update
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"execute_update","arguments":{"sql":"UPDATE stdio_test SET val = 99 WHERE name = 'hello'"}})), 10);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 10);

    // 12. Verify update
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"execute_query","arguments":{"sql":"SELECT val FROM stdio_test WHERE name = 'hello'"}})), 11);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 11);
    assert_eq!(resp["result"]["rows"][0][0], 99);

    // 13. Delete
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"execute_delete","arguments":{"sql":"DELETE FROM stdio_test WHERE name = 'hello'"}})), 12);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 12);

    // 14. Drop table
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"drop_table","arguments":{"table":"stdio_test","if_exists":true}})), 13);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 13);

    // 15. Show current user
    send_request(&mut *stdin, "tools/call",
        Some(json!({"name":"show_current_user","arguments":{}})), 14);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 14);

    // Close and verify clean exit
    drop(stdin);
    let status = child.wait().expect("Failed to wait for child");
    assert!(status.success(), "Process exited with: {status}");
}

#[test]
fn test_stdio_notification_no_response() {
    let (mut child, mut stdin, mut stdout) = spawn_stdio();

    // Send notification - should NOT produce a response
    send_notification(&mut *stdin, "notifications/initialized", None);

    // Send initialize - should produce exactly one response (the notification generates none)
    send_request(&mut *stdin, "initialize",
        Some(json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}})), 1);
    let resp = read_response(&mut *stdout);
    assert_success(&resp, 1);

    drop(stdin);
    let _ = child.wait();
}
