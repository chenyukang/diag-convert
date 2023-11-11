use crate::entry::Entry;
use crate::utils::{self, append_to_string};

#[derive(Default)]
pub struct Parser {
    pub entries: Vec<Entry>,
    pub childs: Vec<(String, String)>,
    pub cur_key: String,
    pub cur_val: String,
    pub parent_key: String,
    pub parent_val: String,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            ..Default::default()
        }
    }

    fn print(&self) {
        for entry in self.entries.iter() {
            entry.print();
        }
    }

    fn add_child(&mut self) {
        if !self.cur_key.is_empty() {
            //eprintln!("now add child: {} => {}", self.cur_key, self.cur_val);
            self.childs
                .push((self.cur_key.to_string(), self.cur_val.to_string()));
        }
    }

    pub fn add_entry(&mut self) {
        if self.parent_key.is_empty() {
            return;
        }
        self.add_child();
        let mut entry = Entry::new(self.parent_key.to_string(), self.parent_val.to_string());
        for (k, v) in self.childs.iter() {
            entry.add_child(k.to_string(), v.to_string());
        }
        eprintln!("now add entry: {:#?}", entry);
        if self.entries.iter().any(|e| e.slug == entry.slug) {
            panic!("error duplicated: {:#?}", entry);
        }
        self.entries.push(entry);
        self.parent_key.clear();
        self.parent_val.clear();
        self.cur_key.clear();
        self.cur_val.clear();
        self.childs.clear();
    }

    pub fn parse_lines(&mut self, lines: Vec<String>) {
        for (index, line) in lines.iter().enumerate() {
            let strip = line.trim();
            //eprintln!("now strip: {}", strip);
            if let Some((k, v)) = utils::check_kv(strip) {
                if k.starts_with(".") {
                    // child entry
                    self.add_child();
                    self.cur_key = k.to_string();
                    self.cur_val = v.to_string();
                } else {
                    // new entry
                    self.add_entry();
                    self.parent_key = k.to_string();
                    self.parent_val = v.to_string();
                }
            } else if !strip.is_empty() {
                // continue line
                if strip.ends_with("=") {
                    let key = strip.trim_end_matches("=").trim();
                    if self.parent_key.is_empty() {
                        self.parent_key = key.to_string();
                    } else {
                        self.cur_key = key.to_string();
                    }
                } else {
                    if self.cur_key.is_empty() {
                        self.parent_val = append_to_string(&self.parent_val, &strip);
                    } else {
                        self.cur_val = append_to_string(&self.cur_val, &strip);
                    }
                }
            } else {
                // new entry
                self.add_entry();
            }
        }
        self.add_entry();
    }
}
