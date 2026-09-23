use std::fs;
use std::path::Path;

use crate::address::Address;

pub struct Settings {
    pub ismain: bool,
    pub id: u64,
    pub address: Address,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            ismain: false,
            id: rand::random(),
            address: Address::here(8735),
        }
    }

    pub fn from_arguments(arguments: impl Iterator<Item = String>) -> Self {
        let mut new = Settings::new();

        let mut i = 0;
        let arguments: Vec<String> = arguments.collect();
        while i < arguments.len() {
            let argument = &arguments[i];
            i += 1;

            if argument == "--main" {
                new.ismain = true;
                continue;
            }
            else if !argument.contains('=') {
                continue;
            }

            let split = argument.split_once('=');
            let (name, value) = split.unwrap();

            if name == "--port" {
                let read = value.parse::<u16>();
                if read.is_err() {
                    eprintln!("unreadable port: {}", value);
                    continue;
                }
                new.address.port = read.unwrap();
                continue;
            }
        }

        if Path::new("data/id.txt").is_file() {
            let read = fs::read_to_string("data/id.txt");
            if read.is_ok() {
                let id = read.unwrap().trim().parse::<u64>();
                if id.is_ok() {
                    new.id = id.unwrap();
                }
                else {
                    eprintln!("Failed to parse id file");
                }
            }
            else {
                eprintln!("Failed to read id file");
            }
        }
        else {
            let _ = fs::create_dir_all("data");
            let _ = fs::write("data/id.txt", new.id.to_string());
        }

        return new;
    }
}