mod app;
mod md_style;
mod provider;
mod session;
mod sysmon;
mod theme;
mod ui;

use std::io;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::Config;

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

fn main() -> io::Result<()> {
    for arg in std::env::args().skip(1) {
        if arg == "--version" || arg == "-V" {
            println!("veritas-agent {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    }

    // Tokio runtime for the OpenAI-compatible SSE client. The TUI loop itself
    // stays synchronous; async work is spawned onto this runtime.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _guard = rt.enter();

    let config = Config::from_env();

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_panic(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = app::run(&mut terminal, config);

    restore_terminal();
    terminal.show_cursor()?;

    res
}
