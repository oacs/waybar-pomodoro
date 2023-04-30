mod dunstify;

use dunstify::{send_notification, PomodoroEvent};
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

const POMODORO_DURATION: u64 = 25 * 60; // 25 minutes in seconds
const SHORT_BREAK_DURATION: u64 = 5 * 60; // 5 minutes in seconds
const LONG_BREAK_DURATION: u64 = 30 * 60; // 30 minutes in seconds
const POMODOROS_PER_LONG_BREAK: u64 = 4; // Number of pomodoros before a long break

const FIFO_PATH: &str = "pomodoro_fifo";
const STATE_PATH: &str = "pomodoro_state.json";

/// Enum representing the type of break to take.
#[derive(PartialEq)]
enum BreakType {
    Short,
    Long,
}

/// Struct representing a Pomodoro timer with start, pause, and break functionalities.
#[derive(Clone, Debug)]
struct Pomodoro {
    start_time: Option<Instant>, // The time at which the Pomodoro was started
    end_time: Option<Instant>,   // The time at which the Pomodoro will end
    total_time: u64,             // The total time of the Pomodoro in seconds
    is_running: bool,            // Flag to indicate if the Pomodoro is currently running
    elapsed_time: u64,           // The elapsed time of the Pomodoro in seconds
    pomodoros_completed: u64,    // The number of pomodoros completed
}

impl Pomodoro {
    fn new() -> Self {
        Self {
            start_time: None,
            end_time: None,
            total_time: POMODORO_DURATION,
            is_running: false,
            elapsed_time: 0,
            pomodoros_completed: 0,
        }
    }

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

    fn pause(&mut self) {
        if self.is_running {
            let now = Instant::now();
            self.elapsed_time += now.duration_since(self.start_time.unwrap()).as_secs();
            self.is_running = false;
        }
    }

    /// Starts a break with the given duration.
    fn setup_timer(&mut self, break_duration: u64) {
        self.total_time = break_duration;
        self.elapsed_time = 0;
        self.is_running = false;
        self.start_time = None;
        self.end_time = None;
    }

    fn current_pomodoro(&mut self) -> String {
        let elapsed_time = if self.is_running {
            self.elapsed_time
                + Instant::now()
                    .duration_since(self.start_time.unwrap())
                    .as_secs()
        } else {
            self.elapsed_time
        };

        println!("{}", self.pomodoros_completed);
        let (total_time, break_type) = match self.pomodoros_completed {
            POMODOROS_PER_LONG_BREAK => (LONG_BREAK_DURATION, BreakType::Long),
            _ => (SHORT_BREAK_DURATION, BreakType::Short),
        };

        if elapsed_time > self.total_time {
            if self.total_time == LONG_BREAK_DURATION || self.total_time == SHORT_BREAK_DURATION {
                if self.is_running {
                    send_notification(PomodoroEvent::Pomodoro);
                    self.setup_timer(POMODORO_DURATION)
                }
            } else {
                match break_type {
                    BreakType::Long => {
                        send_notification(PomodoroEvent::LongBreak);
                        self.pomodoros_completed = 0;
                        self.setup_timer(LONG_BREAK_DURATION);
                    }
                    BreakType::Short => {
                        self.pomodoros_completed += 1;
                        send_notification(PomodoroEvent::ShortBreak);
                        self.setup_timer(SHORT_BREAK_DURATION);
                    }
                }
            }
            let elapsed_time_str = format!("{:02}:{:02}", 0, 0);
            let remaining_time_str =
                format!("{:02}:{:02}", self.total_time / 60, self.total_time % 60);
            return json!({
                "elapsed_time": elapsed_time_str,
                "text": remaining_time_str
            })
            .to_string();
        } else {
            let remaining_time = self.total_time - elapsed_time;
            let elapsed_time_str = format!("{:02}:{:02}", elapsed_time / 60, elapsed_time % 60);
            let remaining_time_str =
                format!("{:02}:{:02}", remaining_time / 60, remaining_time % 60);

            return json!({
                "elapsed_time": elapsed_time_str,
                "text": remaining_time_str
            })
            .to_string();
        }
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
        pomodoro.total_time = state["total_time"].as_u64().unwrap_or(POMODORO_DURATION);
        pomodoro.is_running = state["is_running"].as_bool().unwrap_or(false);
        pomodoro.elapsed_time = state["elapsed_time"].as_u64().unwrap_or(0);
        pomodoro.pomodoros_completed = state["pomodoros_completed"].as_u64().unwrap_or(0);
    }

    if !Path::new(FIFO_PATH).exists() {
        std::fs::remove_file(FIFO_PATH).ok();
        nix::unistd::mkfifo(FIFO_PATH, nix::sys::stat::Mode::S_IRWXU).unwrap();
    }

    let pomodoro_clone = pomodoro.clone();
    println!("{}", pomodoro_clone.lock().unwrap().current_pomodoro());
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
        "elapsed_time": pomodoro.lock().unwrap().elapsed_time,
        "pomodoros_completed": pomodoro.lock().unwrap().pomodoros_completed
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
