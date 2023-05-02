mod args;
mod dunstify;
mod pomodoro;

use dunstify::send_notification;
use pomodoro::Pomodoro;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::args::handle_args;

const FIFO_PATH: &str = "pomodoro_fifo";

fn main() {
    let sound_file = handle_args();
    let pomodoro: Arc<Mutex<Pomodoro>> = Arc::new(Mutex::new(Pomodoro::new(sound_file)));
    let command_queue = Arc::new(Mutex::new(Vec::<String>::new()));
    pomodoro.lock().unwrap().load_pomodoro_state();
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
            "stop" => {
                pomodoro_clone.lock().unwrap().pause();
                break;
            }
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
    pomodoro.lock().unwrap().save_state();
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
