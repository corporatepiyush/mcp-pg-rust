/// Data generator for test schema
/// Generates realistic test data across 12 tables with thousands of records
/// Run: cargo run --release --bin load_test_data -- --database-url "postgres://..."
use tokio_postgres::{NoTls, connect};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());

    let (client, connection) = connect(&db_url, NoTls).await?;

    // Run connection in background
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    println!("🚀 Starting test data generation...\n");

    // Generate customers
    println!("📝 Generating 500 customers...");
    generate_customers(&client, 500).await?;

    // Generate categories
    println!("📝 Generating 15 product categories...");
    generate_categories(&client, 15).await?;

    // Generate products
    println!("📝 Generating 200 products...");
    generate_products(&client, 200).await?;

    // Generate accounts
    println!("📝 Generating 400 accounts...");
    generate_accounts(&client, 400).await?;

    // Generate inventory
    println!("📝 Generating 200 inventory records...");
    generate_inventory(&client, 200).await?;

    // Generate orders
    println!("📝 Generating 1000 orders...");
    generate_orders(&client, 1000).await?;

    // Generate order items
    println!("📝 Generating 3000 order items...");
    generate_order_items(&client, 3000).await?;

    // Generate invoices
    println!("📝 Generating 1000 invoices...");
    generate_invoices(&client, 1000).await?;

    // Generate payments
    println!("📝 Generating 800 payments...");
    generate_payments(&client, 800).await?;

    // Generate subscriptions
    println!("📝 Generating 300 subscriptions...");
    generate_subscriptions(&client, 300).await?;

    // Generate transactions
    println!("📝 Generating 2000 transactions...");
    generate_transactions(&client, 2000).await?;

    // Generate audit logs
    println!("📝 Generating 500 audit logs...");
    generate_audit_logs(&client, 500).await?;

    // Print summary
    println!("\n✅ Data generation complete!\n");
    print_summary(&client).await?;

    Ok(())
}

async fn generate_customers(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..count {
        let email = format!("user{}@example.com", i);
        let first_name = format!("User{}", i);
        let last_name = format!("Test{}", i);
        let phone = format!("+1-555-{:04}-{:04}", i % 10000, (i * 13) % 10000);
        let status = ["active", "inactive", "suspended"][i as usize % 3];

        client
            .execute(
                "INSERT INTO customers (email, first_name, last_name, phone, status) VALUES ($1, $2, $3, $4, $5)",
                &[&email, &first_name, &last_name, &phone, &status],
            )
            .await?;
    }
    Ok(())
}

async fn generate_categories(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let category_names = vec![
        "Electronics",
        "Books",
        "Clothing",
        "Home & Garden",
        "Sports",
        "Toys",
        "Beauty",
        "Food & Beverage",
        "Furniture",
        "Automotive",
        "Pet Supplies",
        "Office Supplies",
        "Tools",
        "Music",
        "Video Games",
    ];

    for (_i, name) in category_names.iter().enumerate().take(count as usize) {
        let description = format!("Category for {}", name);
        client
            .execute(
                "INSERT INTO categories (name, description) VALUES ($1, $2)",
                &[&name, &description],
            )
            .await?;
    }
    Ok(())
}

async fn generate_products(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..count {
        let category_id = (i % 15) + 1;
        let name = format!("Product {}", i);
        let description = format!("High-quality product #{} with excellent features", i);
        let price = (10.0 + (f64::from(i) * std::f64::consts::PI) % 490.0).round() / 100.0;
        let stock = (i * 7) % 1000;

        client
            .execute(
                "INSERT INTO products (category_id, name, description, price, stock_quantity) VALUES ($1, $2, $3, $4, $5)",
                &[&category_id, &name, &description, &price, &stock],
            )
            .await?;
    }
    Ok(())
}

async fn generate_accounts(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let account_types = ["checking", "savings", "credit"];
    for i in 0..count {
        let customer_id = (i % 500) + 1;
        let account_type = account_types[i as usize % 3];
        let balance = ((f64::from(i) * 123.45) % 50000.0).round() / 100.0;

        client
            .execute(
                "INSERT INTO accounts (customer_id, account_type, balance) VALUES ($1, $2, $3)",
                &[&customer_id, &account_type, &balance],
            )
            .await?;
    }
    Ok(())
}

async fn generate_inventory(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let locations = [
        "Warehouse A",
        "Warehouse B",
        "Warehouse C",
        "Store 1",
        "Store 2",
    ];
    for i in 0..count {
        let product_id = (i % 200) + 1;
        let location = locations[i as usize % locations.len()];
        let quantity = (i * 11) % 500;

        client
            .execute(
                "INSERT INTO inventory (product_id, warehouse_location, quantity) VALUES ($1, $2, $3)",
                &[&product_id, &location, &quantity],
            )
            .await?;
    }
    Ok(())
}

