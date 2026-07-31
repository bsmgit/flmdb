use clap::{Parser, Subcommand};
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

const DB_FILE: &str = "flmdb.db";

/// Movie structure matching database records
#[derive(Debug, Serialize, Deserialize)]
struct Movie {
    id: i32,
    imdb_id: Option<String>,
    title: String,
    year: i32,
    format: String,
    director: Option<String>,
    genre: Option<String>,
    runtime: Option<String>,
    plot: Option<String>,
}

/// Serde struct to map OMDb API response JSON
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct OmdbApiResponse {
    title: Option<String>,
    year: Option<String>,
    director: Option<String>,
    genre: Option<String>,
    runtime: Option<String>,
    plot: Option<String>,
    response: String,
    error: Option<String>,
}

#[derive(Parser)]
#[command(name = "flmdb")]
#[command(about = "Film Database CLI (SQLite Engine with Metadata Hydration)", long_about = None)]
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

        /// Optional IMDb ID (e.g., tt0133093)
        #[arg(short, long)]
        imdb: Option<String>,
    },

    /// List films in the catalog (optional search filter)
    List {
        /// Optional search query to filter titles
        #[arg(short, long)]
        search: Option<String>,

        /// Display extended metadata (plot, director, etc.)
        #[arg(short, long)]
        verbose: bool,
    },

    /// Export the catalog to a CSV file
    Export {
        #[arg(short, long, default_value = "catalog.csv")]
        file: String,
    },

    /// Import films from a CSV file (Headers expected: title,year,format,imdb_id)
    Import {
        #[arg(short, long)]
        file: String,
    },

    /// Hydrate missing metadata using the OMDb API (Requires OMDB_API_KEY env var)
    Hydrate {
        /// Hydrate a specific film by IMDb ID (e.g. tt0133093) or all unhydrated entries if omitted
        #[arg(short, long)]
        imdb: Option<String>,
    },
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let conn = init_db()?;

    match cli.command {
        Commands::Add { title, year, format, imdb } => {
            conn.execute(
                "INSERT INTO movies (title, year, format, imdb_id) VALUES (?1, ?2, ?3, ?4)",
                params![title, year, format, imdb],
            )?;
            println!("✅ Added: \"{}\" ({}) [{}]", title, year, format);
        }

        Commands::List { search, verbose } => {
            let mut count = 0;

            if verbose {
                println!(
                    "{:<5} {:<30} {:<6} {:<8} {:<12} {:<20} {:<15}",
                    "ID", "Title", "Year", "Format", "IMDb ID", "Director", "Genre"
                );
                println!("{}", "-".repeat(100));
            } else {
                println!("{:<5} {:<40} {:<6} {:<8}", "ID", "Title", "Year", "Format");
                println!("{}", "-".repeat(62));
            }

            let mut query_str = "SELECT id, imdb_id, title, year, format, director, genre, runtime, plot FROM movies".to_string();

            if search.is_some() {
                query_str.push_str(" WHERE title LIKE ?1");
            }
            query_str.push_str(" ORDER BY title ASC");

            let mut stmt = conn.prepare(&query_str)?;

            let map_row = |row: &rusqlite::Row| map_movie_row(row);

            let movie_iter: Box<dyn Iterator<Item = Result<Movie>>> = if let Some(query) = search {
                let search_pattern = format!("%{}%", query);
                Box::new(stmt.query_map([search_pattern], map_row)?)
            } else {
                Box::new(stmt.query_map([], map_row)?)
            };

            for movie in movie_iter {
                let m = movie?;
                if verbose {
                    println!(
                        "{:<5} {:<30} {:<6} {:<8} {:<12} {:<20} {:<15}",
                        m.id,
                        m.title,
                        m.year,
                        m.format,
                        m.imdb_id.as_deref().unwrap_or("-"),
                        m.director.as_deref().unwrap_or("-"),
                        m.genre.as_deref().unwrap_or("-")
                    );
                    if let Some(plot) = &m.plot {
                        println!("      Plot: {}", plot);
                    }
                } else {
                    println!("{:<5} {:<40} {:<6} {:<8}", m.id, m.title, m.year, m.format);
                }
                count += 1;
            }

            if count == 0 {
                println!("No matching films found.");
            } else {
                println!("{}", "-".repeat(if verbose { 100 } else { 62 }));
                println!("Total records displayed: {}", count);
            }
        }

        Commands::Export { file } => {
            let mut stmt = conn.prepare(
                "SELECT id, imdb_id, title, year, format, director, genre, runtime, plot FROM movies ORDER BY id ASC",
            )?;
            let movie_iter = stmt.query_map([], map_movie_row)?;

            let mut wtr = csv::Writer::from_path(&file)?;
            wtr.write_record([
                "id",
                "imdb_id",
                "title",
                "year",
                "format",
                "director",
                "genre",
                "runtime",
                "plot",
            ])?;

            let mut count = 0;
            for movie in movie_iter {
                let m = movie?;
                wtr.write_record(&[
                    m.id.to_string(),
                    m.imdb_id.unwrap_or_default(),
                    m.title,
                    m.year.to_string(),
                    m.format,
                    m.director.unwrap_or_default(),
                    m.genre.unwrap_or_default(),
                    m.runtime.unwrap_or_default(),
                    m.plot.unwrap_or_default(),
                ])?;
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
                imdb_id: Option<String>,
            }

            let tx = conn.unchecked_transaction()?;
            let mut count = 0;

            for result in rdr.deserialize() {
                let record: CsvRecord = result?;
                tx.execute(
                    "INSERT INTO movies (title, year, format, imdb_id) VALUES (?1, ?2, ?3, ?4)",
                    params![record.title, record.year, record.format, record.imdb_id],
                )?;
                count += 1;
            }

            tx.commit()?;
            println!("⚡ Successfully imported {} films into flmdb from '{}'", count, file);
        }

        Commands::Hydrate { imdb } => {
            let api_key = match std::env::var("OMDB_API_KEY") {
                Ok(key) => key,
                Err(_) => {
                    eprintln!("❌ Error: OMDB_API_KEY environment variable is not set.");
                    eprintln!("Please set it via: export OMDB_API_KEY=\"your_key\"");
                    return Ok(());
                }
            };

            let mut targets: Vec<(i32, String)> = Vec::new();

            if let Some(target_imdb) = imdb {
                let mut stmt = conn.prepare("SELECT id, imdb_id FROM movies WHERE imdb_id = ?1")?;
                let mut rows = stmt.query([target_imdb])?;
                if let Some(row) = rows.next()? {
                    let id: i32 = row.get(0)?;
                    let imdb_id: String = row.get(1)?;
                    targets.push((id, imdb_id));
                } else {
                    println!("No record found with IMDb ID: {}", target_imdb);
                    return Ok(());
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, imdb_id FROM movies WHERE imdb_id IS NOT NULL AND (director IS NULL OR plot IS NULL)",
                )?;
                let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
                for row in rows {
                    targets.push(row?);
                }
            }

            if targets.is_empty() {
                println!("✨ No records require hydration.");
                return Ok(());
            }

            println!("🔍 Hydrating metadata for {} entries...", targets.len());

            for (db_id, imdb_id) in targets {
                let url = format!("http://www.omdbapi.com/?i={}&apikey={}", imdb_id, api_key);
                let response = reqwest::blocking::get(&url)?;

                if response.status().is_success() {
                    let api_data: OmdbApiResponse = response.json()?;

                    if api_data.response == "True" {
                        // Extract clean year integer if available
                        let clean_year = api_data
                            .year
                            .as_deref()
                            .and_then(|y| y.chars().take(4).collect::<String>().parse::<i32>().ok());

                        conn.execute(
                            "UPDATE movies SET 
                                title = COALESCE(?1, title),
                                year = COALESCE(?2, year),
                                director = ?3,
                                genre = ?4,
                                runtime = ?5,
                                plot = ?6
                            WHERE id = ?7",
                            params![
                                api_data.title,
                                clean_year,
                                api_data.director,
                                api_data.genre,
                                api_data.runtime,
                                api_data.plot,
                                db_id
                            ],
                        )?;
                        println!("  ✓ Hydrated: IMDb ID {}", imdb_id);
                    } else {
                        println!("  ✗ API Error for {}: {}", imdb_id, api_data.error.unwrap_or_default());
                    }
                } else {
                    println!("  ✗ HTTP Request failed for IMDb ID {}", imdb_id);
                }

                // Sleep briefly to respect API rate limits
                sleep(Duration::from_millis(150));
            }

            println!("🎉 Hydration complete.");
        }
    }

    Ok(())
}

