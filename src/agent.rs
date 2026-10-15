use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Eventos ao vivo de um turno em andamento.
pub enum Ev {
    Token(String),
    Tool(String),
    Usage(f64),
}

pub struct Reply {
    pub text: String,
    pub json: Option<Value>,
}

pub struct Agent {
    pub model: &'static str,
    pub system: String,
    pub session: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Session(String),
    Token(String),
    Tool(String),
    Usage(f64),
    Result { ok: bool, text: String, json: Option<Value> },
}

/// Uma linha do `--output-format stream-json` do claude → zero ou mais eventos.
pub fn parse(line: &str) -> Vec<Parsed> {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return vec![] };
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    match v["type"].as_str() {
        Some("system") if v["subtype"] == "init" => vec![Parsed::Session(s(&v["session_id"]))],
        Some("stream_event") => {
            let d = &v["event"]["delta"];
            if v["event"]["type"] == "content_block_delta" && d["type"] == "text_delta" {
                vec![Parsed::Token(s(&d["text"]))]
            } else {
                vec![]
            }
        }
        Some("assistant") => v["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["type"] == "tool_use")
            .filter_map(|c| match c["name"].as_str() {
                Some("WebSearch") => Some(Parsed::Tool(trf!("buscando: {}", "searching: {}", s(&c["input"]["query"])))),
                Some("WebFetch") => Some(Parsed::Tool(trf!("lendo: {}", "reading: {}", s(&c["input"]["url"])))),
                _ => None,
            })
            .collect(),
        Some("rate_limit_event") => v["rate_limit_info"]["unifiedWindows"]["five_hour"]["utilization"]
            .as_f64()
            .map(Parsed::Usage)
            .into_iter()
            .collect(),
        Some("result") => vec![Parsed::Result {
            ok: v["is_error"] != true,
            text: v["result"].as_str().map(String::from).unwrap_or_else(|| s(&v["subtype"])),
            json: Some(v["structured_output"].clone()).filter(|j| !j.is_null()),
        }],
        _ => vec![],
    }
}

impl Agent {
    pub fn new(model: &'static str, system: String) -> Self {
        Agent { model, system, session: None }
    }

    /// Um turno: roda `claude -p`, repassa eventos ao vivo e devolve a resposta final.
    pub async fn ask(&mut self, prompt: &str, schema: Option<&str>, mut on: impl FnMut(Ev)) -> Result<Reply, String> {
        let work = crate::debate::data_dir().join("work");
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let bin = std::env::var("GAVEL_CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
        let mut cmd = Command::new(bin);
        cmd.current_dir(&work)
            .args(["-p", "--model", self.model, "--output-format", "stream-json", "--verbose"])
            .args(["--include-partial-messages", "--system-prompt", &self.system])
            .args(["--tools", "WebSearch,WebFetch", "--allowedTools", "WebSearch,WebFetch"])
            // isola os agentes de hooks, MCPs e skills do usuário; NÃO usar --bare (desliga o login da assinatura)
            .args(["--permission-mode", "dontAsk", "--setting-sources", "", "--strict-mcp-config"])
            .arg("--disable-slash-commands");
        if let Some(id) = &self.session {
            cmd.args(["--resume", id]);
        }
        if let Some(schema) = schema {
            cmd.args(["--json-schema", schema]);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| trf!("não consegui rodar o claude: {e}", "could not run claude: {e}"))?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(prompt.as_bytes()).await.map_err(|e| e.to_string())?;
        drop(stdin);
        let mut stderr = child.stderr.take().unwrap();
        let err_task = tokio::spawn(async move {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s).await;
            s
        });

        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut result = None;
        while let Ok(Some(line)) = lines.next_line().await {
            for p in parse(&line) {
                match p {
                    Parsed::Session(id) => self.session = Some(id),
                    Parsed::Token(t) => on(Ev::Token(t)),
                    Parsed::Tool(t) => on(Ev::Tool(t)),
                    Parsed::Usage(u) => on(Ev::Usage(u)),
                    Parsed::Result { ok, text, json } => result = Some((ok, text, json)),
                }
            }
        }
        let _ = child.wait().await;
        match result {
            Some((true, text, json)) => Ok(Reply { text, json }),
            Some((false, text, _)) => Err(text),
            None => {
                let err = err_task.await.unwrap_or_default();
                Err(trf!("claude terminou sem resposta: {}", "claude exited without a reply: {}", err.trim()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_stream() {
        let all: Vec<Parsed> = include_str!("../tests/fixtures/search_schema.jsonl").lines().flat_map(parse).collect();
        assert!(matches!(&all[0], Parsed::Session(id) if !id.is_empty()));
        assert!(all.iter().any(|p| matches!(p, Parsed::Tool(t) if t.starts_with("buscando: "))));
        assert!(all.iter().any(|p| matches!(p, Parsed::Usage(u) if *u > 0.0)));
        let Some(Parsed::Result { ok, json: Some(j), .. }) = all.last() else { panic!("sem result") };
        assert!(ok);
        assert!(j["populacao"].is_string());
    }
}
