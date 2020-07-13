use anyhow::Result;
use crossterm::style::Color;
use crossterm::{cursor, style};
use image::GenericImageView;
use itertools::Itertools;
use rand::prelude::*;
use std::io::Write;
use std::path::{Path, PathBuf};

pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub struct Board {
    img_pixels: Vec<Color>,
    img_size: usize,
    board_size: usize,
    tiles: Vec<usize>,
}

impl Board {
    pub fn image_size(&self) -> usize {
        self.img_size
    }

    pub fn is_solved(&self) -> bool {
        self.tiles.iter().enumerate().all(|(i, t)| i == *t)
    }

    pub fn draw<W: Write>(&self, w: &mut W) -> Result<()> {
        for i in 0..self.board_size * self.board_size {
            self.draw_tile(w, i)?;
        }
        Ok(())
    }

    pub fn move_and_draw_tiles<W: Write>(&mut self, w: &mut W, direction: Direction) -> Result<()> {
        if let Some((a, b)) = self.move_tiles(direction) {
            self.draw_tile(w, a)?;
            self.draw_tile(w, b)?;
            w.flush()?;
        }
        Ok(())
    }

    pub fn move_tiles(&mut self, direction: Direction) -> Option<(usize, usize)> {
        let (dx, dy) = match direction {
            Direction::Up => (0, 1),
            Direction::Down => (0, -1),
            Direction::Left => (1, 0),
            Direction::Right => (-1, 0),
        };

        let blank_tile = self
            .tiles
            .iter()
            .position(|t| *t == blank_tile_num(self.board_size))
            .unwrap();

        let (blank_x, blank_y) = (
            (blank_tile % self.board_size) as isize,
            (blank_tile / self.board_size) as isize,
        );
        let (dest_x, dest_y) = (blank_x + dx, blank_y + dy);

        let movable_x = 0 <= dest_x && dest_x < self.board_size as isize;
        let movable_y = 0 <= dest_y && dest_y < self.board_size as isize;

        if movable_x && movable_y {
            let dest_tile = dest_x as usize + dest_y as usize * self.board_size;
            self.tiles.swap(blank_tile, dest_tile);

            Some((blank_tile, dest_tile))
        } else {
            None
        }
    }

    fn draw_tile<W: Write>(&self, w: &mut W, index: usize) -> Result<()> {
        let img_pos = self.tiles[index];
        let (img_x, img_y) = (img_pos % self.board_size, img_pos / self.board_size);
        let (img_x, img_y) = (
            img_x * self.img_size / self.board_size,
            img_y * self.img_size / self.board_size,
        );

        let screen_pos = index;
        let (screen_x, screen_y) = (screen_pos % self.board_size, screen_pos / self.board_size);
        let (screen_x, screen_y) = (
            screen_x * self.img_size / self.board_size,
            screen_y * self.img_size / self.board_size / 2,
        );

        let width = self.img_size / self.board_size;
        let height = width / 2;

        if img_pos == blank_tile_num(self.board_size) {
            for screen_y in screen_y..screen_y + height {
                crossterm::queue!(
                    w,
                    cursor::MoveTo(screen_x as u16, screen_y as u16),
                    style::ResetColor,
                    style::Print(" ".repeat(width))
                )?;
            }
            return Ok(());
        }

        let y_pos = (screen_y..screen_y + height).zip((img_y..img_y + 2 * height).step_by(2));

        for (screen_y, img_y) in y_pos {
            crossterm::queue!(w, cursor::MoveTo(screen_x as u16, screen_y as u16))?;

            let upper = self.img_pixels[img_x + img_y * self.img_size..][..width].iter();
            let lower = self.img_pixels[img_x + (img_y + 1) * self.img_size..][..width].iter();
            let runs = upper
                .zip(lower)
                .map(|x| (x, 1))
                .coalesce(|(a, a_len), (b, b_len)| {
                    if a == b {
                        Ok((a, a_len + b_len))
                    } else {
                        Err(((a, a_len), (b, b_len)))
                    }
                });

            for ((upper, lower), len) in runs {
                match (upper, lower) {
                    (Color::Reset, Color::Reset) => {
                        crossterm::queue!(w, style::ResetColor, style::Print(" ".repeat(len)))?;
                    }
                    (Color::Reset, fg) => {
                        crossterm::queue!(
                            w,
                            style::SetForegroundColor(*fg),
                            style::SetBackgroundColor(Color::Reset),
                            style::Print("▄".repeat(len))
                        )?;
                    }
                    (fg, bg) => {
                        crossterm::queue!(
                            w,
                            style::SetForegroundColor(*fg),
                            style::SetBackgroundColor(*bg),
                            style::Print("▀".repeat(len))
                        )?;
                    }
                }
            }

            crossterm::queue!(w, style::ResetColor)?;
        }

        Ok(())
    }
}

