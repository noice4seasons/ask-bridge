mod browser;
mod cdp;
mod text;
use anyhow::{Context, Result, bail};
use cdp::Cdp;
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PAGE: &str = include_str!("page.js");
const WAIT: &str = include_str!("wait.js");
const MARKDOWN: &str = include_str!("markdown.js");
const EXTRACT: &str = include_str!("extract.js");

#[derive(Parser)]
#[command(
    version,
    about = "Gemini Edge Lite：单执行文件，复用 Edge 登录，按需处理中文及 Markdown"
)]
struct Cli {
    #[arg(long, global = true)]
    profile: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// 打开专用 Edge 等待用户登录，验证模型后正常退出并保留登录
    Login {
        #[arg(long, default_value_t = 900)]
        timeout: u64,
    },
    /// 检查专用 Edge 连接；完成后退出，不提交文字
    Doctor,
    /// 查看网页当前可用的模型及思考程度
    Models,
    /// 保存实际可用的模型和思考程度
    Config {
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        reasoning: Option<String>,
    },
    /// 仅在用户明确要求 Gemini 时运行；原稿先保存
    Run {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "proofread")]
        mode: String,
        #[arg(long, default_value = "")]
        instruction: String,
        #[arg(long, default_value = "codeblock")]
        format: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        reasoning: Option<String>,
        #[arg(long,default_value_t=300,value_parser=clap::value_parser!(u64).range(1..=3600))]
        timeout: u64,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        print: bool,
    },
    /// 恢复原工作，只取回，不重新提交
    Get {
        #[arg(long)]
        job: PathBuf,
        #[arg(long,default_value_t=300,value_parser=clap::value_parser!(u64).range(1..=3600))]
        timeout: u64,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        print: bool,
    },
    /// 在工具自身创建的离线页面测试 Markdown 提取
    SelfTest,
}
fn env_path(name: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os(name).with_context(|| format!("缺少环境变量 {name}"))?,
    ))
}
fn config_path() -> Result<PathBuf> {
    Ok(env_path("USERPROFILE")?.join(".config/ask-bridge-lite/config.json"))
}
fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}
fn write_json(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}
fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .with_context(|| format!("缺少字段 {key}"))
}
fn state(cdp: &mut Cdp, session: &str) -> Result<Value> {
    cdp.eval(session, PAGE, json!({"action":"status"}), 15)
}
fn wait_state(cdp: &mut Cdp, session: &str, predicate: impl Fn(&Value) -> bool) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let value = state(cdp, session)?;
        if value["authRequired"] == true {
            bail!("Gemini 需要登录，请在 Edge 自行完成");
        }
        if predicate(&value) {
            return Ok(value);
        }
        if Instant::now() > deadline {
            bail!("等待 Gemini 页面控件超时");
        }
        thread::sleep(Duration::from_millis(300));
    }
}
fn click(cdp: &mut Cdp, session: &str, action: &str, label: &str) -> Result<()> {
    let point = cdp.eval(session, PAGE, json!({"action":action,"model":label}), 15)?;
    for event in ["mousePressed", "mouseReleased"] {
        cdp.call(
            "Input.dispatchMouseEvent",
            json!({"type":event,"button":"left","clickCount":1,"x":point["x"],"y":point["y"]}),
            Some(session),
            15,
        )?;
    }
    Ok(())
}
fn escape(cdp: &mut Cdp, session: &str) -> Result<()> {
    for event in ["keyDown", "keyUp"] {
        cdp.call(
            "Input.dispatchKeyEvent",
            json!({"type":event,"key":"Escape","code":"Escape","windowsVirtualKeyCode":27}),
            Some(session),
            15,
        )?;
    }
    Ok(())
}
fn open_picker(cdp: &mut Cdp, session: &str) -> Result<Value> {
    let current = wait_state(cdp, session, |s| s["ready"] == true)?;
    if current["options"].as_array().is_none_or(|a| a.is_empty()) {
        click(cdp, session, "picker", "")?;
    }
    wait_state(cdp, session, |s| {
        s["options"].as_array().is_some_and(|a| !a.is_empty())
    })
}
fn select(cdp: &mut Cdp, session: &str, model: &str, reasoning: &str) -> Result<Value> {
    cdp.call("Page.bringToFront", json!({}), Some(session), 15)?;
    open_picker(cdp, session)?;
    click(cdp, session, "model", model)?;
    let selected = wait_state(cdp, session, |s| {
        s["options"].as_array().is_some_and(|a| a.is_empty())
    })?;
    if !reasoning.is_empty() {
        open_picker(cdp, session)?;
        click(cdp, session, "reasoning", "Thinking level")?;
        wait_state(cdp, session, |s| {
            s["reasoningOptions"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
        })?;
        click(cdp, session, "reasoning", reasoning)?;
        // 重新展开并验证选中标记，不能把成功点击视为成功选择。
        escape(cdp, session)?;
        escape(cdp, session)?;
        open_picker(cdp, session)?;
        click(cdp, session, "reasoning", "Thinking level")?;
        let verified = wait_state(cdp, session, |s| {
            s["selectedReasoning"]
                .as_str()
                .is_some_and(|v| v.eq_ignore_ascii_case(reasoning))
        })?;
        escape(cdp, session)?;
        escape(cdp, session)?;
        return Ok(
            json!({"requested":model,"displayed":selected["model"],"reasoning":verified["selectedReasoning"]}),
        );
    }
    Ok(json!({"requested":model,"displayed":selected["model"]}))
}
fn metrics() -> Value {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Foundation::FILETIME,
            System::{
                ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
                Threading::{GetCurrentProcess, GetProcessTimes},
            },
        };
        let process = GetCurrentProcess();
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of_val(&counters) as u32;
        GetProcessMemoryInfo(
            process,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        );
        let mut create: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        GetProcessTimes(process, &mut create, &mut exit, &mut kernel, &mut user);
        let time = |v: FILETIME| ((v.dwHighDateTime as u64) << 32) | v.dwLowDateTime as u64;
        return json!({"workingSetBytes":counters.WorkingSetSize,"peakWorkingSetBytes":counters.PeakWorkingSetSize,"processCpuMs":(time(kernel)+time(user)) as f64/10000.0});
    }
    #[cfg(not(windows))]
    json!({})
}
struct Work {
    dir: PathBuf,
    data: Value,
}
impl Work {
    fn save(&self) -> Result<()> {
        write_json(&self.dir.join("job.json"), &self.data)
    }
    fn finish(
        &mut self,
        cdp: &mut Cdp,
        session: &str,
        timeout: u64,
        output: Option<&Path>,
        print: bool,
        started: Instant,
    ) -> Result<()> {
        self.data["status"] = json!("waiting");
        self.save()?;
        let mut args = self.data.clone();
        args["timeout"] = json!(timeout * 1000);
        let complete = cdp.eval(session, WAIT, args, timeout + 5)?;
        let script = EXTRACT.replace("__MARKDOWN_FUNCTION__", MARKDOWN);
        let captured = cdp.eval(session, &script, self.data.clone(), 15)?;
        let raw = field(&captured, "raw")?;
        fs::write(self.dir.join("response.raw.md"), raw)?;
        let result = text::unwrap(raw, field(&self.data, "begin")?, field(&self.data, "end")?)?;
        let original = fs::read_to_string(self.dir.join("original.md"))?;
        let audit = text::audit(&original, &result);
        fs::write(self.dir.join("result.md"), &result)?;
        write_json(&self.dir.join("audit.json"), &audit)?;
        self.data["status"] = json!(if audit["passed"] == true {
            "complete"
        } else {
            "review_required"
        });
        self.data["url"] = captured["url"].clone();
        self.data["completedAt"] = json!(timestamp());
        self.data["resultHash"] = json!(text::hash(&result));
        let mut measured = metrics();
        measured["wallMs"] = json!(started.elapsed().as_millis());
        measured["cdpCalls"] = json!(cdp.calls);
        measured["pageChecks"] = complete["evaluations"].clone();
        measured["resultCharacters"] = json!(result.chars().count());
        self.data["metrics"] = measured;
        self.save()?;
        if let Some(output) = output {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)
                .context("输出文件已存在或不可写；工作结果已保留")?;
            file.write_all(result.as_bytes())?;
        }
        // 只关闭本工具创建、仍停留在本次对话的标签，避免后台页面累积。
        if self.data["ownsTarget"] == true {
            if let Ok(tabs) = cdp.tabs() {
                if tabs.iter().any(|tab| {
                    tab["targetId"] == self.data["targetId"] && tab["url"] == self.data["url"]
                }) {
                    let closed = cdp.call(
                        "Target.closeTarget",
                        json!({"targetId":self.data["targetId"]}),
                        None,
                        15,
                    );
                    self.data["tabClosed"] = json!(closed.is_ok());
                    self.save()?;
                }
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"status":self.data["status"],"model":self.data["selectedModel"],"url":self.data["url"],"job":self.dir,"result":self.dir.join("result.md"),"warnings":audit["warnings"],"metrics":self.data["metrics"]})
            )?
        );
        if print {
            print!("{result}");
        }
        Ok(())
    }
    fn record_error(&mut self, cdp: &mut Cdp, error: &anyhow::Error) {
        self.data["lastError"] = json!(error.to_string());
        if let Ok(tabs) = cdp.tabs() {
            if let Some(tab) = tabs.iter().find(|t| t["targetId"] == self.data["targetId"]) {
                self.data["url"] = tab["url"].clone();
            }
        }
        let _ = self.save();
        eprintln!("工作记录：{}", self.dir.display());
        if self.data["before"].is_u64() {
            eprintln!("请用 get --job 恢复取回，不会重新提交。");
        } else {
            eprintln!("尚未提交，请先修正连接或模型问题。");
        }
    }
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    let started = Instant::now();
    let config_file = config_path()?;
    let mut config = match read_json(&config_file) {
        Ok(v) => v,
        Err(e) if !config_file.exists() => {
            let _ = e;
            json!({})
        }
        Err(e) => return Err(e),
    };
    if let Command::Config { model, reasoning } = &cli.command {
        if model.is_some() || reasoning.is_some() || cli.profile.is_some() {
            fs::create_dir_all(config_file.parent().unwrap())?;
            if config_file.exists() {
                fs::copy(
                    &config_file,
                    config_file.with_extension(format!("{}.bak", timestamp())),
                )?;
            }
            if let Some(model) = model {
                config["model"] = json!(model);
            }
            if let Some(reasoning) = reasoning {
                config["reasoning"] = json!(reasoning);
            }
            if let Some(profile) = &cli.profile {
                config["profile"] = json!(profile);
            }
            write_json(&config_file, &config)?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"configPath":config_file,"config":config}))?
        );
        return Ok(());
    }
    let profile = cli
        .profile
        .or_else(|| config["profile"].as_str().map(PathBuf::from))
        .unwrap_or(env_path("LOCALAPPDATA")?.join("Programs/AskBridge/edge-profile"));
    let mut browser = browser::Browser::start(&profile)?;
    let result = execute(cli.command, &config, &mut browser.cdp, started);
    if result.is_ok() {
        browser.close()?;
        eprintln!("专用 Edge 已正常退出，登录数据保留。");
    } else {
        eprintln!("专用 Edge 已保留供登录或恢复；下一次命令会复用它，不自动重新发送。");
    }
    result
}
fn execute(command: Command, config: &Value, mut cdp: &mut Cdp, started: Instant) -> Result<()> {
    match command {
        Command::Login { timeout } => {
            let target = cdp.create("https://gemini.google.com/app")?;
            if let Ok(session) = cdp.attach(&target) {
                cdp.call("Page.bringToFront", json!({}), Some(&session), 5)?;
            }
            let deadline = Instant::now() + Duration::from_secs(timeout);
            eprintln!(
                "请在新开的专用 Edge 窗口中登录 Gemini。只检查 Gemini 就绪状态，不读取登录凭据。"
            );
            loop {
                if let Ok(session) = cdp.attach(&target) {
                    if let Ok(status) = state(cdp, &session) {
                        if status["ready"] == true {
                            let model = field(config, "model")?;
                            let reasoning = config["reasoning"].as_str().unwrap_or("");
                            let selected = select(cdp, &session, model, reasoning)?;
                            println!("{}", json!({"loggedIn":true,"selectedModel":selected}));
                            break;
                        }
                    }
                }
                if Instant::now() >= deadline {
                    bail!("等待登录超时；窗口已保留，完成登录后重新执行 login 验证");
                }
                thread::sleep(Duration::from_secs(2));
            }
        }
        Command::Doctor => {
            let version = cdp.call("Browser.getVersion", json!({}), None, 15)?;
            if cdp.tabs()?.is_empty() {
                let target = cdp.create("https://gemini.google.com/app")?;
                let session = cdp.attach(&target)?;
                wait_state(cdp, &session, |s| s["ready"] == true)?;
            }
            let mut states = Vec::new();
            for tab in cdp.tabs()? {
                let session = cdp.attach(field(&tab, "targetId")?)?;
                states.push(state(&mut cdp, &session)?);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &json!({"connected":true,"browser":version["product"],"gemini":states,"config":config,"wallMs":started.elapsed().as_millis(),"metrics":metrics(),"cdpCalls":cdp.calls})
                )?
            );
        }
        Command::Models => {
            let tabs = cdp.tabs()?;
            let target = match tabs.last() {
                Some(t) => field(t, "targetId")?.to_owned(),
                None => cdp.create("https://gemini.google.com/app")?,
            };
            let session = cdp.attach(&target)?;
            cdp.call("Page.bringToFront", json!({}), Some(&session), 15)?;
            let opened = open_picker(&mut cdp, &session)?;
            println!("{}", serde_json::to_string_pretty(&opened)?);
            escape(&mut cdp, &session)?;
        }
        Command::Run {
            input,
            mode,
            instruction,
            format,
            model,
            reasoning,
            timeout,
            output,
            print,
        } => {
            let model = model
                .or_else(|| config["model"].as_str().map(str::to_owned))
                .context("请先用 models 查看可用选项，再用 config --model 保存")?;
            let reasoning = reasoning
                .or_else(|| config["reasoning"].as_str().map(str::to_owned))
                .unwrap_or_default();
            let original = fs::read_to_string(input).context("原稿必须是 UTF-8 文件")?;
            let original = original.trim_start_matches('\u{feff}');
            if original.trim().is_empty() {
                bail!("原稿为空");
            }
            let id = uuid::Uuid::new_v4().to_string();
            let (prompt, begin, end) = text::prompt(original, &mode, &instruction, &format, &id)?;
            let dir = env_path("LOCALAPPDATA")?
                .join("Programs/AskBridge/jobs")
                .join(&id);
            fs::create_dir_all(&dir)?;
            fs::write(dir.join("original.md"), original)?;
            fs::write(dir.join("prompt.txt"), &prompt)?;
            let mut work = Work {
                dir,
                data: json!({"id":id,"status":"prepared","createdAt":timestamp(),"model":model,"reasoning":reasoning,"mode":mode,"format":format,"begin":begin,"end":end,"originalHash":text::hash(original),"inputCharacters":original.chars().count(),"promptCharacters":prompt.chars().count()}),
            };
            work.save()?;
            let result = (|| -> Result<()> {
                let target = cdp.create("https://gemini.google.com/app")?;
                work.data["targetId"] = json!(target);
                work.data["ownsTarget"] = json!(true);
                work.save()?;
                let session = cdp.attach(&target)?;
                work.data["selectedModel"] = select(&mut cdp, &session, &model, &reasoning)?;
                cdp.eval(&session, PAGE, json!({"action":"focus"}), 15)?;
                cdp.call(
                    "Input.insertText",
                    json!({"text":prompt}),
                    Some(&session),
                    15,
                )?;
                let draft = cdp.eval(&session, PAGE, json!({"action":"draft"}), 15)?;
                let normalize = |s: &str| {
                    s.replace("\r\n", "\n")
                        .replace('\u{a0}', " ")
                        .trim()
                        .to_owned()
                };
                if normalize(field(&draft, "text")?) != normalize(&prompt) {
                    bail!("编辑器文字不完整；未提交");
                }
                work.data["before"] = draft["responses"].clone();
                work.data["status"] = json!("submission_pending");
                work.save()?;
                click(&mut cdp, &session, "send", "")?;
                work.data["status"] = json!("submitted");
                work.data["submittedAt"] = json!(timestamp());
                work.save()?;
                work.finish(
                    &mut cdp,
                    &session,
                    timeout,
                    output.as_deref(),
                    print,
                    started,
                )
            })();
            if let Err(e) = result {
                work.record_error(&mut cdp, &e);
                return Err(e);
            }
        }
        Command::Get {
            job,
            timeout,
            output,
            print,
        } => {
            let mut work = Work {
                data: read_json(&job.join("job.json"))?,
                dir: job,
            };
            if !work.data["before"].is_u64() {
                bail!("此工作尚未进入提交阶段");
            }
            let result = (|| -> Result<()> {
                let mut target = field(&work.data, "targetId")?.to_owned();
                if !cdp.tabs()?.iter().any(|t| t["targetId"] == target) {
                    let url = field(&work.data, "url")?;
                    if !url.starts_with("https://gemini.google.com/app/") {
                        bail!("原分页面已关闭，且没有可恢复的对话网址");
                    }
                    target = cdp.create(url)?;
                    work.data["targetId"] = json!(target);
                    work.data["ownsTarget"] = json!(true);
                    work.save()?;
                }
                let session = cdp.attach(&target)?;
                wait_state(&mut cdp, &session, |s| s["ready"] == true)?;
                work.finish(
                    &mut cdp,
                    &session,
                    timeout,
                    output.as_deref(),
                    print,
                    started,
                )
            })();
            if let Err(e) = result {
                work.record_error(&mut cdp, &e);
                return Err(e);
            }
        }
        Command::SelfTest => self_test(&mut cdp)?,
        Command::Config { .. } => unreachable!(),
    }
    Ok(())
}
fn self_test(cdp: &mut Cdp) -> Result<()> {
    // 唯一例外：仅创建并关闭本工具的离线测试页面，不读取任何用户页面。
    let result = cdp.call(
        "Target.createTarget",
        json!({"url":"about:blank","background":true}),
        None,
        15,
    )?;
    let target = field(&result, "targetId")?;
    let session = cdp.call(
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
        None,
        15,
    )?;
    let session = field(&session, "sessionId")?;
    let run = (|| -> Result<()> {
        let script = format!(
            "function(){{document.body.innerHTML = {}; return ({})(document.body);}}",
            serde_json::to_string(include_str!("../tests/format.html"))?,
            MARKDOWN
        );
        let markdown = cdp.eval(session, &script, json!({}), 15)?;
        let markdown = markdown.as_str().context("无提取结果")?;
        if markdown.contains("界面标签不应提取") {
            bail!("提取了代码块界面标签");
        }
        for expected in [
            "# 中文标题",
            "**必须保留**",
            "3. 第一项",
            "4. 第二项",
            "| 条件 | 数值 |",
            "105 ℃",
            "50 MPa",
            "10月29日16:00",
            "可能改善",
            "a\\|b",
        ] {
            if !markdown.contains(expected) {
                bail!("Markdown 测试缺失：{expected}；结果：{markdown}");
            }
        }
        println!("{{\"markdownFixture\":\"passed\"}}");
        Ok(())
    })();
    let _ = cdp.call("Target.closeTarget", json!({"targetId":target}), None, 15);
    run
}
