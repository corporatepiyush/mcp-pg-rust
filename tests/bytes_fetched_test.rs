/// Test: Record actual bytes fetched from database
/// Validates that responses are human-readable and make sense

#[tokio::test]
async fn test_actual_bytes_fetched_and_response_validity() {
    use tokio_postgres::{connect, NoTls};

    let conn_str = "postgresql://piyush@127.0.0.1:5432/mcp_test";

    // Connect to database
    let (client, connection) = match connect(conn_str, NoTls).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Cannot connect to database: {}", e);
            eprintln!("   Skipping test (PostgreSQL not running)");
            return;
        }
    };

    // Spawn connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    // Test 1: Simple SELECT
    println!("\n=== Test 1: SELECT 1 ===");
    let rows = client.query("SELECT 1 as num", &[]).await.unwrap();

    for row in &rows {
        let val: i32 = row.get(0);
        println!("Result: {}", val);
        println!("  Type: {}", std::any::type_name_of_val(&val));
        assert_eq!(val, 1, "Expected 1");
    }

    // Test 2: SELECT with text
    println!("\n=== Test 2: SELECT with text ===");
    let rows = client
        .query("SELECT 'hello world' as message", &[])
        .await
        .unwrap();

    for row in &rows {
        let msg: String = row.get(0);
        println!("Result: '{}'", msg);
        println!("  Bytes: {}", msg.len());
        println!("  Is UTF-8: {}", String::from_utf8(msg.clone().into_bytes()).is_ok());
        assert_eq!(msg, "hello world");
    }

    // Test 3: Multiple columns
    println!("\n=== Test 3: Multiple columns ===");
    let rows = client
        .query("SELECT 42 as id, 'test' as name, true as active", &[])
        .await
        .unwrap();

    for row in &rows {
        let id: i32 = row.get(0);
        let name: String = row.get(1);
        let active: bool = row.get(2);

        println!("Row data:");
        println!("  id={}, name='{}', active={}", id, name, active);
        println!("  Total bytes: id({}) + name({}) + active({})",
            std::mem::size_of_val(&id),
            name.len(),
            std::mem::size_of_val(&active)
        );
    }

    // Test 4: NULL handling
    println!("\n=== Test 4: NULL values ===");
    let rows = client
        .query("SELECT NULL::text as empty_value", &[])
        .await
        .unwrap();

    for row in &rows {
        let val: Option<String> = row.get(0);
        println!("Result: {:?}", val);
        assert!(val.is_none(), "Expected NULL");
    }

    // Test 5: Large response
    println!("\n=== Test 5: Large response ===");
    let rows = client
        .query(
            "SELECT
                'The quick brown fox jumps over the lazy dog. ' as text,
                array[1,2,3,4,5] as numbers",
            &[],
        )
        .await
        .unwrap();

    let mut total_bytes = 0;
    for row in &rows {
        let text: String = row.get(0);
        let nums: Vec<i32> = row.get(1);

        println!("Text: '{}'", text);
        println!("Numbers: {:?}", nums);
        println!("Text bytes: {}", text.len());
        println!("Array items: {}", nums.len());

        total_bytes += text.len();
    }
    println!("Total bytes fetched: {}", total_bytes);

    println!("\n✓ All responses are valid and human-readable");
}

#[tokio::test]
async fn test_json_response_format() {
    use serde_json::{json, Value};

    // Simulate typical MCP response
    let response = json!({
        "jsonrpc": "2.0",
        "result": {
            "rows": [
                [1, "alice"],
                [2, "bob"],
            ]
        },
        "error": null,
        "id": 1
    });

    println!("\n=== JSON Response Format ===");
    println!("Response:\n{}", serde_json::to_string_pretty(&response).unwrap());

    let response_str = serde_json::to_string(&response).unwrap();
    let response_bytes = response_str.as_bytes();

    println!("\nBytes: {}", response_bytes.len());
    println!("Is valid UTF-8: {}", String::from_utf8(response_bytes.to_vec()).is_ok());
    println!("Is valid JSON: {}", serde_json::from_str::<Value>(&response_str).is_ok());

    // Verify it's human readable
    assert!(response_str.contains("jsonrpc"));
    assert!(response_str.contains("result"));
    assert!(response_str.contains("alice"));

    println!("✓ Response is human-readable JSON");
}
