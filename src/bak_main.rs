mod login;

use dotenvy::from_filename_override;
use reqwest::header::{COOKIE, USER_AGENT};
use rusqlite::{params, Connection};
use spider::website::Website;
use std::env;
use std::fs;

#[derive(Debug, Clone)]
struct Config {
    spider_base_url: String,
    cookie_verify_url: String,
    login_invalid_markers: Vec<String>,
    spider_limit: u32,
    sqlite_path: String,
    cookie: String,
}

impl Config {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let spider_base_url = required_env("SPIDER_BASE_URL")?;
        let cookie_verify_url = required_env("COOKIE_VERIFY_URL")?;
        let login_invalid_markers = parse_csv_env("LOGIN_INVALID_MARKERS");
        let spider_limit = env::var("SPIDER_LIMIT")
            .unwrap_or_else(|_| "20".to_string())
            .parse::<u32>()
            .map_err(|_| "SPIDER_LIMIT 必须是数字")?;
        let sqlite_path =
            env::var("SQLITE_PATH").unwrap_or_else(|_| "data/spider.db".to_string());
        let cookie = env::var("COOKIE").unwrap_or_default();

        Ok(Self {
            spider_base_url,
            cookie_verify_url,
            login_invalid_markers,
            spider_limit,
            sqlite_path,
            cookie,
        })
    }
}

fn required_env(key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = env::var(key)
        .map_err(|_| format!("缺少必填配置: {}，请在 .env 中设置", key))?;
    let value = value.trim().to_string();

    if value.is_empty() {
        return Err(format!("配置为空: {}，请在 .env 中设置有效值", key).into());
    }

    Ok(value)
}

fn parse_csv_env(key: &str) -> Vec<String> {
    env::var(key)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

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
            discovered_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'))
        );

        CREATE INDEX IF NOT EXISTS idx_links_source_url ON links(source_url);
        CREATE INDEX IF NOT EXISTS idx_links_discovered_at ON links(discovered_at);
        "#,
    )?;

    Ok(conn)
}

fn save_links_to_sqlite(
    conn: &mut Connection,
    source_url: &str,
    links: Vec<String>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let tx = conn.transaction()?;
    let mut inserted = 0usize;

    {
        let mut stmt = tx.prepare(
            r#"
            INSERT OR IGNORE INTO links (url, source_url, status)
            VALUES (?1, ?2, 'visited')
            "#,
        )?;

        for link in links {
            let affected = stmt.execute(params![link, source_url])?;
            if affected > 0 {
                inserted += 1;
            }
        }
    }

    tx.commit()?;
    Ok(inserted)
}

async fn cookie_is_valid(
    verify_url: &str,
    cookie: &str,
    invalid_markers: &[String],
) -> Result<bool, Box<dyn std::error::Error>> {
    if cookie.trim().is_empty() {
        return Ok(false);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let request = client
        .get(verify_url)
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        )
        .header(COOKIE, cookie)
        .build()?;

    println!("\n[VERIFY REQUEST]");
    println!("url: {}", request.url());
    println!("method: {}", request.method());

    for (k, v) in request.headers() {
        if k.as_str().eq_ignore_ascii_case("cookie") {
            let cookie_str = v.to_str().unwrap_or("");
            println!("{}: <len={}>", k, cookie_str.len());
        } else {
            println!("{}: {:?}", k, v);
        }
    }

    let resp = client.execute(request).await?;
    let status = resp.status();
    let final_url = resp.url().to_string();
    let body = resp.text().await?;

    println!("cookie verify status: {}", status);
    println!("cookie verify final url: {}", final_url);
    println!("cookie verify body length: {}", body.len());

    if !status.is_success() {
        return Ok(false);
    }

    let body_lower = body.to_lowercase();
    let final_url_lower = final_url.to_lowercase();

    for marker in invalid_markers {
        let marker = marker.trim();
        if marker.is_empty() {
            continue;
        }

        let marker_lower = marker.to_lowercase();
        if final_url_lower.contains(&marker_lower) || body_lower.contains(&marker_lower) {
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

    for (k, v) in request.headers() {
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
            if let Err(err) = login::save_env_value(".env", "COOKIE", &cookie) {
                eprintln!("写入 .env 失败: {}", err);
                return None;
            }

            println!("COOKIE 已写入 .env");
            Some(cookie)
        }
        Err(err) => {
            eprintln!("获取 cookie 失败: {}", err);
            None
        }
    }
}

async fn ensure_cookie(config: &mut Config) -> bool {
    if config.cookie.trim().is_empty() {
        println!("COOKIE 为空。");
        match refresh_cookie().await {
            Some(cookie) => {
                config.cookie = cookie;
                true
            }
            None => false,
        }
    } else {
        println!("检测到已有 COOKIE，先验证是否有效...");

        match cookie_is_valid(
            &config.cookie_verify_url,
            &config.cookie,
            &config.login_invalid_markers,
        )
        .await
        {
            Ok(true) => {
                println!("已有 COOKIE 有效，直接继续。");
                true
            }
            Ok(false) => {
                println!("已有 COOKIE 无效，重新获取...");
                match refresh_cookie().await {
                    Some(cookie) => {
                        config.cookie = cookie;
                        true
                    }
                    None => false,
                }
            }
            Err(err) => {
                eprintln!("验证 COOKIE 时出错: {}", err);
                println!("尝试重新获取新的 COOKIE...");
                match refresh_cookie().await {
                    Some(cookie) => {
                        config.cookie = cookie;
                        true
                    }
                    None => false,
                }
            }
        }
    }
}

async fn run_spider(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    print_cookie_summary(&config.cookie);

    if let Err(err) = debug_fetch(&config.cookie_verify_url, &config.cookie).await {
        eprintln!("debug fetch failed: {}", err);
    }

    let mut website = Website::new(&config.spider_base_url);

    website
        .with_limit(config.spider_limit)
        .with_cookies(&config.cookie);

    println!("[SPIDER] cookies configured, len={}", config.cookie.len());

    let mut website = website.build().expect("build website failed");
    website.crawl().await;

    let links: Vec<String> = website
        .get_links()
        .iter()
        .map(|link| link.to_string())
        .collect();

    println!("done");
    println!("total links found: {}", links.len());

    let mut conn = init_db(&config.sqlite_path)?;
    let inserted = save_links_to_sqlite(&mut conn, &config.spider_base_url, links)?;

    println!("[SQLITE] saved successfully");
    println!("[SQLITE] newly inserted: {}", inserted);

    Ok(())
}

#[tokio::main]
async fn main() {
    from_filename_override(".env").ok();

    let mut config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("读取配置失败: {}", err);
            return;
        }
    };

    println!("start url: {}", config.spider_base_url);
    println!("limit: {}", config.spider_limit);
    println!("sqlite path: {}", config.sqlite_path);
    println!("cookie verify url: {}", config.cookie_verify_url);
    println!("invalid markers count: {}", config.login_invalid_markers.len());

    if !ensure_cookie(&mut config).await {
        return;
    }

    if let Err(err) = run_spider(&config).await {
        eprintln!("spider run failed: {}", err);
    }
}
