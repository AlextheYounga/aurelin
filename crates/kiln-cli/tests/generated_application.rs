//! Builds and exercises the generated app, not a parallel example or fake.

use std::{
    env::consts::EXE_SUFFIX,
    error::Error,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct RunningApp(Child);

impl Drop for RunningApp {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn request(address: SocketAddr, method: &str, path: &str) -> io::Result<String> {
    request_with_headers(address, method, path, &[])
}

fn request_with_headers(address: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)]) -> io::Result<String> {
    let mut stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n")?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

#[test]
fn generated_project_builds_tests_and_serves_real_http() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let destination = directory.path().join("application with spaces");
    let cli_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("new")
        .arg(&destination)
        .args(["--name", "kiln-smoke-app", "--framework-path"])
        .arg(cli_root.join("../kiln-framework"))
        .output()?;
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));

    // Share dependency artifacts with the workspace to keep this real
    // generated-project check inexpensive after the first run.
    let target = cli_root.join("../../target").canonicalize()?;
    for arguments in [
        vec!["fmt", "--all", "--", "--check"],
        vec!["clippy", "--all-targets", "--", "-D", "warnings"],
        vec!["test"],
        vec!["build"],
    ] {
        let output = Command::new("cargo")
            .args(arguments)
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&destination)
            .output()?;
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let binary = target.join("debug").join(format!("kiln-smoke-app{EXE_SUFFIX}"));
    let child = Command::new(&binary)
        .current_dir(&destination)
        .env_clear()
        .env("SERVER_PORT", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut app = RunningApp(child);
    let stderr = app.0.stderr.take().ok_or_else(|| io::Error::other("no startup log pipe"))?;
    let mut startup = String::new();
    BufReader::new(stderr).read_line(&mut startup)?;
    let address: SocketAddr = startup
        .split("address=")
        .nth(1)
        .ok_or_else(|| io::Error::other(format!("server did not start: {startup}")))?
        .trim()
        .parse()?;

    let home = request(address, "GET", "/")?;
    assert!(home.starts_with("HTTP/1.1 200"), "{home}");
    assert!(home.ends_with("Welcome to Kiln"), "{home}");
    assert!(request(address, "GET", "/missing")?.starts_with("HTTP/1.1 404"));
    assert!(request(address, "POST", "/")?.starts_with("HTTP/1.1 405"));
    let health = request_with_headers(address, "GET", "/up", &[("Accept", "application/json")])?;
    assert!(health.starts_with("HTTP/1.1 200"), "{health}");
    assert!(health.ends_with(r#"{"status":"up"}"#), "{health}");
    let head = request(address, "HEAD", "/")?;
    assert!(head.starts_with("HTTP/1.1 200"));
    assert!(head.ends_with("\r\n\r\n"));

    #[cfg(unix)]
    {
        let status = Command::new("kill").args(["-INT", &app.0.id().to_string()]).status()?;
        assert!(status.success());
        assert!(app.0.wait()?.success(), "Ctrl+C must shut down cleanly");
    }

    let occupied = TcpListener::bind("127.0.0.1:0")?;
    let output = Command::new(&binary)
        .current_dir(&destination)
        .env_clear()
        .env("SERVER_PORT", occupied.local_addr()?.port().to_string())
        .output()?;
    assert!(!output.status.success(), "binding failure must not report success");

    fs::write(destination.join(".env"), "PASSWORD=\"secret-without-closing-quote")?;
    let output = Command::new(&binary).current_dir(&destination).env_clear().output()?;
    assert!(!output.status.success(), "invalid dotenv must not start a server");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-without-closing-quote"));
    Ok(())
}
