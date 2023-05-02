use std::env;


pub fn handle_args() -> Option<String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <sound_file>", args[0]);
        return None;
    }

    Some(args[1].clone())
}
