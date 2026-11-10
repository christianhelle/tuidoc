//! The `tdoc` command: reads its arguments, then hands the terminal to the editor.

use std::io::{self, Write};
use std::panic;
use std::process::ExitCode;

use crossterm::cursor::SetCursorStyle;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event,
};
use crossterm::execute;
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;
use tuidoc::app::{App, Mode};
use tuidoc::clipboard;
use tuidoc::document::Document;
use tuidoc::ui::{draw, fit, handle_mouse};

const USAGE: &str = "Usage: tdoc [file]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let target = args.first().map_or("a blank document", String::as_str);
    let opened = match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [flag] if flag == "--version" || flag == "-V" => {
            println!("tdoc {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        [] => Document::blank(),
        [path] => Document::open(path),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let mut app = match opened {
        Ok(document) => App::new(document),
        Err(error) => {
            eprintln!("tdoc: cannot open {target}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut terminal = ratatui::init();
    release_terminal_on_panic();
    let result = execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste)
        .and_then(|()| run(&mut terminal, &mut app));
    // A terminal left capturing the mouse would type escape codes into the user's shell.
    let released = release_terminal();
    ratatui::restore();
    if let Err(error) = result.and(released) {
        eprintln!("tdoc: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn release_terminal() -> io::Result<()> {
    execute!(
        io::stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        SetCursorStyle::DefaultUserShape
    )
}

/// Adds to the panic handling `ratatui::init` installed, which knows nothing about the mouse.
fn release_terminal_on_panic() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = release_terminal();
        previous(info);
    }));
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit() {
        let size = terminal.size()?;
        let screen = Rect::new(0, 0, size.width, size.height);
        fit(app, screen);
        terminal.draw(|frame| draw(frame, app))?;
        // A bar where typing goes in, a block where keys are commands.
        let shape = match app.mode() {
            Mode::Normal | Mode::Visual | Mode::VisualLine | Mode::Outline => {
                SetCursorStyle::SteadyBlock
            }
            _ => SetCursorStyle::SteadyBar,
        };
        execute!(io::stdout(), shape)?;
        match event::read()? {
            Event::Key(key) => app.handle_key(key),
            Event::Paste(text) => app.handle_paste(&text),
            Event::Mouse(mouse) => handle_mouse(app, screen, mouse),
            _ => {}
        }
        if let Some(text) = app.take_clipboard() {
            let mut out = io::stdout();
            out.write_all(clipboard::osc52(&text).as_bytes())?;
            out.flush()?;
        }
    }
    Ok(())
}
