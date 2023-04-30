# Pomodoro Timer

A simple command-line Pomodoro timer app built with Rust. The timer can be started, paused, and toggled, and automatically switches between Pomodoro, short break, and long break states according to predefined durations. The app also uses a named pipe to read commands and serialize the current state of the timer to a JSON file.

## Features

- Start, pause, and toggle timer
- Automatically switch between Pomodoro, short break, and long break states
- Read commands from a named pipe
- Serialize current state of the timer to a JSON file

## Usage

To use the Pomodoro timer app, simply run the following command:

`cargo run`

The timer will start running automatically and print the current state of the timer, including the elapsed time and remaining time.

To interact with the timer, write one of the following commands to the named pipe:

- `start`: Start the timer.
- `pause`: Pause the timer.
- `toggle`: Toggle the timer between running and paused states.
- `stop`: Stop the timer.

The app will automatically switch between Pomodoro, short break, and long break states according to predefined durations. The current state of the timer is serialized to a JSON file when the timer is stopped.

## License

This Pomodoro Timer app is licensed under the [MIT License](https://opensource.org/licenses/MIT). Feel free to use, modify, and distribute it as you like.

