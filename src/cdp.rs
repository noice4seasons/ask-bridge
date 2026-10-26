use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{fs, net::{SocketAddr, TcpStream}, path::Path, time::Duration};
use tungstenite::{Message, WebSocket, client};

pub struct Cdp { socket: WebSocket<TcpStream>, sequence: u64, pub calls: u64 }

pub fn endpoint(profile: &Path) -> Result<(SocketAddr, String)> {
    let text = fs::read_to_string(profile.join("DevToolsActivePort"))
        .context("无法读取 Edge 调试连接；请在 edge://inspect 开启远程调试")?;
    let mut lines = text.lines();
    let port: u16 = lines.next().context("缺少端口")?.trim().parse()?;
    let route = lines.next().context("缺少浏览器端点")?.trim();
    let id = route.strip_prefix("/devtools/browser/").context("无效浏览器端点")?;
    if port == 0 || id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { bail!("无效调试端点"); }
    Ok((SocketAddr::from(([127,0,0,1],port)), format!("ws://127.0.0.1:{port}{route}")))
}

impl Cdp {
    pub fn connect(profile: &Path) -> Result<Self> {
        let (address, url) = endpoint(profile)?;
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5)).context("无法连接 Edge，请确认它正在运行且调试仍开启")?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(15)))?;
        let (socket, _) = client(url.as_str(), stream).map_err(|e| anyhow::anyhow!("Edge 调试握手失败：{e}"))?;
        Ok(Self { socket, sequence: 0, calls: 0 })
    }
    pub fn call(&mut self, method: &str, params: Value, session: Option<&str>, timeout: u64) -> Result<Value> {
        self.sequence += 1; self.calls += 1;
        let id = self.sequence;
        let mut request = json!({"id":id,"method":method,"params":params});
        if let Some(session) = session { request["sessionId"] = json!(session); }
        self.socket.get_mut().set_read_timeout(Some(Duration::from_secs(timeout)))?;
        self.socket.send(Message::Text(request.to_string().into()))?;
        loop {
            match self.socket.read().context("Edge 连接中断或等待超时；不会自动重发请求")? {
                Message::Text(text) => {
                    let response: Value = serde_json::from_str(&text)?;
                    if response["id"].as_u64() == Some(id) {
                        if !response["error"].is_null() { bail!("{}", response["error"]["message"]); }
                        return Ok(response["result"].take());
                    }
                }
                Message::Ping(_) => { self.socket.flush()?; }
                Message::Close(_) => bail!("Edge 已断开连接；请用 get 恢复，不要重新提交"),
                _ => {}
            }
        }
    }
    pub fn tabs(&mut self) -> Result<Vec<Value>> {
        let result = self.call("Target.getTargets",json!({}),None,15)?;
        Ok(result["targetInfos"].as_array().context("无分页面信息")?.iter()
            .filter(|t| t["type"] == "page" && is_gemini(t["url"].as_str().unwrap_or_default())).cloned().collect())
    }
    pub fn attach(&mut self, target: &str) -> Result<String> {
        if !self.tabs()?.iter().any(|t| t["targetId"] == target) { bail!("指定分页面不是 Gemini"); }
        Ok(self.call("Target.attachToTarget",json!({"targetId":target,"flatten":true}),None,15)?["sessionId"].as_str().context("无法附加分页面")?.to_owned())
    }
    pub fn eval(&mut self, session: &str, function: &str, args: Value, timeout: u64) -> Result<Value> {
        let expression = format!("({function})({args})");
        let value = self.call("Runtime.evaluate",json!({"expression":expression,"returnByValue":true,"awaitPromise":true}),Some(session),timeout)?;
        if !value["exceptionDetails"].is_null() { bail!("{}",value["exceptionDetails"]["exception"]["description"].as_str().unwrap_or("页面脚本异常")); }
        Ok(value["result"]["value"].clone())
    }
    pub fn create(&mut self, url: &str) -> Result<String> {
        if !is_gemini(url) { bail!("只允许 Gemini 网址"); }
        Ok(self.call("Target.createTarget",json!({"url":url,"background":true}),None,15)?["targetId"].as_str().context("无法创建分页面")?.to_owned())
    }
}
impl Drop for Cdp { fn drop(&mut self) { let _ = self.socket.close(None); } }
pub fn is_gemini(url: &str) -> bool { url.starts_with("https://gemini.google.com/") }

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn reject_foreign_origin() { assert!(is_gemini("https://gemini.google.com/app")); assert!(!is_gemini("https://gemini.google.com.evil/app")); assert!(!is_gemini("http://gemini.google.com/app")); }
    #[test] fn reject_nonlocal_endpoint_in_file() {
        let dir=std::env::temp_dir().join(uuid::Uuid::new_v4().to_string()); fs::create_dir(&dir).unwrap();
        fs::write(dir.join("DevToolsActivePort"),"9222\n/devtools/browser/foo?remote=evil").unwrap();
        assert!(endpoint(&dir).is_err()); fs::remove_file(dir.join("DevToolsActivePort")).unwrap(); fs::remove_dir(dir).unwrap();
    }
}
