use std::process::ExitCode;

fn main() -> ExitCode {
    match xlsx_dump::run() {
        Ok(code) => code,
        Err(e) => {
            if let Some(io) = e.downcast_ref::<std::io::Error>()
                && io.kind() == std::io::ErrorKind::BrokenPipe
            {
                return ExitCode::SUCCESS;
            }
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}
