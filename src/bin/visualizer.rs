use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_IMAGE: &str = "filler";
const LINUX_PLAYER: &str = "/filler/solution/target/docker-linux/release/filler";

#[derive(Clone)]
struct Config {
    addr: String,
    engine_dir: Option<PathBuf>,
    image: String,
    no_open: bool,
}

struct EventHub {
    clients: Mutex<Vec<Sender<String>>>,
}

impl EventHub {
    fn new() -> Self {
        Self {
            clients: Mutex::new(Vec::new()),
        }
    }

    fn subscribe(&self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        self.clients.lock().unwrap().push(tx);
        rx
    }

    fn publish(&self, payload: String) {
        self.clients
            .lock()
            .unwrap()
            .retain(|client| client.send(payload.clone()).is_ok());
    }
}

struct App {
    root: PathBuf,
    engine_dir: PathBuf,
    image: String,
    hub: EventHub,
    running: AtomicBool,
    stop: AtomicBool,
    child: Mutex<Option<Child>>,
}

#[derive(Clone)]
struct StartRequest {
    map: String,
    opponent: String,
    side: String,
    games: usize,
    seed: Option<i64>,
    live: bool,
}

#[derive(Default)]
struct MatchResult {
    index: usize,
    file: String,
    map: String,
    opponent: String,
    side: String,
    seed: i64,
    p1_score: i64,
    p2_score: i64,
    winner: u8,
    student_won: bool,
    error: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("visualizer: {}", error);
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let config = parse_args()?;
    let root = find_project_root()?;
    let engine_dir = resolve_engine_dir(&root, config.engine_dir.as_deref())?;
    let app = Arc::new(App {
        root,
        engine_dir,
        image: config.image,
        hub: EventHub::new(),
        running: AtomicBool::new(false),
        stop: AtomicBool::new(false),
        child: Mutex::new(None),
    });

    let listener = TcpListener::bind(&config.addr)?;
    let address = format!("http://{}", listener.local_addr()?);
    println!("Filler Visualizer: {}", address);
    println!("Project: {}", app.root.display());
    println!("Engine bundle: {}", app.engine_dir.display());

    if !config.no_open {
        let url = address.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(350));
            let _ = open_browser(&url);
        });
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let app = Arc::clone(&app);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, app) {
                        eprintln!("http: {}", error);
                    }
                });
            }
            Err(error) => eprintln!("accept: {}", error),
        }
    }
    Ok(())
}

fn parse_args() -> io::Result<Config> {
    let args: Vec<String> = env::args().collect();
    let mut config = Config {
        addr: DEFAULT_ADDR.to_owned(),
        engine_dir: None,
        image: DEFAULT_IMAGE.to_owned(),
        no_open: false,
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--addr" => {
                index += 1;
                config.addr = args
                    .get(index)
                    .ok_or_else(|| invalid_input("--addr requires a value"))?
                    .clone();
            }
            "--engine-dir" => {
                index += 1;
                config.engine_dir = Some(PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| invalid_input("--engine-dir requires a value"))?,
                ));
            }
            "--image" => {
                index += 1;
                config.image = args
                    .get(index)
                    .ok_or_else(|| invalid_input("--image requires a value"))?
                    .clone();
            }
            "--no-open" => config.no_open = true,
            "-h" | "--help" => {
                println!("Usage: cargo run --bin visualizer -- [--addr 127.0.0.1:8080] [--engine-dir PATH] [--image filler] [--no-open]");
                std::process::exit(0);
            }
            value => return Err(invalid_input(&format!("unknown argument: {}", value))),
        }
        index += 1;
    }
    Ok(config)
}

fn handle_connection(mut stream: TcpStream, app: Arc<App>) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("").to_owned();
    let target = request_parts.next().unwrap_or("/").to_owned();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    let body = String::from_utf8_lossy(&body).into_owned();
    drop(reader);

    if method == "GET" && target == "/api/events" {
        return handle_events(stream, &app);
    }
    if method == "GET" && target == "/api/status" {
        return respond_json(&mut stream, 200, &status_json(&app));
    }
    if method == "GET" && target == "/api/options" {
        return match options_json(&app) {
            Ok(json) => respond_json(&mut stream, 200, &json),
            Err(error) => respond_text(&mut stream, 412, &error.to_string(), "text/plain; charset=utf-8"),
        };
    }
    if method == "POST" && target == "/api/matches/start" {
        return handle_start(&mut stream, app, &body);
    }
    if method == "POST" && target == "/api/matches/stop" {
        return handle_stop(&mut stream, &app);
    }
    if method == "GET" && target.starts_with("/api/replays/raw?") {
        return handle_raw_replay(&mut stream, &app, &target);
    }
    if method == "GET" || method == "HEAD" {
        return serve_static(&mut stream, &app.root, &target, method == "HEAD");
    }
    respond_text(&mut stream, 405, "method not allowed", "text/plain; charset=utf-8")
}

