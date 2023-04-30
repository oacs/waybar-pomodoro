use std::process::Command;

pub enum PomodoroEvent {
    Pomodoro,
    ShortBreak,
    LongBreak,
    Error,
}

pub fn send_notification(event: PomodoroEvent, sound_file: &str) {
    let message = match event {
        PomodoroEvent::Pomodoro => "Time for a Pomodoro session!",
        PomodoroEvent::ShortBreak => "Take a short break.",
        PomodoroEvent::LongBreak => "Take a long break.",
        PomodoroEvent::Error => "An error occurred.",
    };

    let icon = match event {
        PomodoroEvent::Pomodoro => "tomato",
        PomodoroEvent::ShortBreak => "coffee",
        PomodoroEvent::LongBreak => "rest",
        PomodoroEvent::Error => "dialog-error",
    };

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "dunstify -i {} '{}' && aplay {}",
            icon, message, sound_file
        ))
        .output()
        .expect("Failed to send notification");

    if !output.status.success() {
        eprintln!(
            "Failed to send notification: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
