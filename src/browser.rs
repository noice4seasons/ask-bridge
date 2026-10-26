use crate::cdp::{Cdp, endpoint};
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{
    fs::{self, File, OpenOptions},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MARKER: &str = "ask-bridge-dedicated-edge-v1\n";

pub struct Browser {
    pub cdp: Cdp,
    profile: PathBuf,
    _lease: File,
    child: Option<Child>,
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn prepare(profile: &Path) -> Result<(PathBuf, File)> {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let daily = PathBuf::from(local).join("Microsoft/Edge/User Data");
        if normalize(profile).starts_with(&normalize(&daily)) {
            bail!("不能把日常 Edge 数据目录用作工具专用目录");
        }
    }
    fs::create_dir_all(profile)?;
    let canonical = fs::canonicalize(profile)?;
    let profile = PathBuf::from(canonical.to_string_lossy().trim_start_matches(r"\\?\"));
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let daily = PathBuf::from(local).join("Microsoft/Edge/User Data");
        if normalize(&profile).starts_with(&normalize(&daily)) {
            bail!("专用目录不能指向日常 Edge 数据目录");
        }
    }
    let marker = profile.join(".ask-bridge-profile");
    if !marker.exists() {
        if fs::read_dir(&profile)?.next().is_some() {
            bail!("拒绝接管已有数据目录；请使用空的专用目录");
        }
        fs::write(&marker, MARKER)?;
    }
    if fs::read_to_string(&marker)? != MARKER {
        bail!("专用目录标记无效");
    }
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(profile.join(".ask-bridge-lock"))?;
    lease
        .try_lock()
        .context("另一个 Ask Bridge 任务正在使用此专用浏览器，请等待它完成")?;
    Ok((profile, lease))
}

fn edge_executable() -> Result<PathBuf> {
    for variable in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(variable) {
            let path = PathBuf::from(root).join("Microsoft/Edge/Application/msedge.exe");
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    bail!("找不到已安装的 Microsoft Edge")
}

fn reachable(profile: &Path) -> bool {
    endpoint(profile).is_ok_and(|(address, _)| {
        TcpStream::connect_timeout(&address, Duration::from_millis(150)).is_ok()
    })
}

pub fn login_instructions(profile: &Path) -> Result<()> {
    let (profile, _lease) = prepare(profile)?;
    let edge = edge_executable()?;
    println!(
        "请先关闭工具专用 Edge（保留日常 Edge），然后在 Win+R 运行以下命令，自行登录 Gemini；成功后关闭专用窗口，再执行 ask-bridge check-login。登录期间不启用调试或读取页面。"
    );
    println!(
        "\"{}\" --user-data-dir=\"{}\" https://gemini.google.com/app",
        edge.display(),
        profile.display()
    );
    Ok(())
}

impl Browser {
    pub fn start(profile: &Path) -> Result<Self> {
        let (profile, lease) = prepare(profile)?;
        let mut child = None;
        if !reachable(&profile) {
            // 仅移除自己目录里的过期端点文件，不触碰登录数据。
            let port_file = profile.join("DevToolsActivePort");
            if port_file.exists() {
                fs::remove_file(&port_file)?;
            }
            let mut command = Command::new(edge_executable()?);
            command.args([
                "--remote-debugging-port=0",
                "--remote-debugging-address=127.0.0.1",
                "--enable-automation",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-mode",
            ]);
            command
                .arg(format!("--user-data-dir={}", profile.display()))
                .arg("about:blank");
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000); // 不弹出控制台；正常显示交互式 Edge 窗口。
            }
            child = Some(command.spawn().context("启动专用 Edge 失败")?);
            let deadline = Instant::now() + Duration::from_secs(30);
            while !reachable(&profile) {
                if let Some(process) = child.as_mut() {
                    if let Some(status) = process.try_wait()? {
                        bail!(
                            "专用 Edge 提前退出：{status}；若普通登录窗口仍打开，请先关闭它再重试"
                        );
                    }
                }
                if Instant::now() >= deadline {
                    bail!("专用 Edge 未能开启本机调试端口");
                }
                thread::sleep(Duration::from_millis(200));
            }
        }
        let mut cdp = Cdp::connect_with_timeout(&profile, 5)?;
        // 关闭前的所有权依据：端点属于带专用标记的目录，浏览器实际启动参数也必须匹配。
        let info = cdp.call("Browser.getBrowserCommandLine", json!({}), None, 5)?;
        let owned = info["arguments"].as_array().is_some_and(|args| {
            args.iter().any(|arg| {
                arg.as_str()
                    .and_then(|s| s.strip_prefix("--user-data-dir="))
                    .is_some_and(|value| normalize(Path::new(value)) == normalize(&profile))
            })
        });
        if !owned {
            bail!("浏览器并非本工具专用实例，未接管或关闭它");
        }
        Ok(Self {
            cdp,
            profile,
            _lease: lease,
            child,
        })
    }

    pub fn close(&mut self) -> Result<()> {
        // Browser.close 仅可对上面验证过的专用实例调用，正常退出以落盘登录状态。
        let _ = self.cdp.call("Browser.close", json!({}), None, 5);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let process_exited = match self.child.as_mut() {
                Some(child) => child.try_wait()?.is_some(),
                None => true,
            };
            if !reachable(&self.profile) && process_exited {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("结果已保留，但专用 Edge 尚未完全退出；未强制终止浏览器");
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuse_daily_edge_profile_before_access() {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let daily = PathBuf::from(local).join("Microsoft/Edge/User Data");
            assert!(prepare(&daily).is_err());
            assert!(prepare(&daily.join("Default")).is_err());
        }
    }
    #[test]
    fn dedicated_profile_is_exclusive_and_persistent() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let (profile, lease) = prepare(&dir).unwrap();
        fs::write(profile.join("test-login-placeholder"), "preserve").unwrap();
        assert!(prepare(&dir).is_err());
        drop(lease);
        let (_, lease) = prepare(&dir).unwrap();
        assert_eq!(
            fs::read_to_string(profile.join("test-login-placeholder")).unwrap(),
            "preserve"
        );
        drop(lease);
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn refuse_unowned_nonempty_profile() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("unrelated"), "keep").unwrap();
        assert!(prepare(&dir).is_err());
        assert!(dir.join("unrelated").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
