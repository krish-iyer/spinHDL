use crate::core::{BuildTasks, ModuleType};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub struct DesignCfg {
    pub name: String,
    pub top: String,
    pub rtl_dir: String,
    #[serde(deserialize_with = "parse_files_list")]
    pub rtl: Vec<String>,
    pub xdc_dir: String,
    #[serde(deserialize_with = "parse_files_list")]
    pub xdc: Vec<String>,
    pub xci_dir: String,
    #[serde(deserialize_with = "parse_files_list")]
    pub xci: Vec<String>,
    pub ip_dir: String,
    #[serde(deserialize_with = "parse_files_list")]
    pub ip: Vec<String>,
    pub build: BuildTasks,
    pub moduletype: ModuleType,
    #[serde(skip)]
    pub rtl_files: Vec<String>,
    #[serde(skip)]
    pub xdc_files: Vec<String>,
    #[serde(skip)]
    pub xci_files: Vec<String>,
    #[serde(skip)]
    pub ip_files: Vec<String>,
    #[serde(skip)]
    pub build_path: String,
}

impl DesignCfg {
    pub fn populate_files(&mut self) {
        self.rtl_files = populate_files_list(&self.rtl_dir, &self.rtl);
        self.xdc_files = populate_files_list(&self.xdc_dir, &self.xdc);
        self.xci_files = populate_files_list(&self.xci_dir, &self.xci);
        self.ip_files = populate_files_list(&self.ip_dir, &self.ip);
    }

    pub fn verify_files_exist(&mut self) {
        self.populate_files();

        for file in &self.rtl_files {
            if !std::path::Path::new(file).exists() {
                println!("Missing RTL file: {}", file);
                panic!("Files missing");
            }
        }

        if self.xdc_files.len() > 0 {
            for file in &self.xdc_files {
                if !std::path::Path::new(file).exists() {
                    println!("Missing XDC file: {}", file);
                    panic!("Files missing");
                }
            }
        }

        if self.xci_files.len() > 0 {
            for file in &self.xci_files {
                if !std::path::Path::new(file).exists() {
                    println!("Missing XCI file: {}", file);
                    panic!("Files missing");
                }
            }
        }

        if self.ip_files.len() > 0 {
            for file in &self.ip_files {
                if !std::path::Path::new(file).exists() {
                    println!("Missing IP file: {}", file);
                    panic!("Files missing");
                }
            }
        }
        println!("All RTL files exist for '{}'", self.name);
    }
}

fn parse_files_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect())
}

fn populate_files_list(dir: &str, files: &Vec<String>) -> Vec<String> {
    files
        .iter()
        .map(|f| format!("{}/{}", dir.trim_end_matches('/'), f))
        .collect()
}