fn handle_events(mut stream: TcpStream, app: &App) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
    )?;
    stream.flush()?;
    let receiver = app.hub.subscribe();
    loop {
        match receiver.recv_timeout(Duration::from_secs(15)) {
            Ok(payload) => {
                write!(stream, "data: {}\n\n", payload)?;
                stream.flush()?;
            }
            Err(RecvTimeoutError::Timeout) => {
                stream.write_all(b": ping\n\n")?;
                stream.flush()?;
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn handle_start(stream: &mut TcpStream, app: Arc<App>, body: &str) -> io::Result<()> {
    if app.running.swap(true, Ordering::SeqCst) {
        return respond_text(stream, 409, "a match series is already running", "text/plain; charset=utf-8");
    }

    let request = match parse_start_request(body).and_then(|request| validate_request(&app, request)) {
        Ok(request) => request,
        Err(error) => {
            app.running.store(false, Ordering::SeqCst);
            return respond_text(stream, 400, &error.to_string(), "text/plain; charset=utf-8");
        }
    };

    app.stop.store(false, Ordering::SeqCst);
    thread::spawn(move || {
        run_series(&app, request);
        app.running.store(false, Ordering::SeqCst);
        app.stop.store(false, Ordering::SeqCst);
    });
    respond_json(stream, 202, "{\"started\":true}")
}

fn handle_stop(stream: &mut TcpStream, app: &App) -> io::Result<()> {
    let was_running = app.running.load(Ordering::SeqCst);
    app.stop.store(true, Ordering::SeqCst);
    if let Some(child) = app.child.lock().unwrap().as_mut() {
        let _ = child.kill();
    }
    respond_json(
        stream,
        200,
        if was_running {
            "{\"stopping\":true}"
        } else {
            "{\"stopping\":false}"
        },
    )
}

fn run_series(app: &Arc<App>, request: StartRequest) {
    app.hub.publish(format!(
        "{{\"type\":\"setup\",\"message\":\"{}\"}}",
        json_escape("Preparing Rust player and Docker environment…")
    ));
    if let Err(error) = ensure_setup(app) {
        app.hub.publish(format!(
            "{{\"type\":\"error\",\"message\":\"{}\"}}",
            json_escape(&error.to_string())
        ));
        return;
    }

    app.hub.publish(format!(
        "{{\"type\":\"series_start\",\"map\":\"{}\",\"opponent\":\"{}\",\"games\":{},\"side\":\"{}\",\"live\":{}}}",
        json_escape(&request.map), json_escape(&request.opponent), request.games,
        json_escape(&request.side), request.live
    ));

    let mut wins = 0usize;
    let mut completed = 0usize;
    for game in 0..request.games {
        if app.stop.load(Ordering::SeqCst) {
            break;
        }
        let side = if request.side == "alternate" {
            if game % 2 == 0 { "p1" } else { "p2" }
        } else {
            request.side.as_str()
        };
        let seed = request.seed.map(|base| base + game as i64);
        let result = run_match(app, &request, game + 1, side, seed);
        completed += 1;
        if result.student_won {
            wins += 1;
        }
        app.hub.publish(format!(
            "{{\"type\":\"match_end\",\"result\":{}}}",
            match_result_json(&result)
        ));
    }

    app.hub.publish(format!(
        "{{\"type\":\"series_end\",\"wins\":{},\"completed\":{},\"requested\":{},\"stopped\":{}}}",
        wins, completed, request.games, app.stop.load(Ordering::SeqCst)
    ));
}

fn run_match(
    app: &Arc<App>,
    request: &StartRequest,
    index: usize,
    side: &str,
    requested_seed: Option<i64>,
) -> MatchResult {
    let mut result = MatchResult {
        index,
        map: request.map.clone(),
        opponent: request.opponent.clone(),
        side: side.to_owned(),
        seed: requested_seed.unwrap_or(0),
        ..MatchResult::default()
    };

    let log_name = format!(
        "{}-{}-{}-{:02}-{}.log",
        timestamp_millis(),
        request.map,
        request.opponent,
        index,
        side
    );
    result.file = log_name.clone();
    let replay_dir = app.root.join("replays");
    if let Err(error) = fs::create_dir_all(&replay_dir) {
        result.error = error.to_string();
        return result;
    }
    let log_path = replay_dir.join(&log_name);

    app.hub.publish(format!(
        "{{\"type\":\"match_start\",\"index\":{},\"games\":{},\"map\":\"{}\",\"opponent\":\"{}\",\"side\":\"{}\",\"live\":{}}}",
        index, request.games, json_escape(&request.map), json_escape(&request.opponent),
        json_escape(side), request.live
    ));

    let opponent = format!("linux_robots/{}", request.opponent);
    let (p1, p2) = if side == "p1" {
        (LINUX_PLAYER.to_owned(), opponent)
    } else {
        (opponent, LINUX_PLAYER.to_owned())
    };

    let mount = format!("{}:/filler/solution", app.root.display());
    let mut command = Command::new("docker");
    command
        .args(["run", "--rm", "-v"])
        .arg(mount)
        .args(["--entrypoint", "/filler/linux_game_engine"])
        .arg(&app.image)
        .args(["-f", &format!("maps/{}", request.map), "-p1", &p1, "-p2", &p2]);
    if let Some(seed) = requested_seed {
        command.args(["-s", &seed.to_string()]);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            result.error = error.to_string();
            return result;
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    *app.child.lock().unwrap() = Some(child);

    let file = match File::create(&log_path) {
        Ok(file) => file,
        Err(error) => {
            result.error = error.to_string();
            if let Some(child) = app.child.lock().unwrap().as_mut() {
                let _ = child.kill();
            }
            return result;
        }
    };
    let mut writer = BufWriter::new(file);
    let (tx, rx) = mpsc::channel::<String>();
    if let Some(stdout) = stdout {
        spawn_line_reader(stdout, tx.clone());
    }
    if let Some(stderr) = stderr {
        spawn_line_reader(stderr, tx.clone());
    }
    drop(tx);

    while let Ok(line) = rx.recv() {
        let _ = writeln!(writer, "{}", line);
        parse_engine_line(&line, &mut result);
        app.hub.publish(format!(
            "{{\"type\":\"log\",\"index\":{},\"line\":\"{}\"}}",
            index,
            json_escape(&line)
        ));
    }
    let _ = writer.flush();

    if let Some(mut child) = app.child.lock().unwrap().take() {
        match child.wait() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                if !app.stop.load(Ordering::SeqCst) {
                    result.error = format!("docker/game_engine exited with {}", status);
                }
            }
            Err(error) => result.error = error.to_string(),
        }
    }

    if result.winner == 0 {
        result.winner = if result.p1_score > result.p2_score {
            1
        } else if result.p2_score > result.p1_score {
            2
        } else {
            0
        };
    }
    result.student_won = (side == "p1" && result.winner == 1)
        || (side == "p2" && result.winner == 2);
    result
}

fn spawn_line_reader<R>(reader: R, sender: Sender<String>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn parse_engine_line(line: &str, result: &mut MatchResult) {
    let trimmed = line.trim();
    if let Some(value) = trimmed.strip_prefix("seed:") {
        if let Ok(seed) = value.trim().parse::<i64>() {
            result.seed = seed;
        }
        return;
    }
    if trimmed.starts_with("Player1") {
        if let Some(score) = trimmed.rsplit(':').next() {
            result.p1_score = score.trim().parse().unwrap_or(result.p1_score);
        }
        return;
    }
    if trimmed.starts_with("Player2") {
        if let Some(score) = trimmed.rsplit(':').next() {
            result.p2_score = score.trim().parse().unwrap_or(result.p2_score);
        }
        return;
    }
    if trimmed.starts_with("Player1 won") {
        result.winner = 1;
    } else if trimmed.starts_with("Player2 won") {
        result.winner = 2;
    }
}

fn ensure_setup(app: &App) -> io::Result<()> {
    engine_ready(&app.engine_dir)?;
    if !command_success(Command::new("docker").arg("--version"))? {
        return Err(other("Docker CLI is not available"));
    }

    if !docker_image_ready(&app.image) {
        app.hub.publish(format!(
            "{{\"type\":\"setup\",\"message\":\"{}\"}}",
            json_escape("Building official filler Docker image…")
        ));
        let status = Command::new("docker")
            .args(["build", "-t", &app.image, "."])
            .current_dir(&app.engine_dir)
            .status()?;
        if !status.success() {
            return Err(other("docker build failed"));
        }
    }

    app.hub.publish(format!(
        "{{\"type\":\"setup\",\"message\":\"{}\"}}",
        json_escape("Compiling Rust player inside the official Linux container…")
    ));
    let mount = format!("{}:/filler/solution", app.root.display());
    let status = Command::new("docker")
        .args(["run", "--rm", "-v"])
        .arg(mount)
        .args(["--entrypoint", "/bin/bash"])
        .arg(&app.image)
        .args([
            "-lc",
            "cd /filler/solution && CARGO_TARGET_DIR=target/docker-linux cargo build --release --bin filler",
        ])
        .status()?;
    if !status.success() {
        return Err(other("Rust player build inside Docker failed"));
    }
    Ok(())
}

fn status_json(app: &App) -> String {
    let engine = engine_ready(&app.engine_dir);
    let docker_ready = Command::new("docker").arg("--version").output().is_ok();
    let image_ready = docker_ready && docker_image_ready(&app.image);
    format!(
        "{{\"server\":true,\"projectDir\":\"{}\",\"engineDir\":\"{}\",\"engineReady\":{},\"engineError\":\"{}\",\"dockerReady\":{},\"imageReady\":{},\"image\":\"{}\",\"running\":{},\"autoSetupNote\":\"{}\"}}",
        json_escape(&app.root.display().to_string()),
        json_escape(&app.engine_dir.display().to_string()),
        engine.is_ok(),
        json_escape(&engine.err().map(|e| e.to_string()).unwrap_or_default()),
        docker_ready,
        image_ready,
        json_escape(&app.image),
        app.running.load(Ordering::SeqCst),
        json_escape("The Rust player and Docker image are prepared automatically when a match starts.")
    )
}

fn options_json(app: &App) -> io::Result<String> {
    let (maps, opponents) = options(app)?;
    Ok(format!(
        "{{\"maps\":{},\"opponents\":{}}}",
        json_array(&maps),
        json_array(&opponents)
    ))
}

fn options(app: &App) -> io::Result<(Vec<String>, Vec<String>)> {
    engine_ready(&app.engine_dir)?;
    let mut maps = directory_names(&app.engine_dir.join("maps"))?;
    let mut opponents = directory_names(&app.engine_dir.join("linux_robots"))?;
    maps.sort();
    opponents.sort();
    Ok((maps, opponents))
}

fn directory_names(path: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                if safe_name(name) {
                    names.push(name.to_owned());
                }
            }
        }
    }
    Ok(names)
}

