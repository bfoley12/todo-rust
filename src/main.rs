use clap::Parser;
use csv::{WriterBuilder, Writer};
use std::fs::{File, OpenOptions};
use chrono::{DateTime, Local, Utc};
use serde::{Serialize, Deserialize};
use std::fmt::Write;
use csv::Trim;
use textwrap::wrap;

// Standard clap argument parser
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "")]
    add: String,

    #[arg(short, long, default_value = "General")]
    project: String,
    
    #[arg(short, long, default_value_t = 0)]
    complete: u32,

    #[arg(short, long, default_value = "")]
    sort: String,

    #[arg(short, long, default_value = "")]
    filter: String,

    #[arg(short, long, action = clap::ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,
}

// Need to add serde traits to put into csv
#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct Record {
    id: u32,
    project: String,
    desc: String,
    completed: bool,
    // Need to load chrono with serde feature in Cargo.toml
    // Lets us serialize chrono DateTimes
    #[serde(with = "chrono::serde::ts_seconds")]
    date_added: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    date_completed: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone)]
enum Op { Eq, Ne, Contains, IContains }

fn parse_op(s: &str) -> Option<Op> {
    match s {
        "=="  => Some(Op::Eq),
        "!="  => Some(Op::Ne),
        "~="  => Some(Op::Contains),   // substring
        "~*=" => Some(Op::IContains),  // case-insensitive substring
        _ => None,
    }
}

fn filter_project(records: &mut Vec<Record>, op: Op, needle: String) {
    let needle_lc = needle.to_lowercase();
    records.retain(|r| match op {
        Op::Eq        => r.project == needle,
        Op::Ne        => r.project != needle,
        Op::Contains  => r.project.contains(&needle),
        Op::IContains => r.project.to_lowercase().contains(&needle_lc),
    });
}

