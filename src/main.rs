mod login;

use chrono::Utc;
use dotenvy::dotenv;
use reqwest::header::{COOKIE, USER_AGENT};
use rusqlite::{params, Connection};
use spider::tokio;
use spider::website::Website;
use std::env;
use std::fs;

fn print_cookie_summary(cookie: &str) {
    let trimmed = cookie.trim();
    println!("cookie ready, len = {}", trimmed.len());

    let parts: Vec<&str> = trimmed
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    println!("cookie pairs: {}", parts.len());

    for part in parts.iter().take(8) {
        let key = part.split('=').next().unwrap_or("");
        println!("cookie key: {}", key);
    }

    if parts.len() > 8 {
        println!("... {} more cookie keys", parts.len() - 8);
    }
}

fn init_db(db_path: &str) -> Result<Connection, Box<dyn std::error::Error>> {
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL UNIQUE,
            source_url TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'visited',
            discovered_at TEXT NOT NULL
        );
        "#,
    )?;

    Ok(conn)
}

fn save_links_to_sqlite(
    conn: &mut Connection,
    source_url: &str,
    links: Vec<String>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction()?;

    let mut inserted = 0usize;

    {
        let mut stmt = tx.prepare(
            r#"
            INSERT OR IGNORE INTO links (url, source_url, status, discovered_at)
            VALUES (?1, ?2, 'visited', ?3)
            "#,
        )?;

        for link in links {
            let affected = stmt.execute(params![link, source_url, now])?;
            if affected > 0 {
                inserted += 1;
            }
        }
    }

    tx.commit()?;
    Ok(inserted)
}

async fn verify_cookie(cookie: &str) -> Result<bool, Box<dyn std::error::Error>> {
    if cookie.trim().is_empty() {
        return Ok(false);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let request = client
        .get("https://www.jd.com")
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        )
        .header(COOKIE, cookie)
        .build()?;

    println!("\n[VERIFY REQUEST]");
    println!("url: {}", request.url());
    println!("method: {}", request.method());

    for (k, v) in request.headers().iter() {
        if k.as_str().eq_ignore_ascii_case("cookie") {
            let cookie_str = v.to_str().unwrap_or("");
            println!("{}: <len={}>", k, cookie_str.len());
        } else {
            println!("{}: {:?}", k, v);
        }
    }

    let resp = client.execute(request).await?;

    let status = resp.status();
    let body = resp.text().await?;

    println!("cookie verify status: {}", status);
    println!("cookie verify body length: {}", body.len());

    if !status.is_success() {
        return Ok(false);
    }

    let invalid_markers = ["passport.jd.com", "请登录", "登录"];
    let body_lower = body.to_lowercase();

    for marker in invalid_markers {
        if body.contains(marker) || body_lower.contains(&marker.to_lowercase()) {
            return Ok(false);
        }
    }

    Ok(true)
}

async fn debug_fetch(url: &str, cookie: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let request = client
        .get(url)
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        )
        .header(COOKIE, cookie)
        .build()?;

    println!("\n[DEBUG FETCH REQUEST]");
    println!("url: {}", request.url());
    println!("method: {}", request.method());

    for (k, v) in request.headers().iter() {
        if k.as_str().eq_ignore_ascii_case("cookie") {
            let cookie_str = v.to_str().unwrap_or("");
            println!("{}: <len={}>", k, cookie_str.len());
        } else {
            println!("{}: {:?}", k, v);
        }
    }

    let resp = client.execute(request).await?;
    let status = resp.status();

    println!("[DEBUG FETCH RESPONSE] {} -> {}", url, status);

    Ok(())
}

async fn refresh_cookie() -> Option<String> {
    println!("开始打开浏览器获取新的 cookie...");

    match login::login_and_capture_cookie().await {
        Ok(cookie) => {
            if let Err(err) = login::save_env_value(".env", "JD_COOKIE", &cookie) {
                eprintln!("写入 .env 失败: {}", err);
                return None;
            }

            println!("JD_COOKIE 已写入 .env");
            Some(cookie)
        }
        Err(err) => {
            eprintln!("获取 cookie 失败: {}", err);
            None
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let base_url = env::var("SPIDER_BASE_URL")
        .expect("SPIDER_BASE_URL not set in .env");

    let limit: u32 = env::var("SPIDER_LIMIT")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .expect("SPIDER_LIMIT must be a number");

    let db_path = env::var("SQLITE_PATH")
        .unwrap_or_else(|_| "data/spider.db".to_string());

    let jd_username = env::var("JD_USERNAME").ok();
    let jd_password = env::var("JD_PASSWORD").ok();
    let mut jd_cookie = env::var("JD_COOKIE").unwrap_or_default();

    println!("start url: {}", base_url);
    println!("limit: {}", limit);
    println!("sqlite path: {}", db_path);

    if let Some(username) = &jd_username {
        if !username.is_empty() {
            println!("username loaded");
        }
    }

    if let Some(password) = &jd_password {
        if !password.is_empty() {
            println!("password loaded");
        }
    }

    if jd_cookie.trim().is_empty() {
        println!("JD_COOKIE 为空。");
        match refresh_cookie().await {
            Some(cookie) => jd_cookie = cookie,
            None => return,
        }
    } else {
        println!("检测到已有 JD_COOKIE，先验证是否有效...");

        match verify_cookie(&jd_cookie).await {
            Ok(true) => {
                println!("已有 JD_COOKIE 有效，直接继续。");
            }
            Ok(false) => {
                println!("已有 JD_COOKIE 无效，重新获取...");
                match refresh_cookie().await {
                    Some(cookie) => jd_cookie = cookie,
                    None => return,
                }
            }
            Err(err) => {
                eprintln!("验证 JD_COOKIE 时出错: {}", err);
                println!("尝试重新获取新的 JD_COOKIE...");
                match refresh_cookie().await {
                    Some(cookie) => jd_cookie = cookie,
                    None => return,
                }
            }
        }
    }

    print_cookie_summary(&jd_cookie);

    if let Err(err) = debug_fetch("https://www.jd.com", &jd_cookie).await {
        eprintln!("debug fetch failed: {}", err);
    }

    let mut website = Website::new(&base_url);

    website
        .with_limit(limit)
        .with_cookies(&jd_cookie);

    println!("[SPIDER] cookies configured, len={}", jd_cookie.len());

    let mut website = website
        .build()
        .expect("build website failed");

    let mut rx = website.subscribe(0).expect("subscribe failed");

    let handle = tokio::spawn(async move {
        while let Ok(page) = rx.recv().await {
            println!("[VISITED] {}", page.get_url());
        }
    });

    website.crawl().await;

    website.unsubscribe();
    let _ = handle.await;

    let links: Vec<String> = website
        .get_links()
        .iter()
        .map(|link| link.to_string())
        .collect();

    println!("done");
    println!("total links found: {}", links.len());

    let mut conn = match init_db(&db_path) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("init sqlite failed: {}", err);
            return;
        }
    };

    match save_links_to_sqlite(&mut conn, &base_url, links) {
        Ok(inserted) => {
            println!("[SQLITE] saved successfully");
            println!("[SQLITE] newly inserted: {}", inserted);
        }
        Err(err) => {
            eprintln!("save links to sqlite failed: {}", err);
        }
    }
}
