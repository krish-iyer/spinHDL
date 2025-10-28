use petgraph::dot::{Config, Dot};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::{Direction, algo};
use std::collections::HashMap;

use crate::core::{BuildCfg, BuildTasks};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildStage {
    VerifyFiles,
    CreateProject,
    Synth,
    Route,
    Bitgen,
}

impl BuildStage {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildStage::VerifyFiles => "verify_files",
            BuildStage::CreateProject => "create_project",
            BuildStage::Synth => "synth",
            BuildStage::Route => "route",
            BuildStage::Bitgen => "bitgen",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "verify_files" => Some(BuildStage::VerifyFiles),
            "create_project" => Some(BuildStage::CreateProject),
            "synth" => Some(BuildStage::Synth),
            "route" => Some(BuildStage::Route),
            "bitgen" => Some(BuildStage::Bitgen),
            _ => None,
        }
    }
}

/// One node == one *stage* of a specific design (e.g., main:route)
#[derive(Debug, Clone)]
pub struct FlowNode {
    /// stable key like "main:synth"
    pub key: String,
    pub design: String,
    pub stage: BuildStage,
    pub artifacts: Vec<String>,
}

/// Edges represent ordering/dependencies between stages
#[derive(Debug, Clone, Copy)]
pub enum FlowEdge {
    Depends, // u -> v means "u must complete before v"
}

/// The executable flow graph
#[derive(Debug, Default)]
pub struct FlowGraph {
    pub graph: Graph<FlowNode, FlowEdge>,
    /// key -> node index (key format "<design>:<stage>")
    pub index: HashMap<String, NodeIndex>,
}

impl FlowGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            index: HashMap::new(),
        }
    }

    fn key(design: &str, stage: BuildStage) -> String {
        format!("{}:{}", design, stage.as_str())
    }

    pub fn ensure_node(&mut self, design: &str, stage: BuildStage) -> NodeIndex {
        let k = Self::key(design, stage);
        if let Some(&idx) = self.index.get(&k) {
            return idx;
        }
        let idx = self.graph.add_node(FlowNode {
            key: k.clone(),
            design: design.to_string(),
            stage,
            artifacts: Vec::new(),
        });
        self.index.insert(k, idx);
        idx
    }

    pub fn depend(&mut self, before: (&str, BuildStage), after: (&str, BuildStage)) {
        let u = self.ensure_node(before.0, before.1);
        let v = self.ensure_node(after.0, after.1);
        self.graph.add_edge(u, v, FlowEdge::Depends);
    }

    pub fn to_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        )
    }

    pub fn write_dot_file(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_dot())
    }

    pub fn print_hierarchy(&self) {
        // group nodes by design
        let mut by_design: HashMap<&str, Vec<&FlowNode>> = HashMap::new();
        for idx in self.graph.node_indices() {
            let n = &self.graph[idx];
            by_design.entry(&n.design).or_default().push(n);
        }

        for (d, nodes) in by_design {
            println!("Design: {d}");
            // show outgoing order for each node (compact)
            for n in nodes {
                let mut outs = Vec::new();
                for e in self
                    .graph
                    .edges_directed(*self.index.get(&n.key).unwrap(), Direction::Outgoing)
                {
                    outs.push(self.graph[e.target()].key.clone());
                }
                if outs.is_empty() {
                    println!("  • {}", n.key);
                } else {
                    println!("  • {} -> {}", n.key, outs.join(", "));
                }
            }
        }
    }

    pub fn add_artifact(&mut self, design: &str, stage: BuildStage, path: &str) {
        let key = Self::key(design, stage);
        if let Some(&idx) = self.index.get(&key) {
            self.graph[idx].artifacts.push(path.to_string());
        }
    }

    pub fn get_artifacts(&self, design: &str, stage: BuildStage) -> Option<&[String]> {
        self.index
            .get(&Self::key(design, stage))
            .map(|&idx| self.graph[idx].artifacts.as_slice())
    }

    pub fn all_artifacts(&self) -> Vec<String> {
        self.graph
            .node_weights()
            .flat_map(|n| n.artifacts.clone())
            .collect()
    }

    pub fn topo_order(&self) -> Vec<String> {
        let order = algo::toposort(&self.graph, None)
            .expect("Cycle in flow graph (unexpected for a build plan)");
        order
            .into_iter()
            .map(|i| self.graph[i].key.clone())
            .collect()
    }
}