fn map_movie_row(row: &rusqlite::Row) -> Result<Movie> {
    Ok(Movie {
        id: row.get(0)?,
        imdb_id: row.get(1)?,
        title: row.get(2)?,
        year: row.get(3)?,
        format: row.get(4)?,
        director: row.get(5)?,
        genre: row.get(6)?,
        runtime: row.get(7)?,
        plot: row.get(8)?,
    })
}

fn init_db() -> Result<Connection> {
    let conn = Connection::open(DB_FILE)?;

    conn.pragma_update(None, "journal_mode", "WAL")?;

    // 1. Initial base table schema creation
    conn.execute(
        "CREATE TABLE IF NOT EXISTS movies (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            title       TEXT NOT NULL,
            year        INTEGER NOT NULL,
            format      TEXT NOT NULL,
            imdb_id     TEXT UNIQUE,
            director    TEXT,
            genre       TEXT,
            runtime     TEXT,
            plot        TEXT
        )",
        [],
    )?;

    // 2. Safe Schema Migrations (applies to older databases without crashing)
    let migrations = [
        "ALTER TABLE movies ADD COLUMN imdb_id TEXT UNIQUE",
        "ALTER TABLE movies ADD COLUMN director TEXT",
        "ALTER TABLE movies ADD COLUMN genre TEXT",
        "ALTER TABLE movies ADD COLUMN runtime TEXT",
        "ALTER TABLE movies ADD COLUMN plot TEXT",
    ];

    for migration in migrations {
        let _ = conn.execute(migration, []);
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_movies_title ON movies(title)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_movies_imdb ON movies(imdb_id)",
        [],
    )?;

    Ok(conn)
}