fn validate_request(app: &App, request: StartRequest) -> io::Result<StartRequest> {
    if request.games == 0 || request.games > 20 {
        return Err(invalid_input("games must be between 1 and 20"));
    }
    if request.side != "p1" && request.side != "p2" && request.side != "alternate" {
        return Err(invalid_input("side must be p1, p2 or alternate"));
    }
    if !safe_name(&request.map) || !safe_name(&request.opponent) {
        return Err(invalid_input("invalid map or opponent name"));
    }
    let (maps, opponents) = options(app)?;
    if !maps.iter().any(|name| name == &request.map) {
        return Err(invalid_input("selected map is not present in the engine bundle"));
    }
    if !opponents.iter().any(|name| name == &request.opponent) {
        return Err(invalid_input("selected opponent is not present in linux_robots"));
    }
    Ok(request)
}

fn parse_start_request(body: &str) -> io::Result<StartRequest> {
    Ok(StartRequest {
        map: json_string(body, "map").ok_or_else(|| invalid_input("missing map"))?,
        opponent: json_string(body, "opponent")
            .ok_or_else(|| invalid_input("missing opponent"))?,
        side: json_string(body, "side").unwrap_or_else(|| "alternate".to_owned()),
        games: json_number(body, "games").unwrap_or(1).max(0) as usize,
        seed: match json_string(body, "seed") {
            Some(value) if !value.trim().is_empty() => Some(
                value
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| invalid_input("seed must be an integer"))?,
            ),
            _ => None,
        },
        live: json_bool(body, "live").unwrap_or(true),
    })
}