fn main() {
    let filepath: String = String::from("C:/Users/brend/Documents/Rust Learning/todo/list.csv");
    let args = Args::parse();
    // Make file if needed
    let mut write_file = open_csv_writer_append(&filepath);


    // Current implementation: allows calling again for same ID to flip back to incomplete
    // Is it a bug or a feature? Not sure...
    // Adding todo item
    if !args.add.is_empty() {
        // count records first
        let mut read_file = open_csv_reader(&filepath);
        let num_records = read_file.records().count() as u32;

        let record = Record {
            id: num_records + 1,
            project: args.project.clone(),
            desc: args.add.clone(),
            completed: false,
            date_added: Local::now().into(),
            date_completed: None,
        };

        write_file.serialize(record).expect("serialize");
        write_file.flush().expect("flush");
    }
    // Current implementation: allows calling again for same ID to flip back to incomplete
    // Is it a bug or a feature? Not sure...
    // Mark todo item as complete by specifying ID
    else if args.complete > 0 {
        // 1) read all
        let mut read_file = open_csv_reader(&filepath);
        let mut rec_vec: Vec<Record> = Vec::new();
        for res in read_file.deserialize::<Record>() {
            match res {
                Ok(rec) => rec_vec.push(rec),
                Err(e) => {
                    eprintln!("Skipping bad row: {e} (pos: {:?})", e.position());
                }
            }
        }

        // 2) mutate the matching ID
        let target_id = args.complete;
        for rec in rec_vec.iter_mut() {
            if rec.id == target_id {
                rec.completed = !rec.completed;                 // toggle if you like
                rec.date_completed = if rec.completed {
                   Some(Utc::now())
            } else {
                None
            };
                break;
            }
        }

        // 3) rewrite whole file (truncate)
        let mut write_file = open_csv_writer_truncate(&filepath);
        for rec in rec_vec {
            write_file.serialize(rec).expect("serialize");
        }
        write_file.flush().expect("flush");
    }
    
    // Regardless of command put in, print
    let mut read_file = open_csv_reader(&filepath);
    println!("-------------------------------------------------------------------------------------------------------");
    println!(
        "| {:<3} | {:<15} | {:<28} | {:<11} | {:<12} | {:<15} |",
        "ID", "Project", "Description", "Completed", "Date Added", "Date Completed"
    );
    println!("-------------------------------------------------------------------------------------------------------");
    // Load records into a vec
    let mut records = Vec::new();
    for record in read_file.deserialize::<Record>(){
        match record {
            Ok(rec) => records.push(rec),
            Err(e) => eprintln!("Skipping bad row: {e} (pos: {:?})", e.position())
        }
    }
    // Filter entries
    if !args.filter.is_empty() {
        let mut it = args.filter.split_whitespace();
        let field  = it.next();
        let op_str = it.next().unwrap_or("==");
        let value  = it.collect::<Vec<_>>().join(" "); // allow spaces in value

        if matches!(field, Some("project")) {
            if let Some(op) = parse_op(op_str) {
                filter_project(&mut records, op, value);
            } else {
                eprintln!("Unknown operator: {}", op_str);
            }
        } else {
            eprintln!("Filtering not yet implemented for {:?}", field);
        }
    }
    // Sort by field specified
    match args.sort.as_str() {
        "" => records.sort_by_key(|a| a.id.clone()),
        "id" => records.sort_by_key(|a| a.id.clone()),
        "description" => records.sort_by_key(|a| a.desc.clone()),
        "completed" => records.sort_by_key(|a| a.completed.clone()),
        "date_added" => records.sort_by_key(|a| a.date_added.clone()),
        "date_completed" => records.sort_by_key(|a| a.date_completed.clone()),
        "project" => records.sort_by_key(|a| a.project.clone()),
        _ => eprintln!("Field {} does not exist", args.sort)

    };

    for (_i, record) in records.iter().enumerate() {
        if record.completed && !args.verbose {
            continue;
        }
        let desc_width = 28;

        // wrap the description text into chunks of `desc_width``
        let wrapped_desc = wrap(&record.desc, desc_width);

        for (k, desc_line) in wrapped_desc.iter().enumerate() {
            let mut line = String::new();

            if k == 0 {
                // First line: print all fields
                write!(&mut line, "| ").unwrap();
                write!(&mut line, "{:<3}", record.id).unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<15}", record.project).unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<desc_width$}", desc_line, desc_width = desc_width).unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<11}", record.completed).unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<12}", record.date_added.format("%Y-%m-%d")).unwrap();
                write!(&mut line, " | ").unwrap();                        
                write!(&mut line, "{:<15}", 
                    record.date_completed
                    .map(|dt| dt.format("%Y-%m-%d").to_string()).unwrap_or_default()).unwrap();
                write!(&mut line, " |").unwrap();
            } else {
                // Continuation line: leave ID and completed blank, only show wrapped desc
                write!(&mut line, "| ").unwrap();
                write!(&mut line, "{:<3}", "").unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<15}", "").unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<desc_width$}", desc_line, desc_width = desc_width).unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<11}", "").unwrap();
                write!(&mut line, " | ").unwrap();
                write!(&mut line, "{:<12}", "").unwrap();
                write!(&mut line, " | ").unwrap(); 
                write!(&mut line, "{:<15}", "").unwrap();
                write!(&mut line, " |").unwrap();
            }

            println!("{line}");
        }

        println!("-------------------------------------------------------------------------------------------------------");
    }
}

// Append mode: used when adding a single new record.
fn open_csv_writer_append(path: &str) -> Writer<File> {
    let needs_headers = match std::fs::metadata(path) {
        Ok(md) => md.len() == 0,
        Err(_) => true,
    };

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open append");

    WriterBuilder::new()
        .has_headers(needs_headers)
        .from_writer(file)
}

// Truncate mode: used when rewriting the entire file (e.g., after marking complete).
fn open_csv_writer_truncate(path: &str) -> Writer<File> {
    let file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .expect("open truncate");

    WriterBuilder::new()
        .has_headers(true)   // write headers on full rewrite
        .from_writer(file)
}


fn open_csv_reader(path: &str) -> csv::Reader<File> {
    let file = File::open(path).unwrap();
    csv::ReaderBuilder::new()
        .has_headers(true)     // ← important for deserialize::<Record>()
        .trim(Trim::All)
        .flexible(true)        // tolerate ragged rows if any
        .from_reader(file)
}


