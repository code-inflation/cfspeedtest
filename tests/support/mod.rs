use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
    pub delay: Duration,
    pub disconnect: bool,
    pub declared_length: Option<usize>,
}

impl Response {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
            headers: vec![],
            delay: Duration::ZERO,
            disconnect: false,
            declared_length: None,
        }
    }

    pub fn normal(path: &str) -> Self {
        match path {
            "/cdn-cgi/trace" => Self::new(200, "ip=127.0.0.1\nloc=CH\ncolo=ZRH\n"),
            "/__down?bytes=0" => Self::new(200, ""),
            _ => Self::new(200, vec![1; 100_000]),
        }
    }
}

pub struct Server {
    pub url: String,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Server {
    pub fn new(handler: impl Fn(&str) -> Response + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let running = Arc::new(AtomicBool::new(true));
        let active = running.clone();
        let handle = thread::spawn(move || {
            while active.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if let Some(path) = read_request(&mut stream) {
                            let response = handler(&path);
                            if response.disconnect {
                                continue;
                            }
                            let mut head = format!(
                                "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
                                response.status,
                                response.declared_length.unwrap_or(response.body.len())
                            );
                            for (name, value) in response.headers {
                                head.push_str(&format!("{name}: {value}\r\n"));
                            }
                            head.push_str("\r\n");
                            if stream.write_all(head.as_bytes()).is_err() {
                                continue;
                            }
                            let start = Instant::now();
                            while start.elapsed() < response.delay && active.load(Ordering::SeqCst)
                            {
                                thread::sleep(Duration::from_millis(5));
                            }
                            let _ = stream.write_all(&response.body);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("mock accept: {error}"),
                }
            }
        });
        Self {
            url,
            running,
            handle: Some(handle),
        }
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cfspeedtest"));
        command.args(["--server", &self.url]);
        if !args.contains(&"--upload-only") {
            command.arg("--download-only");
        }
        for (flag, value) in [
            ("-n", "1"),
            ("--nr-latency-tests", "0"),
            ("-m", "100k"),
            ("-o", "json"),
        ] {
            if !args.contains(&flag) {
                command.args([flag, value]);
            }
        }
        command.args(args).env("NO_PROXY", "*").env("no_proxy", "*");
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        finish(self.command(args).spawn().unwrap(), Duration::from_secs(4))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.handle.take().unwrap().join().unwrap();
    }
}

pub fn finish(mut child: Child, timeout: Duration) -> Output {
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > timeout {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "CLI exceeded {timeout:?}; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
    child.wait_with_output().unwrap()
}

fn read_request(stream: &mut TcpStream) -> Option<String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buf = [0; 4096];
    loop {
        let count = stream.read(&mut buf).ok()?;
        if count == 0 {
            return None;
        }
        bytes.extend_from_slice(&buf[..count]);
        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]).into_owned();
            let length = headers
                .lines()
                .filter_map(|l| l.split_once(':'))
                .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .map_or(0, |(_, v)| v.trim().parse::<usize>().unwrap());
            while bytes.len() < end + 4 + length {
                let count = stream.read(&mut buf).ok()?;
                if count == 0 {
                    return None;
                }
                bytes.extend_from_slice(&buf[..count]);
            }
            return headers.split_whitespace().nth(1).map(str::to_owned);
        }
    }
}
