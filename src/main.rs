use clap::{Parser, Subcommand};
use rusqlite::{params, Connection, Result, ToSql};
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
#[command(version)]
#[command(
    about = "Film Database CLI",
    long_about = "flmdb is a fast, terminal-first cataloging tool for physical and digital movie collections.\nIt tracks film media, standardizes unique IMDb identifiers, and hydrates rich metadata directly from the OMDb API.\n\nAPI KEY SETUP:\n  To use 'flmdb hydrate', you need an OMDb API key.\n  Save it to your database securely by running:\n      flmdb api-key set YOUR_KEY"
)]
#[command(after_help = "LICENSE:\n    MIT License\n\n    Copyright (c) 2026 flmdb contributors\n\n    Permission is hereby granted, free of charge, to any person obtaining a copy\n    of this software and associated documentation files (the \"Software\"), to deal\n    in the Software without restriction, including without limitation the rights\n    to use, copy, modify, merge, publish, distribute, sublicense, and/or sell\n    copies of the Software, and to permit persons to whom it is furnished to do\n    so, subject to the following conditions:\n\n    The above copyright notice and this permission notice shall be included in all\n    copies or substantial portions of the Software.\n\n    THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\n    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\n    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\n    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\n    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\n    OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\n    SOFTWARE.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new film to the catalog manually
    Add {
        /// Movie Title (e.g., "The Matrix")
        #[arg(short, long)]
        title: String,

        /// Release Year (e.g., 1999)
        #[arg(short, long)]
        year: i32,

        /// Physical or Digital Format (e.g., DVD, 4K, Blu-ray, VHS)
        #[arg(short, long, default_value = "DVD")]
        format: String,

        /// Unique IMDb Identifier (e.g., tt0133093)
        #[arg(short, long)]
        imdb: Option<String>,
    },

    /// Edit an existing film's details by its local ID
    Edit {
        /// The local ID of the movie to edit (find this using 'flmdb list')
        id: i32,

        /// Update Movie Title
        #[arg(short, long)]
        title: Option<String>,

        /// Update Release Year
        #[arg(short, long)]
        year: Option<i32>,

        /// Update Physical or Digital Format
        #[arg(short, long)]
        format: Option<String>,

        /// Update Unique IMDb Identifier (e.g., tt0133093)
        #[arg(short, long)]
        imdb: Option<String>,
    },

    /// Delete a film from the catalog by its local ID
    Delete {
        /// The local ID of the movie to delete (find this using 'flmdb list')
        id: i32,
    },

    /// List films in the catalog with primary identifiers
    List {
        /// Optional search query to filter titles
        #[arg(short, long)]
        search: Option<String>,

        /// Display extended metadata (director, genre, and full plot summaries)
        #[arg(short, long)]
        verbose: bool,
    },

    /// Export the catalog to a CSV file
    Export {
        /// Destination CSV file path
        #[arg(short, long, default_value = "catalog.csv")]
        file: String,
    },

    /// Import films from a CSV file
    Import {
        /// Path to the source CSV file
        #[arg(short, long)]
        file: String,
    },

    /// Hydrate missing metadata using the OMDb API
    Hydrate {
        /// Hydrate a single specific film by its IMDb ID. If omitted, hydrates all records missing metadata.
        #[arg(short, long)]
        imdb: Option<String>,
    },
    
    /// Manage your OMDb API key
    ApiKey {
        #[command(subcommand)]
        command: ApiKeyCommands,
    },
}

#[derive(Subcommand)]
enum ApiKeyCommands {
    /// Set and save your OMDb API key
    Set {
        /// Your OMDb API key
        key: String,
    },
    /// List/View your currently saved OMDb API key
    List,
    /// Reset/Remove your saved OMDb API key
    Reset,
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let conn = init_db()?;

