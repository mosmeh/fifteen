mod board;

use anyhow::Result;
use board::{BoardBuilder, Direction};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use crossterm::{cursor, event, style, terminal};
use std::io::{self, Write};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    author = env!("CARGO_PKG_AUTHORS"),
    setting(clap::AppSettings::ColoredHelp),
    setting(clap::AppSettings::DeriveDisplayOrder),
)]
struct Opt {
    /// Image file
    file: Option<PathBuf>,

    /// Play on n x n board
    #[structopt(short, default_value = "4")]
    n: usize,

    /// Crops an image instead of stretching
    #[structopt(short, long)]
    crop: bool,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    anyhow::ensure!(opt.n > 1, "n must be 2 or larger");

    let mut builder = BoardBuilder::new();
    builder
        .board_size(opt.n)
        .crop_image(opt.crop)
        .terminal_size(terminal::size()?);
    if let Some(file) = opt.file {
        builder.image(file);
    }
    let mut board = builder.build()?;

    let (tx, rx) = crossbeam_channel::unbounded();
    std::thread::spawn(move || loop {
        if let Ok(event) = event::read() {
            let _ = tx.send(event);
        }
    });

    let mut stdout = setup_terminal()?;
    board.draw(&mut stdout)?;
    stdout.flush()?;

    loop {
        if let Event::Key(key) = rx.recv()? {
            match (key.modifiers, key.code) {
                (_, KeyCode::Esc)
                | (KeyModifiers::CONTROL, KeyCode::Char('c'))
                | (_, KeyCode::Char('q')) => break,
                (_, KeyCode::Up) | (_, KeyCode::Char('k')) | (_, KeyCode::Char('w')) => {
                    board.move_and_draw_tiles(&mut stdout, Direction::Up)?;
                }
                (_, KeyCode::Down) | (_, KeyCode::Char('j')) | (_, KeyCode::Char('s')) => {
                    board.move_and_draw_tiles(&mut stdout, Direction::Down)?;
                }
                (_, KeyCode::Left) | (_, KeyCode::Char('h')) | (_, KeyCode::Char('a')) => {
                    board.move_and_draw_tiles(&mut stdout, Direction::Left)?;
                }
                (_, KeyCode::Right) | (_, KeyCode::Char('l')) | (_, KeyCode::Char('d')) => {
                    board.move_and_draw_tiles(&mut stdout, Direction::Right)?;
                }
                _ => (),
            }

            if board.is_solved() {
                break;
            }
        }
    }

    crossterm::queue!(
        stdout,
        cursor::MoveTo(0, board.image_size() as u16 / 2),
        style::ResetColor
    )?;
    stdout.flush()?;
    cleanup_terminal(stdout)?;

    Ok(())
}

fn setup_terminal() -> Result<io::Stdout> {
    terminal::enable_raw_mode()?;

    let mut stdout = io::stdout();
    crossterm::queue!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::Hide
    )?;

    Ok(stdout)
}

fn cleanup_terminal<W: Write>(mut w: W) -> Result<()> {
    crossterm::queue!(w, cursor::Show)?;
    terminal::disable_raw_mode()?;

    Ok(())
}
