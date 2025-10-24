use crate::design_hier;

use super::init::*;
use serde::{Deserialize};
use std::{env, fs, fs::File, path::Path, process::Command};
use std::{io, io::Error, io::ErrorKind};

pub mod create_tcl;


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

    pub fn run_tcl(&self, tcl: &str) -> io::Result<()> {
        // check if the tcl exists
        if !Path::new(tcl).exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("TCL file not found: {}", tcl),
            ));
        }

        let status = Command::new("vivado")
            .args(["-nojournal", "-nolog", "-mode", "batch", "-source", tcl])
            .status()?;

        if !status.success() {
            return Err(Error::new(ErrorKind::Other, "Vivado TCL execution failed"));
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

            if let Err(e) = self.create_synth_tcl(design) {
                panic!("Failed to create run synth tcl for {} : {}", design.name, e);
            }

            // synth
            if design.build == Build::Synth {
                if let Err(e) = self.run_tcl("run_synth.tcl") {
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

    pub fn gen_bitstreams(&self, root_design: &String) -> io::Result<()> {
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
                    if let Err(e) = self.run_tcl(&tcl_path) {
                        panic! {"Failed to run bitstreams generation {}", e};
                    };
                }
                _ => panic!("Failed to run bitstreams tcl"),
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

            // create a empty file to exec tcl file :
            // TODO: fix this. may be force creation of the file.
            let xdc_path = "pr_main.xdc";
            if let Err(e) = File::create(xdc_path) {
                panic! {"Failed to create pr_main.xdc file {}", e};
            }

            // execute pr_xdc
            if let Err(e) = self.run_tcl("create_pr_xdc.tcl") {
                panic! {"Failed to run create PR XDC {}", e};
            };

            // create route tcl
            if let Err(e) = self.create_route_tcl(&root_design) {
                panic! {"Failed to create PR XDC {}", e};
            };

            // route
            if let Err(e) = self.run_tcl("route_pr.tcl") {
                panic! {"Failed to create Route {}", e};
            };

            // bitgen
            if let Err(e) = self.create_bitstream_tcl(&root_design) {
                panic! {"Failed to create Route {}", e};
            };

            if let Err(e) = self.gen_bitstreams(&root_design) {
                panic! {"Failed to create Route {}", e};
            };

            env::set_current_dir(cur_dir).expect("Failed to change directory to build");
        }
    }
}
