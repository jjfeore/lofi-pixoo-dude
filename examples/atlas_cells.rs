//! Mechanically extract original-resolution RGBA cells for targeted art edits.
//! No creative pixel changes, alpha flattening, or per-cell fitting.
use anyhow::{Context, Result, ensure};
use std::{fs, path::PathBuf};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 4,
        "usage: atlas_cells INPUT OUTPUT_DIRECTORY COLUMNS COUNT"
    );
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let columns: u32 = args[2].parse().context("columns")?;
    let count: u32 = args[3].parse().context("count")?;
    ensure!(
        (1..=40).contains(&columns) && (1..=40).contains(&count),
        "invalid grid"
    );
    ensure!(!output.exists(), "output exists; choose a new directory");
    let rows = count.div_ceil(columns);
    let source = image::open(input)?.to_rgba8();
    ensure!(
        source.width() >= columns && source.height() >= rows,
        "empty cells"
    );
    fs::create_dir_all(&output)?;
    for i in 0..count {
        let column = i % columns;
        let row = i / columns;
        let x0 = column * source.width() / columns;
        let x1 = (column + 1) * source.width() / columns;
        let y0 = row * source.height() / rows;
        let y1 = (row + 1) * source.height() / rows;
        image::imageops::crop_imm(&source, x0, y0, x1 - x0, y1 - y0)
            .to_image()
            .save(output.join(format!("{i:03}.png")))?;
    }
    println!(
        "Extracted {count} original-resolution cells into {}",
        output.display()
    );
    Ok(())
}
