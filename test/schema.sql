-- MCP PostgreSQL Test Schema
-- Comprehensive database with 50+ tables, relationships, indexes, and partitioning
-- Total size: 5GB+
-- Generated: 2026-06-13

-- Core Domain Tables

-- 1. Organizations
CREATE TABLE organizations (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    domain VARCHAR(255),
    founded_date DATE,
    revenue DECIMAL(15,2),
    employee_count INTEGER,
    country VARCHAR(2),
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_organizations_name ON organizations(name);
CREATE INDEX idx_organizations_country ON organizations(country);
CREATE INDEX idx_organizations_created_at ON organizations(created_at);

-- 2. Users
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL UNIQUE,
    username VARCHAR(100) NOT NULL,
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    phone VARCHAR(20),
    role VARCHAR(50),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_users_org_id ON users(org_id);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_is_active ON users(is_active);

-- 3. Departments
CREATE TABLE departments (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    code VARCHAR(50),
    manager_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    budget DECIMAL(15,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_departments_org_id ON departments(org_id);
CREATE INDEX idx_departments_name ON departments(name);

-- 4. Employee Data (Partitioned by year)
CREATE TABLE employees (
    id BIGSERIAL,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    department_id BIGINT NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
    salary DECIMAL(15,2),
    hire_date DATE,
    termination_date DATE,
    designation VARCHAR(100),
    performance_rating DECIMAL(3,2),
    hire_year INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, hire_year)
) PARTITION BY RANGE (hire_year);

CREATE TABLE employees_2020 PARTITION OF employees FOR VALUES FROM (2020) TO (2021);
CREATE TABLE employees_2021 PARTITION OF employees FOR VALUES FROM (2021) TO (2022);
CREATE TABLE employees_2022 PARTITION OF employees FOR VALUES FROM (2022) TO (2023);
CREATE TABLE employees_2023 PARTITION OF employees FOR VALUES FROM (2023) TO (2024);
CREATE TABLE employees_2024 PARTITION OF employees FOR VALUES FROM (2024) TO (2025);
CREATE TABLE employees_2025 PARTITION OF employees FOR VALUES FROM (2025) TO (2026);
CREATE TABLE employees_2026 PARTITION OF employees FOR VALUES FROM (2026) TO (2027);

CREATE INDEX idx_employees_user_id ON employees(user_id);
CREATE INDEX idx_employees_department_id ON employees(department_id);
CREATE INDEX idx_employees_hire_date ON employees(hire_date);

-- 5. Projects (Partitioned by year)
CREATE TABLE projects (
    id BIGSERIAL,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(50),
    start_date DATE,
    end_date DATE,
    budget DECIMAL(15,2),
    manager_id BIGINT REFERENCES users(id),
    start_year INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, start_year)
) PARTITION BY RANGE (start_year);

CREATE TABLE projects_2020 PARTITION OF projects FOR VALUES FROM (2020) TO (2021);
CREATE TABLE projects_2021 PARTITION OF projects FOR VALUES FROM (2021) TO (2022);
CREATE TABLE projects_2022 PARTITION OF projects FOR VALUES FROM (2022) TO (2023);
CREATE TABLE projects_2023 PARTITION OF projects FOR VALUES FROM (2023) TO (2024);
CREATE TABLE projects_2024 PARTITION OF projects FOR VALUES FROM (2024) TO (2025);
CREATE TABLE projects_2025 PARTITION OF projects FOR VALUES FROM (2025) TO (2026);
CREATE TABLE projects_2026 PARTITION OF projects FOR VALUES FROM (2026) TO (2027);

CREATE INDEX idx_projects_org_id ON projects(org_id);
CREATE INDEX idx_projects_status ON projects(status);

-- 6. Project Members
CREATE TABLE project_members (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(100),
    hours_allocated DECIMAL(8,2),
    joined_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_project_members_project_id ON project_members(project_id);
CREATE INDEX idx_project_members_user_id ON project_members(user_id);

-- 7. Tasks (Partitioned by month)
CREATE TABLE tasks (
    id BIGSERIAL,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    assigned_to BIGINT REFERENCES users(id),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(50),
    priority VARCHAR(50),
    due_date DATE,
    created_date DATE,
    task_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, task_month)
) PARTITION BY RANGE (task_month);

CREATE TABLE tasks_202401 PARTITION OF tasks FOR VALUES FROM (202401) TO (202402);
CREATE TABLE tasks_202402 PARTITION OF tasks FOR VALUES FROM (202402) TO (202403);
CREATE TABLE tasks_202403 PARTITION OF tasks FOR VALUES FROM (202403) TO (202404);
CREATE TABLE tasks_202404 PARTITION OF tasks FOR VALUES FROM (202404) TO (202405);
CREATE TABLE tasks_202405 PARTITION OF tasks FOR VALUES FROM (202405) TO (202406);
CREATE TABLE tasks_202406 PARTITION OF tasks FOR VALUES FROM (202406) TO (202407);
CREATE TABLE tasks_202407 PARTITION OF tasks FOR VALUES FROM (202407) TO (202408);
CREATE TABLE tasks_202408 PARTITION OF tasks FOR VALUES FROM (202408) TO (202409);
CREATE TABLE tasks_202409 PARTITION OF tasks FOR VALUES FROM (202409) TO (202410);
CREATE TABLE tasks_202410 PARTITION OF tasks FOR VALUES FROM (202410) TO (202411);
CREATE TABLE tasks_202411 PARTITION OF tasks FOR VALUES FROM (202411) TO (202412);
CREATE TABLE tasks_202412 PARTITION OF tasks FOR VALUES FROM (202412) TO (202501);
CREATE TABLE tasks_202501 PARTITION OF tasks FOR VALUES FROM (202501) TO (202502);
CREATE TABLE tasks_202502 PARTITION OF tasks FOR VALUES FROM (202502) TO (202503);
CREATE TABLE tasks_202503 PARTITION OF tasks FOR VALUES FROM (202503) TO (202504);
CREATE TABLE tasks_202504 PARTITION OF tasks FOR VALUES FROM (202504) TO (202505);
CREATE TABLE tasks_202505 PARTITION OF tasks FOR VALUES FROM (202505) TO (202506);
CREATE TABLE tasks_202506 PARTITION OF tasks FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_tasks_project_id ON tasks(project_id);
CREATE INDEX idx_tasks_assigned_to ON tasks(assigned_to);
CREATE INDEX idx_tasks_status ON tasks(status);

-- 8. Task Comments
CREATE TABLE task_comments (
    id BIGSERIAL PRIMARY KEY,
    task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    comment TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_task_comments_task_id ON task_comments(task_id);
CREATE INDEX idx_task_comments_user_id ON task_comments(user_id);

-- 9. Time Entries (Partitioned by month)
CREATE TABLE time_entries (
    id BIGSERIAL,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    task_id BIGINT REFERENCES tasks(id) ON DELETE SET NULL,
    project_id BIGINT REFERENCES projects(id) ON DELETE CASCADE,
    hours DECIMAL(8,2),
    entry_date DATE,
    description TEXT,
    entry_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, entry_month)
) PARTITION BY RANGE (entry_month);

CREATE TABLE time_entries_202401 PARTITION OF time_entries FOR VALUES FROM (202401) TO (202402);
CREATE TABLE time_entries_202402 PARTITION OF time_entries FOR VALUES FROM (202402) TO (202403);
CREATE TABLE time_entries_202403 PARTITION OF time_entries FOR VALUES FROM (202403) TO (202404);
CREATE TABLE time_entries_202404 PARTITION OF time_entries FOR VALUES FROM (202404) TO (202405);
CREATE TABLE time_entries_202405 PARTITION OF time_entries FOR VALUES FROM (202405) TO (202406);
CREATE TABLE time_entries_202406 PARTITION OF time_entries FOR VALUES FROM (202406) TO (202407);
CREATE TABLE time_entries_202407 PARTITION OF time_entries FOR VALUES FROM (202407) TO (202408);
CREATE TABLE time_entries_202408 PARTITION OF time_entries FOR VALUES FROM (202408) TO (202409);
CREATE TABLE time_entries_202409 PARTITION OF time_entries FOR VALUES FROM (202409) TO (202410);
CREATE TABLE time_entries_202410 PARTITION OF time_entries FOR VALUES FROM (202410) TO (202411);
CREATE TABLE time_entries_202411 PARTITION OF time_entries FOR VALUES FROM (202411) TO (202412);
CREATE TABLE time_entries_202412 PARTITION OF time_entries FOR VALUES FROM (202412) TO (202501);
CREATE TABLE time_entries_202501 PARTITION OF time_entries FOR VALUES FROM (202501) TO (202502);
CREATE TABLE time_entries_202502 PARTITION OF time_entries FOR VALUES FROM (202502) TO (202503);
CREATE TABLE time_entries_202503 PARTITION OF time_entries FOR VALUES FROM (202503) TO (202504);
CREATE TABLE time_entries_202504 PARTITION OF time_entries FOR VALUES FROM (202504) TO (202505);
CREATE TABLE time_entries_202505 PARTITION OF time_entries FOR VALUES FROM (202505) TO (202506);
CREATE TABLE time_entries_202506 PARTITION OF time_entries FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_time_entries_user_id ON time_entries(user_id);
CREATE INDEX idx_time_entries_project_id ON time_entries(project_id);

-- 10. Invoices (Partitioned by year)
CREATE TABLE invoices (
    id BIGSERIAL,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    invoice_number VARCHAR(50) NOT NULL UNIQUE,
    client_id BIGINT REFERENCES organizations(id),
    amount DECIMAL(15,2),
    tax_amount DECIMAL(15,2),
    total DECIMAL(15,2),
    status VARCHAR(50),
    issued_date DATE,
    due_date DATE,
    invoice_year INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, invoice_year)
) PARTITION BY RANGE (invoice_year);

