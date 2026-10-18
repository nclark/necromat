// Parts of the language, evaluator and storage modules are not used by the
// UI yet, so dead-code warnings are allowed per module for now.
#[allow(dead_code, unused_imports)]
mod eval;
mod eval_cmd;
#[allow(dead_code, unused_imports)]
mod lang;
#[allow(dead_code, unused_imports)]
mod storage;
mod tui;

use std::process::ExitCode;

use storage::Command;

fn main() -> ExitCode {
    match storage::parse_args(std::env::args_os().skip(1)) {
        Ok(Command::Eval) => eval_cmd::run(),
        Ok(Command::Edit(file)) => match storage::resolve_startup(file.as_deref()) {
            Ok(startup) => {
                let session_mode = startup.session_mode();
                match tui::run(startup.lines, startup.file_path, session_mode) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("nm: {e}");
                        ExitCode::FAILURE
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Ok(Command::Help) => {
            println!("{}", storage::USAGE);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nm: {e}\n{}", storage::USAGE);
            ExitCode::from(2)
        }
    }
}
