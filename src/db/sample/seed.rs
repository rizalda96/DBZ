//! The sample content: a small shop and an analytics database, generated from a
//! fixed seed so every run looks the same.

use super::server::{SampleDb, SampleServer, SampleTable};
use crate::domain::{FieldDef, Row, TableParts};

const FIRST_NAMES: [&str; 20] = ["Ava", "Liam", "Noah", "Mia", "Ethan", "Sofia", "Lucas", "Amara", "Kenji", "Lena", "Omar", "Priya", "Mateo", "Hana", "Jonas", "Zara", "Ivan", "Nora", "Felix", "Leila"];
const LAST_NAMES: [&str; 15] = ["Walker", "Nguyen", "Santos", "Kim", "Fischer", "Rossi", "Okafor", "Tanaka", "Dubois", "Haddad", "Larsen", "Patel", "Moreno", "Sato", "Novak"];
const CITIES: [(&str, &str); 10] = [("Jakarta", "Indonesia"), ("Surabaya", "Indonesia"), ("Singapore", "Singapore"), ("Berlin", "Germany"), ("Lyon", "France"), ("Osaka", "Japan"), ("Toronto", "Canada"), ("Austin", "USA"), ("Lisbon", "Portugal"), ("Nairobi", "Kenya")];
const PRODUCTS: [(&str, &str, f64); 16] = [
    ("Ceramic Mug", "Kitchen", 8.50),
    ("Steel Water Bottle", "Outdoor", 19.90),
    ("Linen Tote Bag", "Accessories", 14.00),
    ("Desk Lamp", "Home Office", 39.00),
    ("Notebook A5", "Stationery", 4.75),
    ("Gel Pen Set", "Stationery", 6.20),
    ("Wireless Mouse", "Electronics", 24.99),
    ("USB-C Cable 2m", "Electronics", 9.99),
    ("Yoga Mat", "Fitness", 29.50),
    ("Cast Iron Pan", "Kitchen", 45.00),
    ("Rain Jacket", "Outdoor", 79.00),
    ("Wall Clock", "Home", 22.40),
    ("Plant Pot", "Home", 12.30),
    ("Bluetooth Speaker", "Electronics", 54.00),
    ("Coffee Grinder", "Kitchen", 35.80),
    ("Headphone Stand", "Home Office", 17.60),
];
const STATUSES: [&str; 6] = ["paid", "shipped", "delivered", "delivered", "cancelled", "pending"];
const DEPARTMENTS: [&str; 5] = ["Sales", "Support", "Warehouse", "Finance", "IT"];
const TITLES: [&str; 4] = ["Associate", "Specialist", "Lead", "Manager"];
const PAGES: [&str; 8] = ["/", "/products", "/products/mug", "/cart", "/checkout", "/about", "/blog/spring-sale", "/contact"];
const DEVICES: [&str; 3] = ["desktop", "mobile", "tablet"];

/// xorshift64: tiny, deterministic, good enough for fake data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() % v.len() as u64) as usize]
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

fn s(v: impl ToString) -> Option<String> {
    Some(v.to_string())
}

/// Field shorthand; `len` may be "10,2" for length and decimals.
fn f(name: &str, ty: &str, len: &str, flags: (bool, bool, bool), comment: &str) -> FieldDef {
    let (not_null, key, auto_inc) = flags;
    let mut parts = len.split(',');
    FieldDef {
        name: name.into(),
        ty: ty.into(),
        length: parts.next().unwrap_or("").into(),
        decimals: parts.next().unwrap_or("").into(),
        not_null,
        key,
        auto_inc,
        comment: comment.into(),
        ..Default::default()
    }
}

const ID: (bool, bool, bool) = (true, true, true);
const REQ: (bool, bool, bool) = (true, false, false);
const OPT: (bool, bool, bool) = (false, false, false);

fn table(name: &str, comment: &str, fields: Vec<FieldDef>, rows: Vec<Row>) -> SampleTable {
    let auto_inc = rows.len() as u64;
    SampleTable { name: name.into(), fields, rows, comment: comment.into(), auto_inc, created: "2026-01-12 09:30:00".into(), modified: "2026-11-20 16:05:12".into(), parts: TableParts::default() }
}