async fn generate_orders(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let statuses = ["pending", "processing", "shipped", "delivered", "cancelled"];
    for i in 0..count {
        let customer_id = (i % 500) + 1;
        let total = ((f64::from(i) * 47.89) % 5000.0).round() / 100.0;
        let status = statuses[i as usize % statuses.len()];
        let address = format!("{} Main St, City {}", i, i % 100);

        client
            .execute(
                "INSERT INTO orders (customer_id, total_amount, status, shipping_address) VALUES ($1, $2, $3, $4)",
                &[&customer_id, &total, &status, &address],
            )
            .await?;
    }
    Ok(())
}

async fn generate_order_items(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..count {
        let order_id = (i / 3) + 1;
        let product_id = (i % 200) + 1;
        let quantity = (i % 10) + 1;
        let unit_price = ((f64::from(i) * 19.99) % 499.0).round() / 100.0;

        client
            .execute(
                "INSERT INTO order_items (order_id, product_id, quantity, unit_price) VALUES ($1, $2, $3, $4)",
                &[&order_id, &product_id, &quantity, &unit_price],
            )
            .await?;
    }
    Ok(())
}

async fn generate_invoices(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let statuses = ["unpaid", "paid", "overdue", "partial"];
    for i in 0..count {
        let order_id = i + 1;
        let invoice_number = format!("INV-{:06}", i);
        let amount = ((f64::from(i) * 123.45) % 10000.0).round() / 100.0;
        let status = statuses[i as usize % statuses.len()];

        client
            .execute(
                "INSERT INTO invoices (order_id, invoice_number, amount_due, status) VALUES ($1, $2, $3, $4)",
                &[&order_id, &invoice_number, &amount, &status],
            )
            .await?;
    }
    Ok(())
}

async fn generate_payments(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let methods = ["credit_card", "bank_transfer", "check", "cash"];
    for i in 0..count {
        let invoice_id = ((i * 7) % 1000) + 1;
        let amount = ((f64::from(i) * 89.99) % 5000.0).round() / 100.0;
        let method = methods[i as usize % methods.len()];

        client
            .execute(
                "INSERT INTO payments (invoice_id, amount, payment_method) VALUES ($1, $2, $3)",
                &[&invoice_id, &amount, &method],
            )
            .await?;
    }
    Ok(())
}

async fn generate_subscriptions(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let plans = ["basic", "premium", "enterprise"];
    for i in 0..count {
        let customer_id = (i % 500) + 1;
        let plan = plans[i as usize % plans.len()];
        let auto_renew = i % 2 == 0;

        client
            .execute(
                "INSERT INTO subscriptions (customer_id, plan_type, start_date, status, auto_renew) VALUES ($1, $2, $3, $4, $5)",
                &[&customer_id, &plan, &"2024-01-01", &"active", &auto_renew],
            )
            .await?;
    }
    Ok(())
}

async fn generate_transactions(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let types = ["deposit", "withdrawal", "transfer", "fee"];
    for i in 0..count {
        let account_id = ((i * 3) % 400) + 1;
        let tx_type = types[i as usize % types.len()];
        let amount = ((f64::from(i) * 45.67) % 2000.0).round() / 100.0;
        let description = format!("Transaction #{}", i);

        client
            .execute(
                "INSERT INTO transactions (account_id, transaction_type, amount, description) VALUES ($1, $2, $3, $4)",
                &[&account_id, &tx_type, &amount, &description],
            )
            .await?;
    }
    Ok(())
}

async fn generate_audit_logs(
    client: &tokio_postgres::Client,
    count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let operations = ["INSERT", "UPDATE", "DELETE"];
    let tables = ["customers", "orders", "products", "invoices", "payments"];

    for i in 0..count {
        let table = tables[i as usize % tables.len()];
        let operation = operations[i as usize % operations.len()];
        let user_id = (i % 100) + 1;

        client
            .execute(
                "INSERT INTO audit_logs (table_name, operation, user_id) VALUES ($1, $2, $3)",
                &[&table, &operation, &user_id],
            )
            .await?;
    }
    Ok(())
}

async fn print_summary(client: &tokio_postgres::Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Data Summary:");
    println!("─────────────────────────────────");

    let tables = vec![
        "customers",
        "categories",
        "products",
        "accounts",
        "inventory",
        "orders",
        "order_items",
        "invoices",
        "payments",
        "subscriptions",
        "transactions",
        "audit_logs",
    ];

    for table in tables {
        let row = client
            .query_one(&format!("SELECT COUNT(*) as cnt FROM {}", table), &[])
            .await?;
        let count: i64 = row.get("cnt");
        println!("{:<20} {:>6} records", table, count);
    }

    println!("─────────────────────────────────");
    let row = client
        .query_one("SELECT COUNT(*) as cnt FROM information_schema.tables WHERE table_schema='public' AND table_type='BASE TABLE'", &[])
        .await?;
    let table_count: i64 = row.get("cnt");
    println!("Total tables:        {}", table_count);

    Ok(())
}