pub struct BoardBuilder {
    image: Option<PathBuf>,
    crop_image: bool,
    terminal_size: (u16, u16),
    board_size: usize,
}

impl BoardBuilder {
    pub fn new() -> Self {
        Self {
            image: None,
            crop_image: false,
            terminal_size: (80, 24),
            board_size: 4,
        }
    }

    pub fn build(&self) -> Result<Board> {
        let (term_width, term_height) = self.terminal_size;

        let max_img_size = ((term_height - 1) * 2).min(term_width) as usize;
        let mut img_size = self.board_size;
        loop {
            if img_size * 2 <= max_img_size {
                img_size *= 2;
            } else {
                break;
            }
        }
        let board_lines = img_size / 2;
        if self.board_size > board_lines {
            return Err(anyhow::anyhow!("n is too large"));
        }

        let pixels = load_image(self.image.as_ref(), img_size as u32, self.crop_image)?;

        let tiles = generate_tiles(self.board_size);
        assert!(is_solvable(self.board_size, &tiles));

        let board = Board {
            img_pixels: pixels,
            img_size,
            board_size: self.board_size,
            tiles,
        };
        Ok(board)
    }

    pub fn image<P: AsRef<Path>>(&mut self, image: P) -> &mut Self {
        self.image = Some(image.as_ref().to_path_buf());
        self
    }

    pub fn crop_image(&mut self, yes: bool) -> &mut Self {
        self.crop_image = yes;
        self
    }

    pub fn terminal_size(&mut self, terminal_size: (u16, u16)) -> &mut Self {
        self.terminal_size = terminal_size;
        self
    }

    pub fn board_size(&mut self, board_size: usize) -> &mut Self {
        self.board_size = board_size;
        self
    }
}

static DEFAULT_IMAGE: &[u8] = include_bytes!("../img/default.png");

fn load_image<P: AsRef<Path>>(path: Option<P>, size: u32, crop: bool) -> Result<Vec<Color>> {
    let mut img = if let Some(path) = path {
        image::io::Reader::open(path)?
            .with_guessed_format()?
            .decode()?
    } else {
        image::load_from_memory(DEFAULT_IMAGE)?
    };

    if crop {
        let (width, height) = img.dimensions();
        let crop_size = width.min(height);
        img = img.crop(0, 0, crop_size, crop_size);
    }
    let img = img.thumbnail_exact(size, size);

    let pixels: Vec<_> = img
        .pixels()
        .map(|(_, _, data)| {
            if data[3] == 0 {
                Color::Reset
            } else {
                Color::Rgb {
                    r: data[0],
                    g: data[1],
                    b: data[2],
                }
            }
        })
        .collect();

    let (width, height) = img.dimensions();
    assert_eq!(width, height);
    assert_eq!(width, size);

    Ok(pixels)
}

const fn blank_tile_num(n: usize) -> usize {
    n * n - 1
}

fn generate_tiles(n: usize) -> Vec<usize> {
    let mut rng = rand::thread_rng();
    let mut tiles: Vec<_> = (0..n * n).collect();
    tiles.shuffle(&mut rng);
    while !is_solvable(n, &tiles) {
        tiles.shuffle(&mut rng);
    }
    tiles
}

// See https://www.cs.bham.ac.uk/~mdr/teaching/modules04/java2/TilesSolvability.html
fn is_solvable(n: usize, tiles: &[usize]) -> bool {
    let inversions = tiles
        .iter()
        .enumerate()
        .filter(|(_, a)| **a != blank_tile_num(n))
        .fold(0, |sum, (i, a)| {
            sum + tiles[i + 1..]
                .iter()
                .filter(|b| **b != blank_tile_num(n) && *b < a)
                .count()
        });

    if n % 2 == 0 {
        let blank_tile = tiles.iter().position(|t| *t == blank_tile_num(n)).unwrap();
        let blank_row = n - blank_tile / n;
        (blank_row % 2 == 0) ^ (inversions % 2 == 0)
    } else {
        inversions % 2 == 0
    }
}
