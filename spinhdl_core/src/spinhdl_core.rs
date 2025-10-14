use crate::design_hier;

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
pub struct RootDesign {
    pub design: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BuildCfg {
    #[serde(rename = "project")]
    pub projectcfg: ProjectCfg,
    #[serde(rename = "design")]
    pub designcfg: Vec<DesignCfg>,
    pub root: RootDesign,
    pub hier: Vec<design_hier::DesignEntry>,
    #[serde(skip)]
    pub design_graph: design_hier::HierarchyGraph,
}

pub struct PrXdc {
    project_name: String,
    instance_name: String,
    region: String,
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

    pub fn parse_hierarchy(&mut self) {
        for d in &self.hier {
            self.design_graph.add_design(&d.name);
            for m in &d.modules {
                self.design_graph.add_module(&m.name, m.region.as_deref());
                self.design_graph.connect_design_to_module(&d.name, &m.name);
            }
        }

        for d in &self.hier {
            for m in &d.modules {
                for impl_design in &m.rm {
                    self.design_graph.add_design(impl_design);
                    self.design_graph
                        .connect_module_to_design_impl(&m.name, impl_design);
                }
            }
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

    pub fn synth_designs(&self) {
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

            // synth
            if design.build == Build::Synth {
                if let Err(e) = self.run_synth() {
                    panic!("Run Synth failed for {} : {}", design.name, e);
                }
            }

            // symlink
            // match design.moduletype {
            //     ModuleType::Recon => {
            let src = format!(
                "{}/{}/{}/runs/synth_1/{}.dcp",
                cur_dir.to_string_lossy().to_string(),
                build_dir,
                design.name,
                design.name
            );
            let dst = format!(
                "{}/{}/{}.dcp",
                cur_dir.to_string_lossy().to_string(),
                self.projectcfg.build_dir,
                design.name
            );

            println!("Linking DCP: {} -> {}", src, dst);

            let status = Command::new("ln")
                .args(["-sf", &src, &dst])
                .status()
                .unwrap();

            if !status.success() {
                panic!("Failed to create symlink for {}", design.name);
            }
            //     }
            //     _ => {}
            // }

            env::set_current_dir(cur_dir).expect("Failed to change directory to build");
            println!("Generated TCL for design '{}'", design.name);
        }
    }

    pub fn create_pr_xdc_tcl(&self, constr: &PrXdc) -> io::Result<()> {
        // Ensure root design exists

        let tcl_path = "create_pr_xdc.tcl";

        let mut tcl = File::create(tcl_path)?;

        writeln!(tcl, "open_project {}.xpr", constr.project_name)?;
        writeln!(tcl, "open_run synth_1 -name synth_1")?;
        writeln!(
            tcl,
            "set_property target_constrs_file pr_{}.xdc [current_fileset -constrset]",
            constr.project_name
        )?;

        writeln!(tcl, "startgroup")?;
        writeln!(tcl, "create_pblock pblock_{}", constr.instance_name)?;
        writeln!(
            tcl,
            "resize_pblock pblock_{} -add {}",
            constr.instance_name, constr.region
        )?;
        writeln!(
            tcl,
            "add_cells_to_pblock pblock_{} [get_cells [list {}]] -clear_locs",
            constr.instance_name, constr.instance_name
        )?;
        writeln!(tcl, "endgroup")?;

        writeln!(
            tcl,
            "set_property SNAPPING_MODE ON [get_pblocks pblock_{}]",
            constr.instance_name
        )?;
        writeln!(
            tcl,
            "set_property RESET_AFTER_RECONFIG 1 [get_pblocks pblock_{}]",
            constr.instance_name
        )?;
        writeln!(
            tcl,
            "set_property HD.RECONFIGURABLE 1 [get_cells {}]",
            constr.instance_name
        )?;
        writeln!(tcl, "save_constraints -force")?;
        writeln!(tcl, "close_project")?;

        println!(
            "Generated partial reconfiguration XDC for '{}'",
            constr.project_name
        );
        Ok(())
    }

    pub fn run_create_pr_xdc(&self) -> io::Result<()> {
        let xdc_path = "pr_main.xdc";
        File::create(xdc_path)?;

        let status = Command::new("vivado")
            .args([
                "-nojournal",
                "-nolog",
                "-mode",
                "batch",
                "-source",
                "create_pr_xdc.tcl",
            ])
            .status()?;

        if !status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Vivado failed to create PR XDC"),
            ));
        }
        Ok(())
    }

    pub fn run_route(&self) -> io::Result<()> {
        let status = Command::new("vivado")
            .args([
                "-nojournal",
                "-nolog",
                "-mode",
                "batch",
                "-source",
                "route_pr.tcl",
            ])
            .status()?;

        if !status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Vivado failed to route"),
            ));
        }
        Ok(())
    }

    pub fn create_route_tcl(&self, root_design: &String) -> io::Result<()> {
        let pr_node = self.design_graph.get_child_nodes(&root_design, true);

        // TODO: Fix this; you are being sloppy
        let pr_inst = match &pr_node[0] {
            design_hier::NodeKind::Module { name, region } => PrXdc {
                project_name: root_design.clone(),
                instance_name: name.clone(),
                region: region.clone().unwrap_or_default(),
            },
            _ => panic!("Expected Module but received Design node"),
        };

        let rm_designs = self
            .design_graph
            .get_child_nodes(&pr_inst.instance_name, false);

        let tcl_path = "route_pr.tcl";

        let mut tcl = File::create(tcl_path)?;

        writeln!(tcl, "open_project {}.xpr", root_design)?;
        writeln!(tcl, "open_run synth_1 -name synth_1")?;

        for (i, rm) in rm_designs.iter().enumerate() {
            match rm {
                design_hier::NodeKind::Design { name } => {
                    writeln!(
                        tcl,
                        "read_checkpoint -cell [get_cells {}] ../{}.dcp",
                        pr_inst.instance_name, name
                    )?;

                    writeln!(tcl, "opt_design")?;
                    writeln!(tcl, "place_design")?;
                    writeln!(tcl, "route_design")?;

                    writeln!(tcl, "write_checkpoint -force {}_routed.dcp", name)?;

                    writeln!(
                        tcl,
                        "update_design -cell [get_cells {}] -black_box",
                        pr_inst.instance_name
                    )?;

                    if i == 0 {
                        // only lock in the first iter
                        writeln!(tcl, "lock -level routing")?;
                    }
                }
                _ => panic!("Received a Module when Design was expected!"),
            }
        }
        writeln!(tcl, "close_project")?;

        Ok(())
    }

    pub fn create_bitstream_tcl(&self, root_design: &String) -> io::Result<()> {
        let pr_node = self.design_graph.get_child_nodes(&root_design, true);

        // TODO: Fix this; you are being sloppy
        let pr_inst = match &pr_node[0] {
            design_hier::NodeKind::Module { name, region } => PrXdc {
                project_name: root_design.clone(),
                instance_name: name.clone(),
                region: region.clone().unwrap_or_default(),
            },
            _ => panic!("Expected Module but received Design node"),
        };

        let rm_designs = self
            .design_graph
            .get_child_nodes(&pr_inst.instance_name, false);

        for rm in &rm_designs {
            match rm {
                design_hier::NodeKind::Design { name } => {
                    let tcl_path = format!("generate_bit_{}.tcl", name);

                    let mut tcl = File::create(&tcl_path)?;

                    // Write TCL commands
                    writeln!(tcl, "open_project {}.xpr", root_design)?;
                    writeln!(tcl, "open_checkpoint {}_routed.dcp", name)?;
                    writeln!(tcl, "write_bitstream -force -bin_file {}.bit", name)?;
                    writeln!(tcl, "write_debug_probes -force {}.ltx", name)?;
                    writeln!(tcl, "write_hw_platform -fixed -force {}.xsa", name)?;
                    writeln!(
                        tcl,
                        "write_cfgmem -force -format BIN -interface SMAPx32 \
                         -loadbit \"up 0x0 {}_pblock_{}_partial.bit\" \"{}_part.bin\"",
                        name, pr_inst.instance_name, name
                    )?;
                    writeln!(tcl, "close_design")?;
                    writeln!(tcl, "close_project")?;
                }
                _ => panic!("Failed to create bitstreams tcl"),
            }
        }

        for rm in &rm_designs {
            match rm {
                design_hier::NodeKind::Design { name } => {
                    let tcl_path = format!("generate_bit_{}.tcl", name);
                    let status = Command::new("vivado")
                        .args([
                            "-nojournal",
                            "-nolog",
                            "-mode",
                            "batch",
                            "-source",
                            &tcl_path,
                        ])
                        .status()?;

                    if !status.success() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Vivado failed to create PR XDC"),
                        ));
                    }
                }
                _ => panic!("Failed to create bitstreams tcl"),
            }
        }

        Ok(())
    }

    pub fn build_designs(&mut self) {
        // synth designs
        self.synth_designs();

        if let Some(root_design) = self.root.design.take() {
            // PR flow
            self.design_graph = design_hier::HierarchyGraph::new();

            self.parse_hierarchy();

            let pr_node = self.design_graph.get_child_nodes(&root_design, true);
            println!("{:?}", pr_node);

            let pr_constr = match &pr_node[0] {
                design_hier::NodeKind::Module { name, region } => PrXdc {
                    project_name: root_design.clone(),
                    instance_name: name.clone(),
                    region: region.clone().unwrap_or_default(),
                },
                _ => panic!("Expected Module but received Design node"),
            };

            let build_dir = format!("{}/{}", self.projectcfg.build_dir, root_design);
            let cur_dir = env::current_dir().expect("Failed to get current directory");

            env::set_current_dir(build_dir).expect("Failed to change directory to build");

            // create pr_xdc tcl
            if let Err(e) = self.create_pr_xdc_tcl(&pr_constr) {
                panic! {"Failed to create PR XDC {}", e};
            };

            // execute pr_xdc
            if let Err(e) = self.run_create_pr_xdc() {
                panic! {"Failed to run create PR XDC {}", e};
            };

            // create route tcl
            if let Err(e) = self.create_route_tcl(&root_design) {
                panic! {"Failed to create PR XDC {}", e};
            };

            // route
            if let Err(e) = self.run_route() {
                panic! {"Failed to create Route {}", e};
            };
            // bitgen
            if let Err(e) = self.create_bitstream_tcl(&root_design) {
                panic! {"Failed to create Route {}", e};
            };

            env::set_current_dir(cur_dir).expect("Failed to change directory to build");
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