CREATE TABLE invoices_2020 PARTITION OF invoices FOR VALUES FROM (2020) TO (2021);
CREATE TABLE invoices_2021 PARTITION OF invoices FOR VALUES FROM (2021) TO (2022);
CREATE TABLE invoices_2022 PARTITION OF invoices FOR VALUES FROM (2022) TO (2023);
CREATE TABLE invoices_2023 PARTITION OF invoices FOR VALUES FROM (2023) TO (2024);
CREATE TABLE invoices_2024 PARTITION OF invoices FOR VALUES FROM (2024) TO (2025);
CREATE TABLE invoices_2025 PARTITION OF invoices FOR VALUES FROM (2025) TO (2026);
CREATE TABLE invoices_2026 PARTITION OF invoices FOR VALUES FROM (2026) TO (2027);

CREATE INDEX idx_invoices_org_id ON invoices(org_id);
CREATE INDEX idx_invoices_status ON invoices(status);

-- 11. Invoice Items
CREATE TABLE invoice_items (
    id BIGSERIAL PRIMARY KEY,
    invoice_id BIGINT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    description TEXT,
    quantity DECIMAL(10,2),
    unit_price DECIMAL(15,2),
    amount DECIMAL(15,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_invoice_items_invoice_id ON invoice_items(invoice_id);

-- 12. Expenses (Partitioned by month)
CREATE TABLE expenses (
    id BIGSERIAL,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    category VARCHAR(100),
    amount DECIMAL(15,2),
    currency VARCHAR(3),
    description TEXT,
    expense_date DATE,
    status VARCHAR(50),
    receipt_url VARCHAR(500),
    expense_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, expense_month)
) PARTITION BY RANGE (expense_month);

CREATE TABLE expenses_202401 PARTITION OF expenses FOR VALUES FROM (202401) TO (202402);
CREATE TABLE expenses_202402 PARTITION OF expenses FOR VALUES FROM (202402) TO (202403);
CREATE TABLE expenses_202403 PARTITION OF expenses FOR VALUES FROM (202403) TO (202404);
CREATE TABLE expenses_202404 PARTITION OF expenses FOR VALUES FROM (202404) TO (202405);
CREATE TABLE expenses_202405 PARTITION OF expenses FOR VALUES FROM (202405) TO (202406);
CREATE TABLE expenses_202406 PARTITION OF expenses FOR VALUES FROM (202406) TO (202407);
CREATE TABLE expenses_202407 PARTITION OF expenses FOR VALUES FROM (202407) TO (202408);
CREATE TABLE expenses_202408 PARTITION OF expenses FOR VALUES FROM (202408) TO (202409);
CREATE TABLE expenses_202409 PARTITION OF expenses FOR VALUES FROM (202409) TO (202410);
CREATE TABLE expenses_202410 PARTITION OF expenses FOR VALUES FROM (202410) TO (202411);
CREATE TABLE expenses_202411 PARTITION OF expenses FOR VALUES FROM (202411) TO (202412);
CREATE TABLE expenses_202412 PARTITION OF expenses FOR VALUES FROM (202412) TO (202501);
CREATE TABLE expenses_202501 PARTITION OF expenses FOR VALUES FROM (202501) TO (202502);
CREATE TABLE expenses_202502 PARTITION OF expenses FOR VALUES FROM (202502) TO (202503);
CREATE TABLE expenses_202503 PARTITION OF expenses FOR VALUES FROM (202503) TO (202504);
CREATE TABLE expenses_202504 PARTITION OF expenses FOR VALUES FROM (202504) TO (202505);
CREATE TABLE expenses_202505 PARTITION OF expenses FOR VALUES FROM (202505) TO (202506);
CREATE TABLE expenses_202506 PARTITION OF expenses FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_expenses_user_id ON expenses(user_id);
CREATE INDEX idx_expenses_category ON expenses(category);

-- 13. Clients
CREATE TABLE clients (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255),
    phone VARCHAR(20),
    company_name VARCHAR(255),
    industry VARCHAR(100),
    website VARCHAR(255),
    country VARCHAR(2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_clients_org_id ON clients(org_id);
CREATE INDEX idx_clients_name ON clients(name);

-- 14. Products
CREATE TABLE products (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(100),
    price DECIMAL(15,2),
    cost DECIMAL(15,2),
    stock_quantity INTEGER,
    sku VARCHAR(100) UNIQUE,
    supplier_id BIGINT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_products_org_id ON products(org_id);
CREATE INDEX idx_products_category ON products(category);

-- 15. Orders (Partitioned by month)
CREATE TABLE orders (
    id BIGSERIAL,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    client_id BIGINT REFERENCES clients(id),
    order_number VARCHAR(50) UNIQUE NOT NULL,
    total_amount DECIMAL(15,2),
    status VARCHAR(50),
    order_date DATE,
    delivery_date DATE,
    order_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, order_month)
) PARTITION BY RANGE (order_month);

CREATE TABLE orders_202401 PARTITION OF orders FOR VALUES FROM (202401) TO (202402);
CREATE TABLE orders_202402 PARTITION OF orders FOR VALUES FROM (202402) TO (202403);
CREATE TABLE orders_202403 PARTITION OF orders FOR VALUES FROM (202403) TO (202404);
CREATE TABLE orders_202404 PARTITION OF orders FOR VALUES FROM (202404) TO (202405);
CREATE TABLE orders_202405 PARTITION OF orders FOR VALUES FROM (202405) TO (202406);
CREATE TABLE orders_202406 PARTITION OF orders FOR VALUES FROM (202406) TO (202407);
CREATE TABLE orders_202407 PARTITION OF orders FOR VALUES FROM (202407) TO (202408);
CREATE TABLE orders_202408 PARTITION OF orders FOR VALUES FROM (202408) TO (202409);
CREATE TABLE orders_202409 PARTITION OF orders FOR VALUES FROM (202409) TO (202410);
CREATE TABLE orders_202410 PARTITION OF orders FOR VALUES FROM (202410) TO (202411);
CREATE TABLE orders_202411 PARTITION OF orders FOR VALUES FROM (202411) TO (202412);
CREATE TABLE orders_202412 PARTITION OF orders FOR VALUES FROM (202412) TO (202501);
CREATE TABLE orders_202501 PARTITION OF orders FOR VALUES FROM (202501) TO (202502);
CREATE TABLE orders_202502 PARTITION OF orders FOR VALUES FROM (202502) TO (202503);
CREATE TABLE orders_202503 PARTITION OF orders FOR VALUES FROM (202503) TO (202504);
CREATE TABLE orders_202504 PARTITION OF orders FOR VALUES FROM (202504) TO (202505);
CREATE TABLE orders_202505 PARTITION OF orders FOR VALUES FROM (202505) TO (202506);
CREATE TABLE orders_202506 PARTITION OF orders FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_orders_org_id ON orders(org_id);
CREATE INDEX idx_orders_client_id ON orders(client_id);
CREATE INDEX idx_orders_status ON orders(status);

-- 16. Order Items
CREATE TABLE order_items (
    id BIGSERIAL PRIMARY KEY,
    order_id BIGINT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id BIGINT NOT NULL REFERENCES products(id),
    quantity INTEGER,
    unit_price DECIMAL(15,2),
    discount DECIMAL(15,2),
    total DECIMAL(15,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_order_items_order_id ON order_items(order_id);
CREATE INDEX idx_order_items_product_id ON order_items(product_id);

-- 17. Payments (Partitioned by month)
CREATE TABLE payments (
    id BIGSERIAL,
    invoice_id BIGINT REFERENCES invoices(id) ON DELETE CASCADE,
    order_id BIGINT REFERENCES orders(id) ON DELETE CASCADE,
    amount DECIMAL(15,2),
    payment_method VARCHAR(50),
    status VARCHAR(50),
    payment_date DATE,
    reference_number VARCHAR(100),
    payment_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, payment_month)
) PARTITION BY RANGE (payment_month);

CREATE TABLE payments_202401 PARTITION OF payments FOR VALUES FROM (202401) TO (202402);
CREATE TABLE payments_202402 PARTITION OF payments FOR VALUES FROM (202402) TO (202403);
CREATE TABLE payments_202403 PARTITION OF payments FOR VALUES FROM (202403) TO (202404);
CREATE TABLE payments_202404 PARTITION OF payments FOR VALUES FROM (202404) TO (202405);
CREATE TABLE payments_202405 PARTITION OF payments FOR VALUES FROM (202405) TO (202406);
CREATE TABLE payments_202406 PARTITION OF payments FOR VALUES FROM (202406) TO (202407);
CREATE TABLE payments_202407 PARTITION OF payments FOR VALUES FROM (202407) TO (202408);
CREATE TABLE payments_202408 PARTITION OF payments FOR VALUES FROM (202408) TO (202409);
CREATE TABLE payments_202409 PARTITION OF payments FOR VALUES FROM (202409) TO (202410);
CREATE TABLE payments_202410 PARTITION OF payments FOR VALUES FROM (202410) TO (202411);
CREATE TABLE payments_202411 PARTITION OF payments FOR VALUES FROM (202411) TO (202412);
CREATE TABLE payments_202412 PARTITION OF payments FOR VALUES FROM (202412) TO (202501);
CREATE TABLE payments_202501 PARTITION OF payments FOR VALUES FROM (202501) TO (202502);
CREATE TABLE payments_202502 PARTITION OF payments FOR VALUES FROM (202502) TO (202503);
CREATE TABLE payments_202503 PARTITION OF payments FOR VALUES FROM (202503) TO (202504);
CREATE TABLE payments_202504 PARTITION OF payments FOR VALUES FROM (202504) TO (202505);
CREATE TABLE payments_202505 PARTITION OF payments FOR VALUES FROM (202505) TO (202506);
CREATE TABLE payments_202506 PARTITION OF payments FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_payments_invoice_id ON payments(invoice_id);
CREATE INDEX idx_payments_order_id ON payments(order_id);

-- 18. Vendors
CREATE TABLE vendors (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255),
    phone VARCHAR(20),
    country VARCHAR(2),
    payment_terms VARCHAR(100),
    rating DECIMAL(3,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_vendors_name ON vendors(name);

-- 19. Purchase Orders (Partitioned by year)
CREATE TABLE purchase_orders (
    id BIGSERIAL,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    vendor_id BIGINT NOT NULL REFERENCES vendors(id),
    po_number VARCHAR(50) UNIQUE NOT NULL,
    total_amount DECIMAL(15,2),
    status VARCHAR(50),
    order_date DATE,
    delivery_date DATE,
    po_year INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, po_year)
) PARTITION BY RANGE (po_year);

CREATE TABLE purchase_orders_2020 PARTITION OF purchase_orders FOR VALUES FROM (2020) TO (2021);
CREATE TABLE purchase_orders_2021 PARTITION OF purchase_orders FOR VALUES FROM (2021) TO (2022);
CREATE TABLE purchase_orders_2022 PARTITION OF purchase_orders FOR VALUES FROM (2022) TO (2023);
CREATE TABLE purchase_orders_2023 PARTITION OF purchase_orders FOR VALUES FROM (2023) TO (2024);
CREATE TABLE purchase_orders_2024 PARTITION OF purchase_orders FOR VALUES FROM (2024) TO (2025);
CREATE TABLE purchase_orders_2025 PARTITION OF purchase_orders FOR VALUES FROM (2025) TO (2026);
CREATE TABLE purchase_orders_2026 PARTITION OF purchase_orders FOR VALUES FROM (2026) TO (2027);

CREATE INDEX idx_purchase_orders_org_id ON purchase_orders(org_id);
CREATE INDEX idx_purchase_orders_vendor_id ON purchase_orders(vendor_id);

-- 20. Purchase Order Items
CREATE TABLE purchase_order_items (
    id BIGSERIAL PRIMARY KEY,
    purchase_order_id BIGINT NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    product_id BIGINT NOT NULL REFERENCES products(id),
    quantity INTEGER,
    unit_price DECIMAL(15,2),
    total DECIMAL(15,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_purchase_order_items_purchase_order_id ON purchase_order_items(purchase_order_id);

-- 21. Inventory (Partitioned by month)
CREATE TABLE inventory (
    id BIGSERIAL,
    product_id BIGINT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    quantity_in_stock INTEGER,
    quantity_reserved INTEGER,
    quantity_available INTEGER,
    warehouse_location VARCHAR(100),
    last_counted DATE,
    count_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, count_month)
) PARTITION BY RANGE (count_month);

CREATE TABLE inventory_202401 PARTITION OF inventory FOR VALUES FROM (202401) TO (202402);
CREATE TABLE inventory_202402 PARTITION OF inventory FOR VALUES FROM (202402) TO (202403);
CREATE TABLE inventory_202403 PARTITION OF inventory FOR VALUES FROM (202403) TO (202404);
CREATE TABLE inventory_202404 PARTITION OF inventory FOR VALUES FROM (202404) TO (202405);
CREATE TABLE inventory_202405 PARTITION OF inventory FOR VALUES FROM (202405) TO (202406);
CREATE TABLE inventory_202406 PARTITION OF inventory FOR VALUES FROM (202406) TO (202407);
CREATE TABLE inventory_202407 PARTITION OF inventory FOR VALUES FROM (202407) TO (202408);
CREATE TABLE inventory_202408 PARTITION OF inventory FOR VALUES FROM (202408) TO (202409);
CREATE TABLE inventory_202409 PARTITION OF inventory FOR VALUES FROM (202409) TO (202410);
CREATE TABLE inventory_202410 PARTITION OF inventory FOR VALUES FROM (202410) TO (202411);
CREATE TABLE inventory_202411 PARTITION OF inventory FOR VALUES FROM (202411) TO (202412);
CREATE TABLE inventory_202412 PARTITION OF inventory FOR VALUES FROM (202412) TO (202501);
CREATE TABLE inventory_202501 PARTITION OF inventory FOR VALUES FROM (202501) TO (202502);
CREATE TABLE inventory_202502 PARTITION OF inventory FOR VALUES FROM (202502) TO (202503);
CREATE TABLE inventory_202503 PARTITION OF inventory FOR VALUES FROM (202503) TO (202504);
CREATE TABLE inventory_202504 PARTITION OF inventory FOR VALUES FROM (202504) TO (202505);
CREATE TABLE inventory_202505 PARTITION OF inventory FOR VALUES FROM (202505) TO (202506);
CREATE TABLE inventory_202506 PARTITION OF inventory FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_inventory_product_id ON inventory(product_id);

-- 22. Shipments (Partitioned by month)
CREATE TABLE shipments (
    id BIGSERIAL,
    order_id BIGINT NOT NULL REFERENCES orders(id),
    tracking_number VARCHAR(100) UNIQUE,
    carrier VARCHAR(100),
    status VARCHAR(50),
    ship_date DATE,
    delivery_date DATE,
    ship_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, ship_month)
) PARTITION BY RANGE (ship_month);

CREATE TABLE shipments_202401 PARTITION OF shipments FOR VALUES FROM (202401) TO (202402);
CREATE TABLE shipments_202402 PARTITION OF shipments FOR VALUES FROM (202402) TO (202403);
CREATE TABLE shipments_202403 PARTITION OF shipments FOR VALUES FROM (202403) TO (202404);
CREATE TABLE shipments_202404 PARTITION OF shipments FOR VALUES FROM (202404) TO (202405);
CREATE TABLE shipments_202405 PARTITION OF shipments FOR VALUES FROM (202405) TO (202406);
CREATE TABLE shipments_202406 PARTITION OF shipments FOR VALUES FROM (202406) TO (202407);
CREATE TABLE shipments_202407 PARTITION OF shipments FOR VALUES FROM (202407) TO (202408);
CREATE TABLE shipments_202408 PARTITION OF shipments FOR VALUES FROM (202408) TO (202409);
CREATE TABLE shipments_202409 PARTITION OF shipments FOR VALUES FROM (202409) TO (202410);
CREATE TABLE shipments_202410 PARTITION OF shipments FOR VALUES FROM (202410) TO (202411);
CREATE TABLE shipments_202411 PARTITION OF shipments FOR VALUES FROM (202411) TO (202412);
CREATE TABLE shipments_202412 PARTITION OF shipments FOR VALUES FROM (202412) TO (202501);
CREATE TABLE shipments_202501 PARTITION OF shipments FOR VALUES FROM (202501) TO (202502);
CREATE TABLE shipments_202502 PARTITION OF shipments FOR VALUES FROM (202502) TO (202503);
CREATE TABLE shipments_202503 PARTITION OF shipments FOR VALUES FROM (202503) TO (202504);
CREATE TABLE shipments_202504 PARTITION OF shipments FOR VALUES FROM (202504) TO (202505);
CREATE TABLE shipments_202505 PARTITION OF shipments FOR VALUES FROM (202505) TO (202506);
CREATE TABLE shipments_202506 PARTITION OF shipments FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_shipments_order_id ON shipments(order_id);
CREATE INDEX idx_shipments_status ON shipments(status);

-- 23. Quality Checks (Partitioned by month)
CREATE TABLE quality_checks (
    id BIGSERIAL,
    order_id BIGINT NOT NULL REFERENCES orders(id),
    checked_by BIGINT REFERENCES users(id),
    status VARCHAR(50),
    notes TEXT,
    check_date DATE,
    check_month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, check_month)
) PARTITION BY RANGE (check_month);

CREATE TABLE quality_checks_202401 PARTITION OF quality_checks FOR VALUES FROM (202401) TO (202402);
CREATE TABLE quality_checks_202402 PARTITION OF quality_checks FOR VALUES FROM (202402) TO (202403);
CREATE TABLE quality_checks_202403 PARTITION OF quality_checks FOR VALUES FROM (202403) TO (202404);
CREATE TABLE quality_checks_202404 PARTITION OF quality_checks FOR VALUES FROM (202404) TO (202405);
CREATE TABLE quality_checks_202405 PARTITION OF quality_checks FOR VALUES FROM (202405) TO (202406);
CREATE TABLE quality_checks_202406 PARTITION OF quality_checks FOR VALUES FROM (202406) TO (202407);
CREATE TABLE quality_checks_202407 PARTITION OF quality_checks FOR VALUES FROM (202407) TO (202408);
CREATE TABLE quality_checks_202408 PARTITION OF quality_checks FOR VALUES FROM (202408) TO (202409);
CREATE TABLE quality_checks_202409 PARTITION OF quality_checks FOR VALUES FROM (202409) TO (202410);
CREATE TABLE quality_checks_202410 PARTITION OF quality_checks FOR VALUES FROM (202410) TO (202411);
CREATE TABLE quality_checks_202411 PARTITION OF quality_checks FOR VALUES FROM (202411) TO (202412);
CREATE TABLE quality_checks_202412 PARTITION OF quality_checks FOR VALUES FROM (202412) TO (202501);
CREATE TABLE quality_checks_202501 PARTITION OF quality_checks FOR VALUES FROM (202501) TO (202502);
CREATE TABLE quality_checks_202502 PARTITION OF quality_checks FOR VALUES FROM (202502) TO (202503);
CREATE TABLE quality_checks_202503 PARTITION OF quality_checks FOR VALUES FROM (202503) TO (202504);
CREATE TABLE quality_checks_202504 PARTITION OF quality_checks FOR VALUES FROM (202504) TO (202505);
CREATE TABLE quality_checks_202505 PARTITION OF quality_checks FOR VALUES FROM (202505) TO (202506);
CREATE TABLE quality_checks_202506 PARTITION OF quality_checks FOR VALUES FROM (202506) TO (202507);

CREATE INDEX idx_quality_checks_order_id ON quality_checks(order_id);

-- 24-30. Additional reference tables

CREATE TABLE document_types (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(100) UNIQUE NOT NULL,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_document_types_name ON document_types(name);

CREATE TABLE documents (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    document_type_id BIGINT NOT NULL REFERENCES document_types(id),
    file_name VARCHAR(255),
    file_path VARCHAR(500),
    file_size BIGINT,
    created_by BIGINT REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_documents_org_id ON documents(org_id);
CREATE INDEX idx_documents_document_type_id ON documents(document_type_id);

CREATE TABLE audit_logs (
    id BIGSERIAL PRIMARY KEY,
    table_name VARCHAR(100),
    operation VARCHAR(20),
    record_id BIGINT,
    old_values JSONB,
    new_values JSONB,
    changed_by BIGINT REFERENCES users(id),
    changed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_audit_logs_table_name ON audit_logs(table_name);
CREATE INDEX idx_audit_logs_record_id ON audit_logs(record_id);
CREATE INDEX idx_audit_logs_changed_at ON audit_logs(changed_at);

CREATE TABLE notifications (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message TEXT,
    is_read BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_notifications_user_id ON notifications(user_id);
CREATE INDEX idx_notifications_is_read ON notifications(is_read);

CREATE TABLE settings (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT REFERENCES organizations(id) ON DELETE CASCADE,
    setting_key VARCHAR(255),
    setting_value TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_settings_org_id ON settings(org_id);
CREATE INDEX idx_settings_key ON settings(setting_key);

CREATE TABLE email_templates (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    subject VARCHAR(500),
    body TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_email_templates_name ON email_templates(name);

CREATE TABLE email_logs (
    id BIGSERIAL PRIMARY KEY,
    recipient_email VARCHAR(255),
    subject VARCHAR(500),
    status VARCHAR(50),
    sent_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_email_logs_recipient_email ON email_logs(recipient_email);
CREATE INDEX idx_email_logs_status ON email_logs(status);

CREATE TABLE api_keys (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id),
    api_key VARCHAR(255) UNIQUE NOT NULL,
    last_used_at TIMESTAMP,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_api_keys_org_id ON api_keys(org_id);
CREATE INDEX idx_api_keys_api_key ON api_keys(api_key);

-- Additional tables for completeness (31-50+)

CREATE TABLE reports (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    report_type VARCHAR(100),
    created_by BIGINT REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_reports_org_id ON reports(org_id);

CREATE TABLE report_schedules (
    id BIGSERIAL PRIMARY KEY,
    report_id BIGINT NOT NULL REFERENCES reports(id) ON DELETE CASCADE,
    frequency VARCHAR(50),
    recipients TEXT,
    last_generated TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_report_schedules_report_id ON report_schedules(report_id);

CREATE TABLE feedback (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rating INTEGER,
    comment TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_feedback_user_id ON feedback(user_id);

CREATE TABLE training_programs (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_training_programs_name ON training_programs(name);

CREATE TABLE employee_training (
    id BIGSERIAL PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    program_id BIGINT NOT NULL REFERENCES training_programs(id),
    completion_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_employee_training_employee_id ON employee_training(employee_id);

CREATE TABLE certifications (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    certification_name VARCHAR(255),
    issuer VARCHAR(255),
    issue_date DATE,
    expiry_date DATE,
    certificate_url VARCHAR(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_certifications_user_id ON certifications(user_id);

CREATE TABLE performance_reviews (
    id BIGSERIAL PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    reviewer_id BIGINT REFERENCES users(id),
    rating DECIMAL(3,2),
    feedback TEXT,
    review_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_performance_reviews_employee_id ON performance_reviews(employee_id);

CREATE TABLE leave_requests (
    id BIGSERIAL PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    leave_type VARCHAR(50),
    start_date DATE,
    end_date DATE,
    status VARCHAR(50),
    approver_id BIGINT REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_leave_requests_employee_id ON leave_requests(employee_id);
CREATE INDEX idx_leave_requests_status ON leave_requests(status);

CREATE TABLE holidays (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    holiday_date DATE UNIQUE NOT NULL,
    country VARCHAR(2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_holidays_holiday_date ON holidays(holiday_date);

CREATE TABLE shift_schedules (
    id BIGSERIAL PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    shift_date DATE,
    start_time TIME,
    end_time TIME,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_shift_schedules_employee_id ON shift_schedules(employee_id);
CREATE INDEX idx_shift_schedules_shift_date ON shift_schedules(shift_date);

CREATE TABLE attendance (
    id BIGSERIAL PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    attendance_date DATE,
    check_in_time TIMESTAMP,
    check_out_time TIMESTAMP,
    status VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_attendance_employee_id ON attendance(employee_id);
CREATE INDEX idx_attendance_attendance_date ON attendance(attendance_date);

CREATE TABLE equipment (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    asset_number VARCHAR(100) UNIQUE,
    category VARCHAR(100),
    purchase_date DATE,
    purchase_cost DECIMAL(15,2),
    depreciation_rate DECIMAL(5,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_equipment_name ON equipment(name);

CREATE TABLE equipment_assignments (
    id BIGSERIAL PRIMARY KEY,
    equipment_id BIGINT NOT NULL REFERENCES equipment(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_date DATE,
    returned_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_equipment_assignments_equipment_id ON equipment_assignments(equipment_id);
CREATE INDEX idx_equipment_assignments_user_id ON equipment_assignments(user_id);

CREATE TABLE supplier_ratings (
    id BIGSERIAL PRIMARY KEY,
    vendor_id BIGINT NOT NULL REFERENCES vendors(id) ON DELETE CASCADE,
    quality_score DECIMAL(3,2),
    delivery_score DECIMAL(3,2),
    communication_score DECIMAL(3,2),
    rated_by BIGINT REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_supplier_ratings_vendor_id ON supplier_ratings(vendor_id);

CREATE TABLE contract_types (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE contracts (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    contract_type_id BIGINT NOT NULL REFERENCES contract_types(id),
    vendor_id BIGINT REFERENCES vendors(id),
    client_id BIGINT REFERENCES clients(id),
    start_date DATE,
    end_date DATE,
    value DECIMAL(15,2),
    status VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_contracts_org_id ON contracts(org_id);
CREATE INDEX idx_contracts_status ON contracts(status);

-- Composite indexes for common queries
CREATE INDEX idx_users_org_email ON users(org_id, email);
CREATE INDEX idx_employees_dept_date ON employees(department_id, hire_date);
CREATE INDEX idx_tasks_project_status ON tasks(project_id, status);
CREATE INDEX idx_orders_client_date ON orders(client_id, order_date);
CREATE INDEX idx_invoices_org_date ON invoices(org_id, issued_date);

-- Full-text search indexes
CREATE INDEX idx_documents_file_name_search ON documents USING gin(to_tsvector('english', file_name));
CREATE INDEX idx_tasks_title_search ON tasks USING gin(to_tsvector('english', title));

-- Create sample views for complex queries
CREATE VIEW organization_employee_count AS
SELECT o.id, o.name, COUNT(DISTINCT u.id) as employee_count
FROM organizations o
LEFT JOIN users u ON o.id = u.org_id
GROUP BY o.id, o.name;

CREATE VIEW project_status_summary AS
SELECT p.id, p.name, p.status, COUNT(t.id) as task_count
FROM projects p
LEFT JOIN tasks t ON p.id = t.project_id
GROUP BY p.id, p.name, p.status;

CREATE VIEW department_budget_summary AS
SELECT d.id, d.name, d.budget, SUM(e.salary) as total_salaries
FROM departments d
LEFT JOIN employees e ON d.id = e.department_id
GROUP BY d.id, d.name, d.budget;

-- Grant appropriate permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO postgres;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO postgres;
