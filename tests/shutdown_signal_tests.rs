#[cfg(unix)]
mod unix_shutdown_signal {
    use std::io::ErrorKind;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    const SIGHUP: i32 = 1;
    const SIGTERM: i32 = 15;

    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "aegishv-{label}-{}-{}",
            std::process::id(),
            aegishv::util::next_sequence()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn sigterm_stops_live_run_and_emits_shutdown_event() {
        let tracefs = temp_dir("tracefs");
        let jsonl_dir = temp_dir("jsonl");
        let jsonl = jsonl_dir.join("shutdown.jsonl");
        std::fs::write(tracefs.join("trace_pipe"), "").expect("create trace_pipe fixture");

        let mut child = Command::new(env!("CARGO_BIN_EXE_aegishv"))
            .arg("run")
            .arg("--tracefs")
            .arg(&tracefs)
            .arg("--jsonl")
            .arg(&jsonl)
            .arg("--listen")
            .arg("")
            .arg("--quiet")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn aegishv");

        thread::sleep(Duration::from_millis(500));
        unsafe {
            // The child installs a SIGTERM handler during startup; this sends the real OS signal.
            assert_eq!(kill(child.id() as i32, SIGTERM), 0);
        }

        let _ = wait_for_exit(&mut child);

        let output = std::fs::read_to_string(&jsonl).expect("read shutdown JSONL");
        assert!(output.contains("\"reason\":\"shutdown_signal\""));
        assert!(output.contains("SIGTERM"));

        let _ = remove_dir_if_exists(tracefs);
        let _ = remove_dir_if_exists(jsonl_dir);
    }

    #[test]
    fn sighup_reloads_config_and_emits_reload_event() {
        let tracefs = temp_dir("tracefs");
        let jsonl_dir = temp_dir("jsonl");
        let config_dir = temp_dir("config");
        let jsonl = jsonl_dir.join("reload.jsonl");
        let config = config_dir.join("aegishv.toml");
        std::fs::write(tracefs.join("trace_pipe"), "").expect("create trace_pipe fixture");
        std::fs::write(
            &config,
            r#"
[general]
flush_every = 1

[[policy.rules]]
name = "reload-noop"
id = "reload-noop"
mode = "dry_run"
action = { kind = "noop" }
match = { category = "wx", severity_at_least = "critical" }
"#,
        )
        .expect("write reload config");

        let mut child = Command::new(env!("CARGO_BIN_EXE_aegishv"))
            .arg("run")
            .arg("--tracefs")
            .arg(&tracefs)
            .arg("--config")
            .arg(&config)
            .arg("--jsonl")
            .arg(&jsonl)
            .arg("--listen")
            .arg("")
            .arg("--quiet")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn aegishv");

        thread::sleep(Duration::from_millis(500));
        unsafe {
            assert_eq!(kill(child.id() as i32, SIGHUP), 0);
        }
        thread::sleep(Duration::from_millis(500));
        unsafe {
            assert_eq!(kill(child.id() as i32, SIGTERM), 0);
        }

        let _ = wait_for_exit(&mut child);

        let output = std::fs::read_to_string(&jsonl).expect("read reload JSONL");
        assert!(output.contains("\"reason\":\"config_reload\""));
        assert!(output.contains("\"reason\":\"jsonl_reopen\""));
        assert!(output.contains("\"reason\":\"shutdown_signal\""));

        let _ = remove_dir_if_exists(tracefs);
        let _ = remove_dir_if_exists(jsonl_dir);
        let _ = remove_dir_if_exists(config_dir);
    }

    fn wait_for_exit(child: &mut std::process::Child) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            match child.try_wait().expect("poll child") {
                Some(status) => break status,
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
                None => {
                    let _ = child.kill();
                    panic!("aegishv did not exit after signal");
                }
            }
        };
        assert!(status.success(), "unexpected exit status: {status}");
        status
    }

    fn remove_dir_if_exists(path: PathBuf) -> std::io::Result<()> {
        match std::fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }
}