pub fn sample_server() -> SampleServer {
    let mut r = Rng(0x9e37_79b9_7f4a_7c15);
    let customers = customers(&mut r);
    let products = products(&mut r);
    let (orders, items) = orders(&mut r);
    let employees = employees(&mut r);
    let page_views = page_views(&mut r);
    let daily = daily_stats(&mut r);
    SampleServer {
        dbs: vec![
            SampleDb { name: "analytics".into(), tables: vec![table("daily_stats", "Rolled-up traffic per day", daily_fields(), daily), table("page_views", "Raw page view events", page_view_fields(), page_views)] },
            SampleDb {
                name: "shop".into(),
                tables: vec![
                    table("customers", "Registered customers", customer_fields(), customers),
                    table("employees", "Staff directory", employee_fields(), employees),
                    table("order_items", "Lines of each order", order_item_fields(), items),
                    table("orders", "Customer orders", order_fields(), orders),
                    table("products", "Product catalogue", product_fields(), products),
                ],
            },
        ],
    }
}

fn customers(r: &mut Rng) -> Vec<Row> {
    (1..=48u64)
        .map(|i| {
            let fname = r.pick(&FIRST_NAMES);
            let lname = r.pick(&LAST_NAMES);
            let (city, country) = CITIES[(r.next() % CITIES.len() as u64) as usize];
            let email = format!("{}.{}{}@example.com", fname.to_lowercase(), lname.to_lowercase(), i);
            let phone = if i % 7 == 0 { None } else { s(format!("+62 8{:02} {:04} {:04}", r.range(11, 99), r.range(1000, 9999), r.range(1000, 9999))) };
            let created = format!("2025-{:02}-{:02} {:02}:{:02}:00", r.range(1, 12), r.range(1, 28), r.range(8, 20), r.range(0, 59));
            vec![s(i), s(fname), s(lname), s(email), phone, s(city), s(country), s(created)]
        })
        .collect()
}

fn products(r: &mut Rng) -> Vec<Row> {
    PRODUCTS
        .iter()
        .enumerate()
        .map(|(i, (n, c, p))| vec![s(i + 1), s(format!("SKU-{:04}", 1001 + i)), s(n), s(c), s(format!("{:.2}", p)), s(r.range(0, 240)), s(if i % 5 == 4 { "0" } else { "1" })])
        .collect()
}

/// Orders and their line items.
fn orders(r: &mut Rng) -> (Vec<Row>, Vec<Row>) {
    let mut orders = vec![];
    let mut items = vec![];
    for i in 1..=64u64 {
        let mut total = 0.0;
        for _ in 0..r.range(1, 4) {
            let p = r.range(1, PRODUCTS.len() as u64) as usize;
            let q = r.range(1, 3);
            let price = PRODUCTS[p - 1].2;
            total += price * q as f64;
            items.push(vec![s(items.len() + 1), s(i), s(p), s(q), s(format!("{:.2}", price))]);
        }
        let customer = r.range(1, 48);
        let at = format!("2026-{:02}-{:02} {:02}:{:02}:{:02}", r.range(1, 10), r.range(1, 28), r.range(0, 23), r.range(0, 59), r.range(0, 59));
        let note = if i % 6 == 0 { s("Gift wrap please") } else { None };
        orders.push(vec![s(i), s(customer), s(at), s(r.pick(&STATUSES)), s(format!("{:.2}", total)), note]);
    }
    (orders, items)
}

fn employees(r: &mut Rng) -> Vec<Row> {
    (1..=18u64)
        .map(|i| {
            let name = format!("{} {}", r.pick(&FIRST_NAMES), r.pick(&LAST_NAMES));
            let dept = r.pick(&DEPARTMENTS);
            let title = r.pick(&TITLES);
            let hired = format!("20{:02}-{:02}-{:02}", r.range(15, 25), r.range(1, 12), r.range(1, 28));
            let salary = format!("{}.00", r.range(30, 120) * 1000);
            let manager = if i == 1 { None } else { s(r.range(1, i - 1)) };
            vec![s(i), s(name), s(dept), s(title), s(hired), s(salary), manager]
        })
        .collect()
}

