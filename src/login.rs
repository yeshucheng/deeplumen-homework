use std::env;
use std::fs;
use std::path::Path;

use thirtyfour::prelude::*;
use tokio::io::{self, AsyncBufReadExt, BufReader};

pub async fn login_and_capture_cookie() -> Result<String, Box<dyn std::error::Error>> {
    let login_url = env::var("LOGIN_URL")
        .unwrap_or_else(|_| "https://passport.jd.com/new/login.aspx".to_string());

    // Chrome WebDriver 能力
    let caps = DesiredCapabilities::chrome();

    // 默认连本地 chromedriver
    // 先在另一个终端启动：
    // chromedriver --port=9515
    let driver = WebDriver::new("http://localhost:9515", caps).await?;

    // 打开登录页
    driver.goto(&login_url).await?;

    println!("已打开登录页：{}", login_url);
    println!("请在浏览器里手动完成登录。");
    println!("登录成功后，回到终端按一次回车继续...");

    // 等你手动按回车
    let mut line = String::new();
    let mut reader = BufReader::new(io::stdin());
    reader.read_line(&mut line).await?;

    // 尝试跳到主站，确保能读到 jd.com 域下的 cookie
    driver.goto("https://jd.com").await?;

    // 读取全部 cookie
    let cookies = driver.get_all_cookies().await?;

    if cookies.is_empty() {
        driver.quit().await?;
        return Err("没有读取到任何 cookie。请确认你已在浏览器里成功登录。".into());
    }

    // 拼成 .env 里常用的 Cookie Header 形式
    let cookie_header = cookies
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ");

    println!("读取到 {} 个 cookie。", cookies.len());
    println!("JD_COOKIE={}", cookie_header);

    driver.quit().await?;

    Ok(cookie_header)
}

pub fn save_env_value<P: AsRef<Path>>(
    env_path: P,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let env_path = env_path.as_ref();

    let mut lines = if env_path.exists() {
        fs::read_to_string(env_path)?
            .lines()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let new_line = format!(r#"{}={}"#, key, value.replace('"', "\\\""));

    let mut replaced = false;
    for line in &mut lines {
        if line.starts_with(&format!("{}=", key)) {
            *line = new_line.clone();
            replaced = true;
            break;
        }
    }

    if !replaced {
        lines.push(new_line);
    }

    let mut out = lines.join("\n");
    out.push('\n');
    fs::write(env_path, out)?;

    Ok(())
}
