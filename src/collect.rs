use crate::{model::Snapshot, parse};
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// No shell, fixed executable paths, bounded wait, concurrent pipe draining.
pub fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let (ok, out, err) = capture(program, args, 5)?;
    if ok {
        Ok(out)
    } else {
        Err(format!("{program}: {}", err.trim()))
    }
}
/// Like [`command`] but returns `(success, stdout, stderr)` so callers such as ping can
/// interpret a non-zero exit, and takes its own wait limit.
pub fn capture(
    program: &str,
    args: &[&str],
    seconds: u64,
) -> Result<(bool, String, String), String> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let stderr = child.stderr.take().ok_or("Missing stderr")?;
    let read = |pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.take(8 * 1024 * 1024).read_to_end(&mut bytes);
            result.map(|_| String::from_utf8_lossy(&bytes).into_owned())
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < Duration::from_secs(seconds) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program}: timed out after {seconds} seconds"));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program}: {e}"));
            }
        }
    };
    let out = out
        .join()
        .map_err(|_| "stdout reader failed")?
        .map_err(|e| e.to_string())?;
    let err = err
        .join()
        .map_err(|_| "stderr reader failed")?
        .map_err(|e| e.to_string())?;
    Ok((status.success(), out, err))
}
pub fn raw() -> Result<String, String> {
    if cfg!(target_os = "macos") {
        command("/sbin/ifconfig", &["-a"])
    } else {
        Err("Live collection supports macOS; use --from to inspect a saved macOS capture".into())
    }
}
pub fn snapshot() -> Result<Snapshot, String> {
    let mut s = parse::ifconfig(&raw()?)?;
    if cfg!(target_os = "macos") {
        match command("/usr/sbin/networksetup", &["-listallhardwareports"]) {
            Ok(text) => {
                let ports = parse::hardware_ports(&text);
                for i in &mut s.interfaces {
                    i.hardware = ports.get(&i.name).cloned();
                }
            }
            Err(e) => s.warnings.push(e),
        }
        match command("/usr/sbin/netstat", &["-ibn"]) {
            Ok(text) => {
                let counters = parse::netstat_counters(&text);
                for i in &mut s.interfaces {
                    i.counters = counters.get(&i.name).cloned();
                }
            }
            Err(e) => s.warnings.push(e),
        }
    }
    Ok(s)
}
pub fn routes() -> Result<String, String> {
    if cfg!(target_os = "macos") {
        command("/usr/sbin/netstat", &["-rn"])
    } else {
        Err("Live routes require macOS".into())
    }
}
pub fn dns() -> Result<String, String> {
    if cfg!(target_os = "macos") {
        command("/usr/sbin/scutil", &["--dns"])
    } else {
        Err("Live DNS inspection requires macOS".into())
    }
}
