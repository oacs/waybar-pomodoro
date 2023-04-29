use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const FIFO_PATH: &str = "pomodoro_fifo";
const STATE_PATH: &str = "pomodoro_state.json";

/// Struct representing a Pomodoro timer with start and pause functionalities.
#[derive(Clone, Debug)]
struct Pomodoro {
    start_time: Option<Instant>,  // The time at which the Pomodoro was started
    end_time: Option<Instant>,    // The time at which the Pomodoro will end
    total_time: u64,              // The total time of the Pomodoro in seconds
    is_running: bool,             // Flag to indicate if the Pomodoro is currently running
    elapsed_time: u64,            // The elapsed time of the Pomodoro in seconds
}

impl Pomodoro {
    /// Creates a new Pomodoro instance with default settings.
    ///
    /// # Examples
    ///
    /// ```
    /// let pomodoro = Pomodoro::new();
    /// ```
    fn new() -> Self {
        Self {
            start_time: None,
            end_time: None,
            total_time: 1500,  // 25 minutes in seconds
            is_running: false,
            elapsed_time: 0,
        }
    }

    /// Starts the Pomodoro timer. If the timer is already running, updates the end time
    /// of the Pomodoro to maintain the same total time.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut pomodoro = Pomodoro::new();
    ///
    /// pomodoro.start();
    /// ```
    fn start(&mut self) {
        if !self.is_running {
            let now = Instant::now();
            if self.start_time.is_none() {
                self.start_time = Some(now);
                self.end_time = Some(now + Duration::from_secs(self.total_time));
            } else {
                self.end_time = Some(
                    now + self
                        .end_time
                        .unwrap()
                        .duration_since(self.start_time.unwrap()),
                );
                self.start_time = Some(now);
            }
            self.is_running = true;
        }
    }

    /// Pauses the Pomodoro timer and updates the elapsed time with the amount of time
    /// the timer was running.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut pomodoro = Pomodoro::new();
    ///
    /// pomodoro.start();
    /// // Do some work
    /// pomodoro.pause();
    /// ```
    fn pause(&mut self) {
        if self.is_running {
            let now = Instant::now();
            self.elapsed_time += now.duration_since(self.start_time.unwrap()).as_secs();
            self.is_running = false;
        }
    }

    /// Returns a JSON string containing the elapsed time and remaining time of the Pomodoro
    /// timer in minutes and seconds.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut pomodoro = Pomodoro::new();
    ///
    /// pomodoro.start();
    /// let pomodoro_json = pomodoro.current_pomodoro();
    /// ```
    ///
    /// Returns:
    ///
    /// ```json
    /// {
    ///     "elapsed_time": "00:00",
    ///     "text": "25:00"
    /// }
    /// ```
    fn current_pomodoro(&self) -> String {
        if self.start_time.is_none() {
            return json!({"text": ""}).to_string();
        }

        let elapsed_time = if self.is_running {
            self.elapsed_time
                + Instant::now()
                    .duration_since(self.start_time.unwrap())
                    .as_secs()
        } else {
            self.elapsed_time
        };

        let remaining_time = self.total_time - elapsed_time;
        let elapsed_time_str = format!("{:02}:{:02}", elapsed_time / 60, elapsed_time % 60);
        let remaining_time_str = format!("{:02}:{:02}", remaining_time / 60, remaining_time % 60);

        json!({
            "elapsed_time": elapsed_time_str,
            "text": remaining_time_str
        })
        .to_string()
    }
}

fn main() {
    let pomodoro = Arc::new(Mutex::new(Pomodoro::new()));
    let command_queue = Arc::new(Mutex::new(Vec::<String>::new()));

    // Load pomodoro state
    if let Ok(state_file) = File::open(STATE_PATH) {
        let state: serde_json::Value = serde_json::from_reader(state_file).unwrap_or_default();
        let mut pomodoro = pomodoro.lock().unwrap();
        pomodoro.start_time = state["start_time"]
            .as_u64()
            .map(|secs| Instant::now() - Duration::from_secs(secs));
        pomodoro.end_time = state["end_time"]
            .as_u64()
            .map(|secs| Instant::now() + Duration::from_secs(secs));
        pomodoro.total_time = state["total_time"].as_u64().unwrap_or(1500);
        pomodoro.is_running = state["is_running"].as_bool().unwrap_or(false);
        pomodoro.elapsed_time = state["elapsed_time"].as_u64().unwrap_or(0);
    }

    if !Path::new(FIFO_PATH).exists() {
        std::fs::remove_file(FIFO_PATH).ok();
        nix::unistd::mkfifo(FIFO_PATH, nix::sys::stat::Mode::S_IRWXU).unwrap();
    }

    let pomodoro_clone = pomodoro.clone();
    let timer_thread = thread::spawn(move || loop {
        let command = read_command(FIFO_PATH);
        match command.as_str() {
            "start" => pomodoro_clone.lock().unwrap().start(),
            "pause" => pomodoro_clone.lock().unwrap().pause(),
            "toggle" => {
                let mut pomodoro = pomodoro_clone.lock().unwrap();
                if pomodoro.is_running {
                    pomodoro.pause();
                } else {
                    pomodoro.start();
                }
            }
            "stop" => break,
            _ => {}
        }
        println!("{}", pomodoro_clone.lock().unwrap().current_pomodoro());
        thread::sleep(Duration::from_secs(1));
    });

    let fifo = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(FIFO_PATH)
        .unwrap();

    let reader = BufReader::new(fifo);

    for line in reader.lines() {
        let cmd = line.unwrap().to_lowercase();
        if ["start", "pause", "toggle", "stop"].contains(&cmd.as_str()) {
            command_queue.lock().unwrap().push(cmd);
        } else {
            println!("Invalid command");
        }
    }

    timer_thread.join().unwrap();

    // Save pomodoro state
    let state_file = File::create(STATE_PATH).unwrap();
    let state = json!({
        "start_time": pomodoro.lock().unwrap().start_time.map(|t| t.elapsed().as_secs()),
        "end_time": pomodoro.lock().unwrap().end_time.map(|t| t.duration_since(Instant::now()).as_secs()),
        "total_time": pomodoro.lock().unwrap().total_time,
        "is_running": pomodoro.lock().unwrap().is_running,
        "elapsed_time": pomodoro.lock().unwrap().elapsed_time
    });

    serde_json::to_writer_pretty(state_file, &state).unwrap();
}

fn read_command(command_path: &str) -> String {
    if let Ok(file) = File::open(command_path) {
        let mut buf_reader = BufReader::new(file);
        let mut command = String::new();
        buf_reader.read_line(&mut command).unwrap();
        std::fs::remove_file(command_path).ok();
        command.trim().to_lowercase()
    } else {
        String::new()
    }
}
