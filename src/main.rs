use clap::{Parser, Subcommand};
use rusqlite::{params, Connection, Result};
use serde::Deserialize;
use std::path::Path;

const DB_FILE: &str = "flmdb.db";

/// Movie structure matching database rows
#[derive(Debug)]
struct Movie {
    id: i32,
    title: String,
    year: i32,
    format: String,
}

#[derive(Parser)]
#[command(name = "flmdb")]
#[command(about = "Film Database CLI (SQLite Engine)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new film to the catalog
    Add {
        #[arg(short, long)]
        title: String,

        #[arg(short, long)]
        year: i32,

        #[arg(short, long, default_value = "DVD")]
        format: String,
    },
    /// List films in the catalog (optional search filter)
    List {
        /// Optional search query to filter titles
        #[arg(short, long)]
        search: Option<String>,
    },
    /// Export the catalog to a CSV file
    Export {
        #[arg(short, long, default_value = "catalog.csv")]
        file: String,
    },
    /// Import films from a CSV file (Headers expected: title,year,format)
    Import {
        #[arg(short, long)]
        file: String,
    },
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let conn = init_db()?;

    match cli.command {
        Commands::Add { title, year, format } => {
            conn.execute(
                "INSERT INTO movies (title, year, format) VALUES (?1, ?2, ?3)",
                params![title, year, format],
            )?;
            println!("✅ Added: \"{}\" ({}) [{}]", title, year, format);
        }

        Commands::List { search } => {
            let mut count = 0;
            println!("{:<5} {:<40} {:<6} {:<8}", "ID", "Title", "Year", "Format");
            println!("{}", "-".repeat(62));

            if let Some(query) = search {
                let mut stmt = conn.prepare(
                    "SELECT id, title, year, format FROM movies WHERE title LIKE ?1 ORDER BY title ASC",
                )?;
                let search_pattern = format!("%{}%", query);
                let movie_iter = stmt.query_map([search_pattern], map_movie_row)?;

                for movie in movie_iter {
                    let m = movie?;
                    println!("{:<5} {:<40} {:<6} {:<8}", m.id, m.title, m.year, m.format);
                    count += 1;
                }
            } else {
                let mut stmt = conn.prepare("SELECT id, title, year, format FROM movies ORDER BY title ASC")?;
                let movie_iter = stmt.query_map([], map_movie_row)?;

                for movie in movie_iter {
                    let m = movie?;
                    println!("{:<5} {:<40} {:<6} {:<8}", m.id, m.title, m.year, m.format);
                    count += 1;
                }
            }

            if count == 0 {
                println!("No matching films found.");
            } else {
                println!("{}", "-".repeat(62));
                println!("Total records displayed: {}", count);
            }
        }

        Commands::Export { file } => {
            let mut stmt = conn.prepare("SELECT id, title, year, format FROM movies ORDER BY id ASC")?;
            let movie_iter = stmt.query_map([], map_movie_row)?;

            let mut wtr = csv::Writer::from_path(&file)?;
            wtr.write_record(["id", "title", "year", "format"])?;

            let mut count = 0;
            for movie in movie_iter {
                let m = movie?;
                wtr.write_record(&[&m.id.to_string(), &m.title, &m.year.to_string(), &m.format])?;
                count += 1;
            }
            wtr.flush()?;
            println!("💾 Successfully exported {} items to '{}'", count, file);
        }

        Commands::Import { file } => {
            if !Path::new(&file).exists() {
                return Err(format!("File '{}' not found.", file).into());
            }

            let mut rdr = csv::Reader::from_path(&file)?;

            #[derive(Deserialize)]
            struct CsvRecord {
                title: String,
                year: i32,
                format: String,
            }

            let tx = conn.unchecked_transaction()?;
            let mut count = 0;

            for result in rdr.deserialize() {
                let record: CsvRecord = result?;
                tx.execute(
                    "INSERT INTO movies (title, year, format) VALUES (?1, ?2, ?3)",
                    params![record.title, record.year, record.format],
                )?;
                count += 1;
            }

            tx.commit()?;
            println!("⚡ Successfully imported {} films into flmdb from '{}'", count, file);
        }
    }

    Ok(())
}

fn map_movie_row(row: &rusqlite::Row) -> Result<Movie> {
    Ok(Movie {
        id: row.get(0)?,
        title: row.get(1)?,
        year: row.get(2)?,
        format: row.get(3)?,
    })
}

fn init_db() -> Result<Connection> {
    let conn = Connection::open(DB_FILE)?;

    conn.pragma_update(None, "journal_mode", "WAL")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS movies (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            title   TEXT NOT NULL,
            year    INTEGER NOT NULL,
            format  TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_movies_title ON movies(title)",
        [],
    )?;

    Ok(conn)
}