    match cli.command {
        Commands::ApiKey { command } => match command {
            ApiKeyCommands::Set { key } => {
                conn.execute(
                    "REPLACE INTO config (key, value) VALUES ('omdb_api_key', ?1)",
                    params![key],
                )?;
                println!("✅ OMDb API key saved successfully.");
            }
            ApiKeyCommands::List => {
                let db_key: rusqlite::Result<String> = conn.query_row(
                    "SELECT value FROM config WHERE key = 'omdb_api_key'",
                    [],
                    |row| row.get(0),
                );
                match db_key {
                    Ok(k) => println!("🔑 Current OMDb API Key: {}", k),
                    Err(_) => {
                        println!("No API key is currently saved.");
                        println!("Set one using: flmdb api-key set YOUR_KEY");
                    }
                }
            }
            ApiKeyCommands::Reset => {
                conn.execute("DELETE FROM config WHERE key = 'omdb_api_key'", [])?;
                println!("🗑️ OMDb API key has been reset/removed.");
            }
        },

        Commands::Add {
            title,
            year,
            format,
            imdb,
        } => {
            conn.execute(
                "INSERT INTO movies (title, year, format, imdb_id) VALUES (?1, ?2, ?3, ?4)",
                params![title, year, format, imdb],
            )?;
            let imdb_str = imdb.as_deref().unwrap_or("None");
            println!(
                "✅ Added: \"{}\" ({}) [{}] (IMDb ID: {})",
                title, year, format, imdb_str
            );
        }

        Commands::Edit { id, title, year, format, imdb } => {
            let mut updates = Vec::new();
            let mut params_vec: Vec<Box<dyn ToSql>> = Vec::new();
            let mut param_index = 1;

            if let Some(t) = title {
                updates.push(format!("title = ?{}", param_index));
                params_vec.push(Box::new(t));
                param_index += 1;
            }
            if let Some(y) = year {
                updates.push(format!("year = ?{}", param_index));
                params_vec.push(Box::new(y));
                param_index += 1;
            }
            if let Some(f) = format {
                updates.push(format!("format = ?{}", param_index));
                params_vec.push(Box::new(f));
                param_index += 1;
            }
            if let Some(i) = imdb {
                updates.push(format!("imdb_id = ?{}", param_index));
                params_vec.push(Box::new(i));
                param_index += 1;
            }

            if updates.is_empty() {
                println!("⚠️ No fields provided to update. Use flags like -t, -y, -f, or -i.");
                return Ok(());
            }

            let query = format!("UPDATE movies SET {} WHERE id = ?{}", updates.join(", "), param_index);
            params_vec.push(Box::new(id));

            let param_refs: Vec<&dyn ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
            
            let rows_updated = conn.execute(&query, param_refs.as_slice())?;
            
            if rows_updated > 0 {
                println!("✅ Successfully updated movie ID {}.", id);
            } else {
                println!("❌ Error: No movie found with ID {}.", id);
            }
        }

        Commands::Delete { id } => {
            let rows_deleted = conn.execute("DELETE FROM movies WHERE id = ?1", params![id])?;
            if rows_deleted > 0 {
                println!("🗑️ Successfully deleted movie ID {}.", id);
            } else {
                println!("❌ Error: No movie found with ID {}.", id);
            }
        }

        Commands::List { search, verbose } => {
            let mut count = 0;

            if verbose {
                println!(
                    "{:<5} {:<12} {:<30} {:<6} {:<8} {:<20} {:<15}",
                    "ID", "IMDb ID", "Title", "Year", "Format", "Director", "Genre"
                );
                println!("{}", "-".repeat(100));
            } else {
                println!(
                    "{:<5} {:<12} {:<35} {:<6} {:<8}",
                    "ID", "IMDb ID", "Title", "Year", "Format"
                );
                println!("{}", "-".repeat(70));
            }

            let mut query_str =
                "SELECT id, imdb_id, title, year, format, director, genre, runtime, plot FROM movies"
                    .to_string();

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
                let imdb_display = m.imdb_id.as_deref().unwrap_or("-");

                if verbose {
                    println!(
                        "{:<5} {:<12} {:<30} {:<6} {:<8} {:<20} {:<15}",
                        m.id,
                        imdb_display,
                        m.title,
                        m.year,
                        m.format,
                        m.director.as_deref().unwrap_or("-"),
                        m.genre.as_deref().unwrap_or("-")
                    );
                    if let Some(plot) = &m.plot {
                        println!("      Plot: {}", plot);
                    }
                } else {
                    println!(
                        "{:<5} {:<12} {:<35} {:<6} {:<8}",
                        m.id, imdb_display, m.title, m.year, m.format
                    );
                }
                count += 1;
            }

            if count == 0 {
                println!("No matching films found.");
            } else {
                println!("{}", "-".repeat(if verbose { 100 } else { 70 }));
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
            let api_key = match conn.query_row(
                "SELECT value FROM config WHERE key = 'omdb_api_key'",
                [],
                |row| row.get::<_, String>(0),
            ) {
                Ok(key) => key,
                Err(_) => match std::env::var("OMDB_API_KEY") {
                    Ok(key) => key,
                    Err(_) => {
                        eprintln!("❌ Error: No OMDb API key found.");
                        eprintln!("Please save your key by running: flmdb api-key set YOUR_KEY");
                        return Ok(());
                    }
                },
            };

            let mut targets: Vec<(i32, String)> = Vec::new();

            if let Some(ref target_imdb) = imdb {
                let mut stmt = conn.prepare("SELECT id, imdb_id FROM movies WHERE imdb_id = ?1")?;
                let mut rows = stmt.query([target_imdb.as_str()])?;
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

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