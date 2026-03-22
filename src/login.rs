use std::env;
use std::fs;
use std::path::Path;

use thirtyfour::prelude::*;
use tokio::io::{self, AsyncBufReadExt, BufReader};

pub async fn login_and_capture_cookie() -> Result<String, Box<dyn std::error::Error>> {
    let login_url = env::var("LOGIN_URL").map_err(|_| "未在 .env 中配置 LOGIN_URL")?;

    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:9515", caps).await?;

    driver.goto(&login_url).await?;

    let current_url = driver.current_url().await?.to_string();
    let current_title = driver.title().await.unwrap_or_default();

    println!("已打开登录页：{}", login_url);
    println!("当前页面 URL：{}", current_url);
    println!("当前页面标题：{}", current_title);

    if current_url.contains("error.taobao.com") {
        driver.quit().await?;
        return Err(format!(
            "打开登录页后立即跳转到了淘宝错误页：{}。通常是风控、网络环境或 WebDriver 自动化特征触发导致的。",
            current_url
        )
        .into());
    }

    println!("请在浏览器里手动完成登录。");
    println!("登录成功后，确认页面不是错误页，再回到终端按一次回车继续...");

    let mut line = String::new();
    let mut reader = BufReader::new(io::stdin());
    reader.read_line(&mut line).await?;

    let after_login_url = driver.current_url().await?.to_string();
    let after_login_title = driver.title().await.unwrap_or_default();

    println!("登录后页面 URL：{}", after_login_url);
    println!("登录后页面标题：{}", after_login_title);

    if after_login_url.contains("error.taobao.com") {
        driver.quit().await?;
        return Err(format!(
            "登录后页面跳转到了淘宝错误页：{}。当前浏览器环境很可能被风控拦截，未能完成正常登录。",
            after_login_url
        )
        .into());
    }

    let cookies = driver.get_all_cookies().await?;

    if cookies.is_empty() {
        driver.quit().await?;
        return Err(format!(
            "没有读取到任何 cookie。当前页面 URL：{}。请确认你已在浏览器里成功登录。",
            after_login_url
        )
        .into());
    }

    let cookie_header = cookies
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ");

    println!("读取到 {} 个 cookie。", cookies.len());
    println!("COOKIE={}", cookie_header);

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

    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");

    let new_line = format!(r#"{}="{}""#, key, escaped);

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

    unsafe {
        env::set_var(key, value);
    }

    Ok(())
}
