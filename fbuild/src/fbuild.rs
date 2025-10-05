use serde::{Deserialize, Deserializer};
use std::{env, fs, fs::File, path::Path, process::Command};
use std::{io, io::Error, io::ErrorKind, io::Write};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Build {
    Synth,
    Route,
    Bitgen,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModuleType {
    Static,
    Recon,
}

#[derive(Debug, Deserialize)]
pub struct ProjectCfg {
    pub name: String,
    pub version: String,
    pub part: String,
    pub arch: String,
    pub part_xdc: String,
    pub build_dir: String,
}

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
    pub build: Build,
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

#[derive(Debug, Deserialize)]
pub struct BuildCfg {
    #[serde(rename = "project")]
    pub projectcfg: ProjectCfg,
    #[serde(rename = "design")]
    pub designcfg: Vec<DesignCfg>,
}

impl DesignCfg {
    pub fn populate_files(&mut self) {
        self.rtl_files = populate_files_list(&self.rtl_dir, &self.rtl);
        self.xdc_files = populate_files_list(&self.xdc_dir, &self.xdc);
        self.xci_files = populate_files_list(&self.xci_dir, &self.xci);
        self.ip_files = populate_files_list(&self.ip_dir, &self.ip);
    }

    pub fn verify_files_exist(&self) {
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

impl ProjectCfg {
    pub fn verify_project_setup(&self) {
        if !Path::new(&self.part_xdc).exists() {
            println!("Missing part_xdc file: {}", self.part_xdc);
            panic!("Required part_xdc file not found");
        }

        if !Path::new(&self.build_dir).exists() {
            println!("Creating build directory: {}", self.build_dir);
            fs::create_dir_all(&self.build_dir).expect("Failed to create build directory");
        } else {
            println!("Build directory already exists: {}", self.build_dir);
        }
    }
}

impl BuildCfg {
    pub fn verify_build_setup(&mut self) {
        self.projectcfg.verify_project_setup();
        for design in &mut self.designcfg {
            design.build_path = format!("{}/{}", self.projectcfg.build_dir, design.name);

            if !Path::new(&design.build_path).exists() {
                println!("Creating design build directory: {}", design.build_path);
                fs::create_dir_all(&design.build_path)
                    .expect("Failed to create design build directory");
            }

            println!("Changing directory to build: {}", self.projectcfg.build_dir);

            let build_dir = &design.build_path;
            let cur_dir = env::current_dir().expect("Failed to get current directory");

            env::set_current_dir(build_dir).expect("Failed to change directory to build");

            design.populate_files();
            design.verify_files_exist();

            env::set_current_dir(cur_dir).expect("Failed to change directory to build");
        }
    }

    pub fn create_project_tcl(&self, design: &DesignCfg) -> io::Result<()> {
        let tcl_path = "create_project.tcl";
        let mut tcl_file = File::create(&tcl_path)?;

        writeln!(
            tcl_file,
            "create_project -force -part {} {}",
            self.projectcfg.part, design.name
        )?;

        if !design.rtl_files.is_empty() {
            writeln!(
                tcl_file,
                "add_files -fileset sources_1 {}",
                design.rtl_files.join(" ")
            )?;
        }

        writeln!(
            tcl_file,
            "set_property top {} [current_fileset]",
            design.top
        )?;

        if !design.xdc_files.is_empty() {
            writeln!(
                tcl_file,
                "add_files -fileset constrs_1 {}",
                design.xdc_files.join(" ")
            )?;
        }

        for file in &design.xci_files {
            writeln!(tcl_file, "import_ip {}", file)?;
        }

        for file in &design.ip_files {
            writeln!(tcl_file, "source {}", file)?;
        }

        println!("Created create_project.tcl for '{}'", design.name);
        Ok(())
    }

    pub fn gen_project(&self) -> io::Result<()> {
        let status = Command::new("vivado")
            .args([
                "-nojournal",
                "-nolog",
                "-mode",
                "batch",
                "-source",
                "create_project.tcl",
            ])
            .status()?;

        if !status.success() {
            return Err(Error::new(ErrorKind::Other, format!("Gen Project failed")));
        }

        Ok(())
    }

    pub fn create_run_synth_tcl(&self, design: &DesignCfg) -> io::Result<()> {
        let synth_tcl_path = "run_synth.tcl";
        let mut synth_tcl = File::create(&synth_tcl_path).expect("Failed to create run_synth.tcl");

        writeln!(synth_tcl, "open_project {}.xpr", design.name)?;

        match design.moduletype {
            ModuleType::Recon => {
                writeln!(synth_tcl, "synth_design -mode out_of_context")?;
                writeln!(
                    synth_tcl,
                    "write_checkpoint -force {}/runs/synth_1/{}.dcp",
                    design.name, design.name
                )?;
                writeln!(synth_tcl, "close_project")?;
            }

            ModuleType::Static => {
                writeln!(synth_tcl, "reset_run synth_1")?;
                writeln!(synth_tcl, "launch_runs -jobs 4 synth_1")?;
                writeln!(synth_tcl, "wait_on_run synth_1")?;
            }
        }

        Ok(())
    }

    pub fn run_synth(&self) -> io::Result<()> {
        let status = Command::new("vivado")
            .args([
                "-nojournal",
                "-nolog",
                "-mode",
                "batch",
                "-source",
                "run_synth.tcl",
            ])
            .status()?;

        if !status.success() {
            return Err(Error::new(ErrorKind::Other, format!("Run Synth failed")));
        }

        Ok(())
    }

    pub fn build_designs(&self) {
        for design in &self.designcfg {
            let build_dir = &design.build_path;
            let cur_dir = env::current_dir().expect("Failed to get current directory");

            env::set_current_dir(build_dir).expect("Failed to change directory to build");

            if let Err(e) = self.create_project_tcl(design) {
                panic!("Failed to create project tcl for {} : {}", design.name, e);
            }

            println!("Running Vivado for design '{}'", design.name);

            if let Err(e) = self.gen_project() {
                panic!("Vivado failed for {} : {}", design.name, e);
            }

            if let Err(e) = self.create_run_synth_tcl(design) {
                panic!("Failed to create run synth tcl for {} : {}", design.name, e);
            }

            if design.build == Build::Synth {
                if let Err(e) = self.run_synth() {
                    panic!("Run Synth failed for {} : {}", design.name, e);
                }
            }

            env::set_current_dir(cur_dir).expect("Failed to change directory to build");
            println!("Generated TCL for design '{}'", design.name);
        }
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
