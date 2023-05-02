use serde_json::json;

use crate::dunstify::PomodoroEvent;
use crate::send_notification;
use std::time::{Duration, Instant};

use std::fs::File;

const STATE_PATH: &str = "pomodoro_state.json";

const POMODORO_DURATION: u64 = 25 * 60; // 25 minutes in seconds
const SHORT_BREAK_DURATION: u64 = 5 * 60; // 5 minutes in seconds
const LONG_BREAK_DURATION: u64 = 30 * 60; // 30 minutes in seconds
const POMODOROS_PER_LONG_BREAK: u64 = 4; // Number of pomodoros before a long break
/// Enum representing the type of break to take.
#[derive(PartialEq)]
enum BreakType {
    Short,
    Long,
}

/// Struct representing a Pomodoro timer with start, pause, and break functionalities.
#[derive(Clone, Debug)]
pub struct Pomodoro {
    start_time: Option<Instant>, // The time at which the Pomodoro was started
    end_time: Option<Instant>,   // The time at which the Pomodoro will end
    total_time: u64,             // The total time of the Pomodoro in seconds
    pub is_running: bool,        // Flag to indicate if the Pomodoro is currently running
    elapsed_time: u64,           // The elapsed time of the Pomodoro in seconds
    pomodoros_completed: u64,    // The number of pomodoros completed
    sound_path: Option< String >,          // The number of pomodoros completed
}

impl Pomodoro {
    pub fn new(sound_path: Option<String>) -> Self {
        Self {
            start_time: None,
            end_time: None,
            total_time: POMODORO_DURATION,
            is_running: false,
            elapsed_time: 0,
            pomodoros_completed: 0,
            sound_path,
        }
    }

    pub fn start(&mut self) {
        if !self.is_running {
            let now = Instant::now();
            if let Some(start_time) = self.start_time {
                self.end_time = Some(
                    now + self
                        .end_time
                        .unwrap()
                        .duration_since(start_time),
                );
            } else {
                self.end_time = Some(now + Duration::from_secs(self.total_time));
            }
            self.start_time = Some(now);
            self.is_running = true;
        }
    }

    pub fn pause(&mut self) {
        if self.is_running {
            let now = Instant::now();
            self.elapsed_time += now.duration_since(self.start_time.unwrap()).as_secs();
            self.is_running = false;
        }
    }

    /// Starts a break with the given duration.
    pub fn setup_timer(&mut self, break_duration: u64) {
        self.total_time = break_duration;
        self.elapsed_time = 0;
        self.is_running = false;
        self.start_time = None;
        self.end_time = None;
    }

    fn get_elapsed_time(self) -> u64 {
        if self.is_running {
            self.elapsed_time
                + Instant::now()
                    .duration_since(self.start_time.unwrap())
                    .as_secs()
        } else {
            self.elapsed_time
        }
    }

    fn get_total_time_and_break_type(self) -> (u64, BreakType) {
        match self.pomodoros_completed {
            POMODOROS_PER_LONG_BREAK => (LONG_BREAK_DURATION, BreakType::Long),
            _ => (SHORT_BREAK_DURATION, BreakType::Short),
        }
    }

    fn handle_elapsed_time_over_total_time(
        &mut self,
        total_time: u64,
        break_type: BreakType,
    ) -> String {
        if total_time == LONG_BREAK_DURATION || total_time == SHORT_BREAK_DURATION {
            if self.is_running {
                send_notification(PomodoroEvent::Pomodoro, self.sound_path.as_deref());
                self.setup_timer(POMODORO_DURATION)
            }
        } else {
            match break_type {
                BreakType::Long => {
                    send_notification(PomodoroEvent::LongBreak, self.sound_path.as_deref());
                    self.pomodoros_completed = 0;
                    self.setup_timer(LONG_BREAK_DURATION);
                }
                BreakType::Short => {
                    self.pomodoros_completed += 1;
                    send_notification(PomodoroEvent::ShortBreak, self.sound_path.as_deref());
                    self.setup_timer(SHORT_BREAK_DURATION);
                }
            }
        }
        let elapsed_time_str = format!("{:02}:{:02}", 0, 0);
        let remaining_time_str = format!("{:02}:{:02}", total_time / 60, total_time % 60);
        json!({
            "elapsed_time": elapsed_time_str,
            "text": remaining_time_str
        })
        .to_string()
    }

    fn handle_remaining_time(total_time: u64, elapsed_time: u64) -> String {
        let remaining_time = total_time - elapsed_time;
        let elapsed_time_str = format!("{:02}:{:02}", elapsed_time / 60, elapsed_time % 60);
        let remaining_time_str = format!("{:02}:{:02}", remaining_time / 60, remaining_time % 60);

        json!({
            "elapsed_time": elapsed_time_str,
            "text": remaining_time_str
        })
        .to_string()
    }

    pub fn current_pomodoro(&mut self) -> String {
        let elapsed_time = self.clone().get_elapsed_time();
        let (total_time, break_type) = self.clone().get_total_time_and_break_type();

        if elapsed_time > total_time {
            self.handle_elapsed_time_over_total_time(total_time, break_type)
        } else {
            Pomodoro::handle_remaining_time(total_time, elapsed_time)
        }
    }

    pub fn load_pomodoro_state(&mut self) {
        if let Ok(state_file) = File::open(STATE_PATH) {
            let state: serde_json::Value = serde_json::from_reader(state_file).unwrap_or_default();
            self.start_time = state["start_time"]
                .as_u64()
                .map(|secs| Instant::now() - Duration::from_secs(secs));
            self.end_time = state["end_time"]
                .as_u64()
                .map(|secs| Instant::now() + Duration::from_secs(secs));
            self.total_time = state["total_time"].as_u64().unwrap_or(POMODORO_DURATION);
            self.is_running = state["is_running"].as_bool().unwrap_or(false);
            self.elapsed_time = state["elapsed_time"].as_u64().unwrap_or(0);
            self.pomodoros_completed = state["pomodoros_completed"].as_u64().unwrap_or(0);
        }
    }

    pub fn save_state(&self) {
        let state_file = File::create(STATE_PATH).unwrap();
        let state = json!({
            "start_time": self.start_time.map(|t| t.elapsed().as_secs()),
            "end_time": self.end_time.map(|t| t.duration_since(Instant::now()).as_secs()),
            "total_time": self.total_time,
            "is_running": self.is_running,
            "elapsed_time": self.elapsed_time,
            "pomodoros_completed": self.pomodoros_completed
        });
        serde_json::to_writer_pretty(state_file, &state).unwrap();
    }
}