fn engine_ready(engine_dir: &Path) -> io::Result<()> {
    for path in [
        engine_dir.join("Dockerfile"),
        engine_dir.join("linux_game_engine"),
        engine_dir.join("linux_robots"),
        engine_dir.join("maps"),
    ] {
        if !path.exists() {
            return Err(other(&format!("missing official engine component: {}", path.display())));
        }
    }
    Ok(())
}

fn resolve_engine_dir(root: &Path, explicit: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Ok(value) = env::var("FILLER_ENGINE_DIR") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    let parent = root.parent().unwrap_or(root);
    Ok(parent.join("filler-engine"))
}

fn find_project_root() -> io::Result<PathBuf> {
    let mut current = env::current_dir()?;
    loop {
        if current.join("Cargo.toml").is_file() && current.join("visualizer").is_dir() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(other("project root not found; run the visualizer from the filler repository"));
        }
    }
}

fn docker_image_ready(image: &str) -> bool {
    Command::new("docker")
        .args(["image", "inspect", image])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn command_success(command: &mut Command) -> io::Result<bool> {
    Ok(command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}

fn handle_raw_replay(stream: &mut TcpStream, app: &App, target: &str) -> io::Result<()> {
    let query = target.splitn(2, '?').nth(1).unwrap_or("");
    let name = query
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next() == Some("name") {
                parts.next().map(percent_decode)
            } else {
                None
            }
        })
        .unwrap_or_default();
    if !safe_name(&name) {
        return respond_text(stream, 400, "invalid replay name", "text/plain; charset=utf-8");
    }
    let path = app.root.join("replays").join(&name);
    match fs::read(&path) {
        Ok(bytes) => respond_bytes(stream, 200, &bytes, "text/plain; charset=utf-8", false),
        Err(_) => respond_text(stream, 404, "replay not found", "text/plain; charset=utf-8"),
    }
}