fn page_views(r: &mut Rng) -> Vec<Row> {
    (1..=120u64)
        .map(|i| {
            let page = r.pick(&PAGES);
            let device = r.pick(&DEVICES);
            let ip = format!("{}.{}.{}.{}", r.range(10, 220), r.range(0, 255), r.range(0, 255), r.range(1, 254));
            let duration = r.range(200, 9000);
            let at = format!("2026-11-{:02} {:02}:{:02}:{:02}", r.range(1, 23), r.range(0, 23), r.range(0, 59), r.range(0, 59));
            vec![s(i), s(page), s(device), s(ip), s(duration), s(at)]
        })
        .collect()
}

fn daily_stats(r: &mut Rng) -> Vec<Row> {
    (1..=23u64)
        .map(|d| {
            let visits = r.range(300, 1500);
            let unique = r.range(80, 600);
            let bounce = r.range(1500, 6000) as f64 / 100.0;
            vec![s(format!("2026-11-{:02}", d)), s(visits), s(unique), s(format!("{:.2}", bounce))]
        })
        .collect()
}

fn daily_fields() -> Vec<FieldDef> {
    vec![f("day", "date", "", (true, true, false), ""), f("visits", "int", "", REQ, ""), f("unique_visitors", "int", "", REQ, ""), f("bounce_rate", "decimal", "5,2", OPT, "Percent")]
}

fn page_view_fields() -> Vec<FieldDef> {
    vec![
        f("id", "bigint", "", ID, ""),
        f("path", "varchar", "255", REQ, ""),
        f("device", "varchar", "20", OPT, ""),
        f("ip_address", "varchar", "45", OPT, ""),
        f("duration_ms", "int", "", OPT, "Time on page"),
        f("viewed_at", "datetime", "", REQ, ""),
    ]
}

fn customer_fields() -> Vec<FieldDef> {
    vec![
        f("id", "int", "11", ID, ""),
        f("first_name", "varchar", "50", REQ, ""),
        f("last_name", "varchar", "50", REQ, ""),
        f("email", "varchar", "120", REQ, "Login e-mail"),
        f("phone", "varchar", "30", OPT, ""),
        f("city", "varchar", "60", OPT, ""),
        f("country", "varchar", "60", OPT, ""),
        f("created_at", "datetime", "", REQ, ""),
    ]
}

fn employee_fields() -> Vec<FieldDef> {
    vec![
        f("id", "int", "11", ID, ""),
        f("full_name", "varchar", "100", REQ, ""),
        f("department", "varchar", "40", OPT, ""),
        f("title", "varchar", "40", OPT, ""),
        f("hire_date", "date", "", OPT, ""),
        f("salary", "decimal", "10,2", OPT, ""),
        f("manager_id", "int", "11", OPT, ""),
    ]
}

fn order_item_fields() -> Vec<FieldDef> {
    vec![f("id", "int", "11", ID, ""), f("order_id", "int", "11", REQ, ""), f("product_id", "int", "11", REQ, ""), f("quantity", "int", "11", REQ, ""), f("unit_price", "decimal", "10,2", REQ, "")]
}

fn order_fields() -> Vec<FieldDef> {
    vec![
        f("id", "int", "11", ID, ""),
        f("customer_id", "int", "11", REQ, ""),
        f("ordered_at", "datetime", "", REQ, ""),
        f("status", "varchar", "20", REQ, ""),
        f("total", "decimal", "10,2", REQ, ""),
        f("note", "text", "", OPT, ""),
    ]
}

fn product_fields() -> Vec<FieldDef> {
    vec![
        f("id", "int", "11", ID, ""),
        f("sku", "varchar", "20", REQ, ""),
        f("name", "varchar", "120", REQ, ""),
        f("category", "varchar", "40", OPT, ""),
        f("price", "decimal", "10,2", REQ, ""),
        f("stock", "int", "11", REQ, ""),
        f("active", "tinyint", "1", REQ, ""),
    ]
}
