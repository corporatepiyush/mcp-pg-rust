use anyhow::Result;
use clap::Parser;
use fake::faker::company::en::CompanyName;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::{FirstName, LastName, Name};
use fake::faker::lorem::en::Words;
use fake::Fake;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::info;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value = "3000")]
    port: u16,
    #[arg(long, default_value = "1000000")]
    target_rows: u64,
    #[arg(long, default_value = "1000")]
    batch_size: usize,
}

struct Generator {
    host: String, port: u16, _batch_size: usize, total: Arc<AtomicU64>,
}

impl Generator {
    fn new(h: String, p: u16, b: usize) -> Self {
        Self { host: h, port: p, _batch_size: b, total: Arc::new(AtomicU64::new(0)) }
    }

    async fn call(&self, n: &str, a: Value) -> Result<Value> {
        let mut s = TcpStream::connect(format!("{}:{}", self.host, self.port)).await?;
        let r = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":n,"arguments":a}});
        let m = serde_json::to_vec(&r)?;
        s.write_all(&m).await?;
        s.write_all(b"
").await?;
        let mut b = vec![0; 512*1024];
        let n = s.read(&mut b).await?;
        let rs: Value = serde_json::from_slice(&b[..n])?;
        if let Some(e) = rs.get("error") { return Err(anyhow::anyhow!("MCP Error: {:?}", e)); }
        Ok(rs["result"].clone())
    }

    async fn ins(&self, t: &str, cols: &[&str], rows: Vec<Vec<Value>>, ret: Option<&str>) -> Result<Vec<i64>> {
        if rows.is_empty() { return Ok(vec![]); }
        let mut p = json!({"table":t, "columns": cols, "rows":rows});
        if let Some(c) = ret { p["returning"] = json!(c); }
        let res = self.call("batch_insert", p).await?;
        self.total.fetch_add(rows.len() as u64, Ordering::Relaxed);
        if ret.is_some() {
            let ids = res["inserted_ids"].as_array().cloned().unwrap_or_default();
            Ok(ids.iter().map(|v| v.as_i64().unwrap_or(0)).collect())
        } else { Ok(vec![]) }
    }
}

impl Generator {
    async fn gen_core(&self) -> Result<(Vec<i64>, Vec<i64>, Vec<i64>, Vec<i64>)> {
        let mut o_r = vec![];
        for _ in 0..50 {
            o_r.push(vec![json!(CompanyName().fake::<String>()), json!(SafeEmail().fake::<String>()), json!(1000000.0), json!(100), json!("US")]);
        }
        let o_ids = self.ins("organizations", &["name", "domain", "revenue", "employee_count", "country"], o_r, Some("id")).await?;

        let mut d_r = vec![];
        for &oid in &o_ids {
            for i in 0..3 {
                d_r.push(vec![json!(oid), json!(format!("Dept {}", i)), json!(format!("D{}", i)), json!(100000.0)]);
            }
        }
        let d_ids = self.ins("departments", &["org_id", "name", "code", "budget"], d_r, Some("id")).await?;

        let mut u_r = vec![];
        for &oid in &o_ids {
            for _ in 0..20 {
                u_r.push(vec![json!(oid), json!(SafeEmail().fake::<String>()), json!(format!("u{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())), json!(FirstName().fake::<String>()), json!(LastName().fake::<String>()), json!("user")]);
            }
        }
        let u_ids = self.ins("users", &["org_id", "email", "username", "first_name", "last_name", "role"], u_r, Some("id")).await?;

        let mut e_r = vec![];
        for (&uid, &did) in u_ids.iter().zip(d_ids.iter().cycle()) {
            e_r.push(vec![json!(uid), json!(did), json!(50000.0), json!(2024), json!("2024-01-01"), json!("Engineer")]);
        }
        let e_ids = self.ins("employees", &["user_id", "department_id", "salary", "hire_year", "hire_date", "designation"], e_r, Some("id")).await?;
        
        Ok((o_ids, d_ids, u_ids, e_ids))
    }

    async fn gen_projects(&self, o_ids: &[i64], u_ids: &[i64]) -> Result<(Vec<i64>, Vec<i64>)> {
        let mut p_r = vec![];
        for &oid in o_ids {
            for _ in 0..5 {
                p_r.push(vec![json!(oid), json!(format!("Project {}", Words(1..2).fake::<Vec<String>>().join(" "))), json!("active"), json!(2024), json!(50000.0)]);
            }
        }
        let p_ids = self.ins("projects", &["org_id", "name", "status", "start_year", "budget"], p_r, Some("id")).await?;

        let mut pm_r = vec![];
        for &pid in &p_ids {
            for &uid in u_ids.iter().take(3) {
                pm_r.push(vec![json!(pid), json!(uid), json!("Contributor"), json!(40.0)]);
            }
        }
        self.ins("project_members", &["project_id", "user_id", "role", "hours_allocated"], pm_r, None).await?;

        let mut t_r = vec![];
        for &pid in &p_ids {
            for _ in 0..10 {
                t_r.push(vec![json!(pid), json!(u_ids[0]), json!(Words(2..4).fake::<Vec<String>>().join(" ")), json!("todo"), json!("high"), json!(202406)]);
            }
        }
        let t_ids = self.ins("tasks", &["project_id", "assigned_to", "title", "status", "priority", "task_month"], t_r, Some("id")).await?;

        Ok((p_ids, t_ids))
    }
}

impl Generator {
    async fn gen_sales(&self, o_ids: &[i64]) -> Result<(Vec<i64>, Vec<i64>, Vec<i64>, Vec<i64>)> {
        let mut c_r = vec![];
        for &oid in o_ids {
            for _ in 0..10 {
                c_r.push(vec![json!(oid), json!(Name().fake::<String>()), json!(SafeEmail().fake::<String>()), json!(CompanyName().fake::<String>()), json!("Tech")]);
            }
        }
        let c_ids = self.ins("clients", &["org_id", "name", "email", "company_name", "industry"], c_r, Some("id")).await?;

        let mut pr_r = vec![];
        for &oid in o_ids {
            for _ in 0..20 {
                pr_r.push(vec![json!(oid), json!(format!("Product {}", Words(1..2).fake::<Vec<String>>().join(" "))), json!("Electronics"), json!(100.0), json!(uuid::Uuid::new_v4().to_string())]);
            }
        }
        let pr_ids = self.ins("products", &["org_id", "name", "category", "price", "sku"], pr_r, Some("id")).await?;

        let mut ord_r = vec![];
        for (&oid, &cid) in o_ids.iter().zip(c_ids.iter().cycle()) {
            for _ in 0..5 {
                ord_r.push(vec![json!(oid), json!(cid), json!(format!("ORD-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())), json!(500.0), json!("completed"), json!(202406)]);
            }
        }
        let ord_ids = self.ins("orders", &["org_id", "client_id", "order_number", "total_amount", "status", "order_month"], ord_r, Some("id")).await?;

        let mut inv_r = vec![];
        for (&oid, &cid) in o_ids.iter().zip(c_ids.iter().cycle()) {
            inv_r.push(vec![json!(oid), json!(format!("INV-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())), json!(cid), json!(500.0), json!("paid"), json!(2024)]);
        }
        let inv_ids = self.ins("invoices", &["org_id", "invoice_number", "client_id", "amount", "status", "invoice_year"], inv_r, Some("id")).await?;

        Ok((c_ids, pr_ids, ord_ids, inv_ids))
    }

    async fn gen_supply_chain(&self, o_ids: &[i64], pr_ids: &[i64]) -> Result<(Vec<i64>, Vec<i64>)> {
        let mut v_r = vec![];
        for _ in 0..10 {
            v_r.push(vec![json!(CompanyName().fake::<String>()), json!(SafeEmail().fake::<String>()), json!("US"), json!(4.5)]);
        }
        let v_ids = self.ins("vendors", &["name", "email", "country", "rating"], v_r, Some("id")).await?;

        let mut po_r = vec![];
        for (&oid, &vid) in o_ids.iter().zip(v_ids.iter().cycle()) {
            po_r.push(vec![json!(oid), json!(vid), json!(format!("PO-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())), json!(1000.0), json!("approved"), json!(2024)]);
        }
        let po_ids = self.ins("purchase_orders", &["org_id", "vendor_id", "po_number", "total_amount", "status", "po_year"], po_r, Some("id")).await?;

        let mut i_r = vec![];
        for &prid in pr_ids {
            i_r.push(vec![json!(prid), json!(100), json!(20), json!(80), json!("Warehouse A"), json!(202406)]);
        }
        self.ins("inventory", &["product_id", "quantity_in_stock", "quantity_reserved", "quantity_available", "warehouse_location", "count_month"], i_r, None).await?;

        Ok((v_ids, po_ids))
    }
}

impl Generator {
    async fn gen_misc(&self, o_ids: &[i64], u_ids: &[i64], e_ids: &[i64]) -> Result<()> {
        let mut dt_r = vec![];
        dt_r.push(vec![json!("Contract"), json!("Legal contracts")]);
        dt_r.push(vec![json!("Invoice"), json!("Customer invoices")]);
        let dt_ids = self.ins("document_types", &["name", "description"], dt_r, Some("id")).await?;

        let mut doc_r = vec![];
        for (&oid, &dtid) in o_ids.iter().zip(dt_ids.iter().cycle()) {
            doc_r.push(vec![json!(oid), json!(dtid), json!("file.pdf"), json!("/path/to/file.pdf"), json!(1024), json!(u_ids[0])]);
        }
        self.ins("documents", &["org_id", "document_type_id", "file_name", "file_path", "file_size", "created_by"], doc_r, None).await?;

        let mut tp_r = vec![];
        tp_r.push(vec![json!("Safety 101"), json!("Basic safety training")]);
        let tp_ids = self.ins("training_programs", &["name", "description"], tp_r, Some("id")).await?;

        let mut et_r = vec![];
        for (&eid, &tpid) in e_ids.iter().zip(tp_ids.iter().cycle()) {
            et_r.push(vec![json!(eid), json!(tpid), json!("2024-06-01")]);
        }
        self.ins("employee_training", &["employee_id", "program_id", "completion_date"], et_r, None).await?;

        Ok(())
    }
}

async fn run() -> Result<()> {
    let a = Args::parse();
    let g = Arc::new(Generator::new(a.host.clone(), a.port, a.batch_size));
    let start = Instant::now();
    info!("Comprehensive high-volume generation: target={}, batch={}", a.target_rows, a.batch_size);

    let (o_ids, _d_ids, u_ids, e_ids) = g.gen_core().await?;
    let (p_ids, t_ids) = g.gen_projects(&o_ids, &u_ids).await?;
    let (c_ids, pr_ids, ord_ids, inv_ids) = g.gen_sales(&o_ids).await?;
    let (v_ids, po_ids) = g.gen_supply_chain(&o_ids, &pr_ids).await?;
    g.gen_misc(&o_ids, &u_ids, &e_ids).await?;
    g.gen_hr_extra(&u_ids, &e_ids).await?;
    g.gen_finance_extra(&o_ids, &u_ids).await?;
    g.gen_operational_items(&ord_ids, &pr_ids).await?;
    g.gen_task_depth(&u_ids, &t_ids, &p_ids).await?;
    g.gen_financial_depth(&inv_ids, &ord_ids).await?;
    g.gen_admin_depth(&o_ids, &u_ids).await?;
    g.gen_remaining(&o_ids, &u_ids, &e_ids, &c_ids, &pr_ids, &ord_ids, &v_ids, &po_ids, &p_ids).await?;

    let mut au = vec![];
    let mut current = g.total.load(Ordering::Relaxed);
    info!("Base entities created: {} rows. Filling remaining {} rows with audit logs...", current, a.target_rows.saturating_sub(current));

    while current < a.target_rows {
        let uid = u_ids[current as usize % u_ids.len()];
        au.push(vec![json!("users"), json!("UPDATE"), json!(uid), json!(null), json!({"status":"active"}), json!(uid)]);
        if au.len() >= a.batch_size {
            g.ins("audit_logs", &["table_name", "operation", "record_id", "old_values", "new_values", "changed_by"], au.drain(..).collect(), None).await?;
            current = g.total.load(Ordering::Relaxed);
        }
        if current + (au.len() as u64) >= a.target_rows { break; }
    }
    if !au.is_empty() { g.ins("audit_logs", &["table_name", "operation", "record_id", "old_values", "new_values", "changed_by"], au, None).await?; }

    info!("Generation complete: {} total rows in {:?}", g.total.load(Ordering::Relaxed), start.elapsed());
    Ok(())
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _ = rt.block_on(run());
}

impl Generator {
    async fn gen_hr_extra(&self, u_ids: &[i64], e_ids: &[i64]) -> Result<()> {
        let mut f_r = vec![];
        for &uid in u_ids {
            f_r.push(vec![json!(uid), json!(5), json!("Great work!")]);
        }
        self.ins("feedback", &["user_id", "rating", "comment"], f_r, None).await?;

        let mut lr_r = vec![];
        for &eid in e_ids {
            lr_r.push(vec![json!(eid), json!("Annual"), json!("2024-12-20"), json!("2024-12-31"), json!("approved")]);
        }
        self.ins("leave_requests", &["employee_id", "leave_type", "start_date", "end_date", "status"], lr_r, None).await?;

        let mut att_r = vec![];
        for &eid in e_ids {
            att_r.push(vec![json!(eid), json!("2024-06-01"), json!("2024-06-01 09:00:00"), json!("2024-06-01 17:00:00"), json!("present")]);
        }
        self.ins("attendance", &["employee_id", "attendance_date", "check_in_time", "check_out_time", "status"], att_r, None).await?;

        Ok(())
    }

    async fn gen_finance_extra(&self, o_ids: &[i64], u_ids: &[i64]) -> Result<()> {
        let mut s_r = vec![];
        for &oid in o_ids {
            s_r.push(vec![json!(oid), json!("theme"), json!("dark")]);
        }
        self.ins("settings", &["org_id", "setting_key", "setting_value"], s_r, None).await?;

        let mut ex_r = vec![];
        for &uid in u_ids {
            ex_r.push(vec![json!(uid), json!("Travel"), json!(150.0), json!("USD"), json!("Flight"), json!("2024-06-01"), json!("approved"), json!(202406)]);
        }
        self.ins("expenses", &["user_id", "category", "amount", "currency", "description", "expense_date", "status", "expense_month"], ex_r, None).await?;

        Ok(())
    }
}

impl Generator {
    async fn gen_operational_items(&self, ord_ids: &[i64], pr_ids: &[i64]) -> Result<()> {
        let mut oi_r = vec![];
        for &ordid in ord_ids {
            for &prid in pr_ids.iter().take(3) {
                oi_r.push(vec![json!(ordid), json!(prid), json!(2), json!(100.0), json!(0.0), json!(200.0)]);
            }
        }
        self.ins("order_items", &["order_id", "product_id", "quantity", "unit_price", "discount", "total"], oi_r, None).await?;
        Ok(())
    }
}

impl Generator {
    async fn gen_task_depth(&self, u_ids: &[i64], t_ids: &[i64], p_ids: &[i64]) -> Result<()> {
        let mut tc_r = vec![];
        for &tid in t_ids {
            tc_r.push(vec![json!(tid), json!(u_ids[0]), json!("Looks good!")]);
        }
        self.ins("task_comments", &["task_id", "user_id", "comment"], tc_r, None).await?;

        let mut te_r = vec![];
        for (&uid, &tid) in u_ids.iter().zip(t_ids.iter().cycle()) {
            te_r.push(vec![json!(uid), json!(tid), json!(p_ids[0]), json!(8.0), json!("2024-06-01"), json!(202406)]);
        }
        self.ins("time_entries", &["user_id", "task_id", "project_id", "hours", "entry_date", "entry_month"], te_r, None).await?;
        Ok(())
    }

    async fn gen_financial_depth(&self, inv_ids: &[i64], ord_ids: &[i64]) -> Result<()> {
        let mut ii_r = vec![];
        for &iid in inv_ids {
            ii_r.push(vec![json!(iid), json!("Service Charge"), json!(1.0), json!(500.0), json!(500.0)]);
        }
        self.ins("invoice_items", &["invoice_id", "description", "quantity", "unit_price", "amount"], ii_r, None).await?;

        let mut pay_r = vec![];
        for (&iid, &oid) in inv_ids.iter().zip(ord_ids.iter().cycle()) {
            pay_r.push(vec![json!(iid), json!(oid), json!(500.0), json!("credit_card"), json!("success"), json!("2024-06-01"), json!(202406)]);
        }
        self.ins("payments", &["invoice_id", "order_id", "amount", "payment_method", "status", "payment_date", "payment_month"], pay_r, None).await?;
        Ok(())
    }

    async fn gen_admin_depth(&self, o_ids: &[i64], u_ids: &[i64]) -> Result<()> {
        let mut ak_r = vec![];
        for (&oid, &uid) in o_ids.iter().zip(u_ids.iter().cycle()) {
            ak_r.push(vec![json!(oid), json!(uid), json!(uuid::Uuid::new_v4().to_string())]);
        }
        self.ins("api_keys", &["org_id", "user_id", "api_key"], ak_r, None).await?;

        let mut n_r = vec![];
        for &uid in u_ids {
            n_r.push(vec![json!(uid), json!("Welcome to the system!"), json!(false)]);
        }
        self.ins("notifications", &["user_id", "message", "is_read"], n_r, None).await?;

        let mut r_r = vec![];
        for &oid in o_ids {
            r_r.push(vec![json!(oid), json!("Monthly Report"), json!(u_ids[0])]);
        }
        let r_ids = self.ins("reports", &["org_id", "name", "created_by"], r_r, Some("id")).await?;

        let mut rs_r = vec![];
        for &rid in &r_ids {
            rs_r.push(vec![json!(rid), json!("monthly"), json!("admin@example.com")]);
        }
        self.ins("report_schedules", &["report_id", "frequency", "recipients"], rs_r, None).await?;
        Ok(())
    }

    async fn gen_remaining(&self, o_ids: &[i64], u_ids: &[i64], e_ids: &[i64], c_ids: &[i64], pr_ids: &[i64], ord_ids: &[i64], v_ids: &[i64], po_ids: &[i64], _p_ids: &[i64]) -> Result<()> {
        let mut ct_r = vec![];
        ct_r.push(vec![json!("Service")]);
        ct_r.push(vec![json!("Maintenance")]);
        ct_r.push(vec![json!("Lease")]);
        let ct_ids = self.ins("contract_types", &["name"], ct_r, Some("id")).await?;

        let mut con_r = vec![];
        for (i, (&oid, &ctid)) in o_ids.iter().zip(ct_ids.iter().cycle()).enumerate() {
            let vid = v_ids[i % v_ids.len()];
            let cid = c_ids[i % c_ids.len()];
            con_r.push(vec![json!(oid), json!(ctid), json!(vid), json!(cid), json!("2024-01-01"), json!("2024-12-31"), json!(100000.0), json!("active")]);
        }
        self.ins("contracts", &["org_id", "contract_type_id", "vendor_id", "client_id", "start_date", "end_date", "value", "status"], con_r, None).await?;

        let mut sr_r = vec![];
        for &vid in v_ids {
            sr_r.push(vec![json!(vid), json!(4.5), json!(4.0), json!(5.0), json!(u_ids[0])]);
        }
        self.ins("supplier_ratings", &["vendor_id", "quality_score", "delivery_score", "communication_score", "rated_by"], sr_r, None).await?;

        let mut poi_r = vec![];
        for &poid in po_ids {
            for &prid in pr_ids.iter().take(3) {
                poi_r.push(vec![json!(poid), json!(prid), json!(10), json!(50.0), json!(500.0)]);
            }
        }
        self.ins("purchase_order_items", &["purchase_order_id", "product_id", "quantity", "unit_price", "total"], poi_r, None).await?;

        let mut eq_r = vec![];
        eq_r.push(vec![json!("Laptop"), json!("AST-001"), json!("Electronics"), json!("2024-01-15"), json!(2000.0)]);
        eq_r.push(vec![json!("Monitor"), json!("AST-002"), json!("Electronics"), json!("2024-02-01"), json!(500.0)]);
        eq_r.push(vec![json!("Chair"), json!("AST-003"), json!("Furniture"), json!("2024-01-10"), json!(800.0)]);
        let eq_ids = self.ins("equipment", &["name", "asset_number", "category", "purchase_date", "purchase_cost"], eq_r, Some("id")).await?;

        let mut ea_r = vec![];
        for (&eid, &uid) in eq_ids.iter().zip(u_ids.iter().cycle()) {
            ea_r.push(vec![json!(eid), json!(uid), json!("2024-01-20")]);
        }
        self.ins("equipment_assignments", &["equipment_id", "user_id", "assigned_date"], ea_r, None).await?;

        let mut cer_r = vec![];
        for &uid in u_ids.iter().take(10) {
            cer_r.push(vec![json!(uid), json!("AWS Certified"), json!("Amazon"), json!("2024-03-01"), json!("2026-03-01")]);
        }
        self.ins("certifications", &["user_id", "certification_name", "issuer", "issue_date", "expiry_date"], cer_r, None).await?;

        let mut pr_r = vec![];
        for (&eid, &rid) in e_ids.iter().zip(u_ids.iter().cycle()) {
            pr_r.push(vec![json!(eid), json!(rid), json!(4.5), json!("Exceeds expectations"), json!("2024-06-15")]);
        }
        self.ins("performance_reviews", &["employee_id", "reviewer_id", "rating", "feedback", "review_date"], pr_r, None).await?;

        let mut hol_r = vec![];
        hol_r.push(vec![json!("New Year"), json!("2024-01-01"), json!("US")]);
        hol_r.push(vec![json!("Christmas"), json!("2024-12-25"), json!("US")]);
        self.ins("holidays", &["name", "holiday_date", "country"], hol_r, None).await?;

        let mut ss_r = vec![];
        for &eid in e_ids.iter().take(20) {
            ss_r.push(vec![json!(eid), json!("2024-06-10"), json!("09:00:00"), json!("17:00:00")]);
        }
        self.ins("shift_schedules", &["employee_id", "shift_date", "start_time", "end_time"], ss_r, None).await?;

        let mut et_r = vec![];
        et_r.push(vec![json!("Welcome Email"), json!("Welcome {{name}}"), json!("Hello {{name}}, welcome!")]);
        et_r.push(vec![json!("Invoice Reminder"), json!("Invoice Due"), json!("Your invoice is due.")]);
        self.ins("email_templates", &["name", "subject", "body"], et_r, None).await?;

        let mut el_r = vec![];
        for _ in 0..10 {
            el_r.push(vec![json!(SafeEmail().fake::<String>()), json!("Welcome!"), json!("sent")]);
        }
        self.ins("email_logs", &["recipient_email", "subject", "status"], el_r, None).await?;

        let mut qc_r = vec![];
        for &ordid in ord_ids.iter().take(20) {
            qc_r.push(vec![json!(ordid), json!(u_ids[0]), json!("passed"), json!("All checks passed"), json!("2024-06-05"), json!(202406)]);
        }
        self.ins("quality_checks", &["order_id", "checked_by", "status", "notes", "check_date", "check_month"], qc_r, None).await?;

        let mut sh_r = vec![];
        for &ordid in ord_ids.iter().take(20) {
            sh_r.push(vec![json!(ordid), json!(format!("TRACK-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())), json!("FedEx"), json!("shipped"), json!("2024-06-01"), json!("2024-06-05"), json!(202406)]);
        }
        self.ins("shipments", &["order_id", "tracking_number", "carrier", "status", "ship_date", "delivery_date", "ship_month"], sh_r, None).await?;

        Ok(())
    }
}