fn serve_static(stream: &mut TcpStream, root: &Path, target: &str, head: bool) -> io::Result<()> {
    let raw = target.split('?').next().unwrap_or("/");
    let name = if raw == "/" {
        "index.html"
    } else {
        raw.trim_start_matches('/')
    };
    if name.contains("..") || name.contains('/') || name.contains('\\') || !safe_static_name(name) {
        return respond_text(stream, 404, "not found", "text/plain; charset=utf-8");
    }
    let path = root.join("visualizer").join(name);
    match fs::read(&path) {
        Ok(bytes) => respond_bytes(stream, 200, &bytes, content_type(name), head),
        Err(_) => respond_text(stream, 404, "not found", "text/plain; charset=utf-8"),
    }
}

fn respond_json(stream: &mut TcpStream, status: u16, body: &str) -> io::Result<()> {
    respond_text(stream, status, body, "application/json; charset=utf-8")
}

fn respond_text(stream: &mut TcpStream, status: u16, body: &str, content_type: &str) -> io::Result<()> {
    respond_bytes(stream, status, body.as_bytes(), content_type, false)
}

fn respond_bytes(
    stream: &mut TcpStream,
    status: u16,
    body: &[u8],
    content_type: &str,
    head: bool,
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        412 => "Precondition Failed",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len()
    )?;
    if !head {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn content_type(name: &str) -> &'static str {
    if name.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if name.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if name.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if name.ends_with(".json") {
        "application/json; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

fn match_result_json(result: &MatchResult) -> String {
    format!(
        "{{\"index\":{},\"file\":\"{}\",\"map\":\"{}\",\"opponent\":\"{}\",\"side\":\"{}\",\"seed\":{},\"p1Score\":{},\"p2Score\":{},\"winner\":{},\"studentWon\":{},\"error\":\"{}\"}}",
        result.index,
        json_escape(&result.file),
        json_escape(&result.map),
        json_escape(&result.opponent),
        json_escape(&result.side),
        result.seed,
        result.p1_score,
        result.p2_score,
        result.winner,
        result.student_won,
        json_escape(&result.error)
    )
}

fn json_array(values: &[String]) -> String {
    let items: Vec<String> = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect();
    format!("[{}]", items.join(","))
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 8);
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => escaped.push(' '),
            character => escaped.push(character),
        }
    }
    escaped
}

fn json_string(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{}\"", key);
    let after_key = body.get(body.find(&marker)? + marker.len()..)?;
    let after_colon = after_key.get(after_key.find(':')? + 1..)?.trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let mut result = String::new();
    let mut escaped = false;
    for character in after_colon[1..].chars() {
        if escaped {
            result.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(result);
        } else {
            result.push(character);
        }
    }
    None
}

fn json_number(body: &str, key: &str) -> Option<i64> {
    let marker = format!("\"{}\"", key);
    let after_key = body.get(body.find(&marker)? + marker.len()..)?;
    let after_colon = after_key.get(after_key.find(':')? + 1..)?.trim_start();
    let token: String = after_colon
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    token.parse().ok()
}

fn json_bool(body: &str, key: &str) -> Option<bool> {
    let marker = format!("\"{}\"", key);
    let after_key = body.get(body.find(&marker)? + marker.len()..)?;
    let after_colon = after_key.get(after_key.find(':')? + 1..)?.trim_start();
    if after_colon.starts_with("true") {
        Some(true)
    } else if after_colon.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn safe_static_name(value: &str) -> bool {
    safe_name(value)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                output.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        output.push(if bytes[index] == b'+' { b' ' } else { bytes[index] });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis()
}

fn open_browser(url: &str) -> io::Result<()> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?;
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()?;
    } else {
        Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_owned())
}

fn other(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, message.to_owned())
}
