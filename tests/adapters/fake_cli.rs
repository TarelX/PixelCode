use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn emit(out: &mut impl Write, line: &str) {
    writeln!(out, "{line}").unwrap();
    out.flush().unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "__pixelcode_grandchild__") {
        thread::sleep(Duration::from_secs(120));
        return;
    }
    if let Ok(path) = std::env::var("PIXELCODE_FAKE_ARGS_FILE") {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{}", args.join(" ")).unwrap();
    }
    let mut prompt = String::new();
    if io::stdin().read_to_string(&mut prompt).is_err() {
        std::process::exit(2);
    }
    if prompt.contains("abort") {
        eprintln!("fake cli abort");
        std::process::exit(3);
    }
    let codex = args.iter().any(|arg| arg == "--json") || args.iter().any(|arg| arg == "exec");
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if prompt.contains("tree") {
        let exe = std::env::current_exe().unwrap();
        let child = Command::new(exe)
            .arg("__pixelcode_grandchild__")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        if let Ok(path) = std::env::var("PIXELCODE_FAKE_PID_FILE") {
            std::fs::write(path, child.id().to_string()).unwrap();
        }
        if codex {
            emit(&mut out, "{\"type\":\"item.started\",\"item\":{\"id\":\"msg\",\"type\":\"agent_message\",\"text\":\"working\"}}");
        } else {
            emit(
                &mut out,
                "{\"type\":\"stream_event\",\"event\":{\"type\":\"message_start\"}}",
            );
            emit(&mut out, "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"working\"}}}");
        }
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }
    if codex {
        emit(
            &mut out,
            "{\"type\":\"thread.started\",\"thread_id\":\"fake-codex-thread\"}",
        );
        emit(&mut out, "{\"type\":\"item.started\",\"item\":{\"id\":\"msg\",\"type\":\"agent_message\",\"text\":\"Hello \"}}");
        thread::sleep(Duration::from_millis(50));
        emit(&mut out, "{\"type\":\"item.completed\",\"item\":{\"id\":\"msg\",\"type\":\"agent_message\",\"text\":\"Hello world\"}}");
        emit(&mut out, "{\"type\":\"turn.completed\"}");
    } else {
        emit(
            &mut out,
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"message_start\"}}",
        );
        thread::sleep(Duration::from_millis(50));
        emit(&mut out, "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello \"}}}");
        thread::sleep(Duration::from_millis(50));
        emit(&mut out, "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"world\"}}}");
        emit(&mut out, "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"Hello world\",\"session_id\":\"fake-claude-session\"}");
    }
}
