use petgraph::Direction;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ModuleEntry {
    pub name: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub rm: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DesignEntry {
    pub name: String,
    #[serde(default)]
    pub modules: Vec<ModuleEntry>,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Design {
        name: String,
    },
    Module {
        name: String,
        region: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum EdgeKind {
    Instance,
    Implement,
}

#[derive(Debug, Default)]
pub struct HierarchyGraph {
    pub graph: Graph<NodeKind, EdgeKind>,
    pub lookup: HashMap<String, NodeIndex>,
}

impl HierarchyGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            lookup: HashMap::new(),
        }
    }

    fn key_design(name: &str) -> String {
        format!("D:{name}")
    }

    fn key_module(name: &str) -> String {
        format!("M:{name}")
    }

    pub fn add_design(&mut self, name: &str) -> NodeIndex {
        let k = Self::key_design(name);
        if let Some(&idx) = self.lookup.get(&k) {
            return idx;
        }
        let idx = self.graph.add_node(NodeKind::Design {
            name: name.to_string(),
        });
        self.lookup.insert(k, idx);
        idx
    }

    pub fn add_module(&mut self, name: &str, region: Option<&str>) -> NodeIndex {
        let k = Self::key_module(name);
        if let Some(&idx) = self.lookup.get(&k) {
            return idx;
        }
        let idx = self.graph.add_node(NodeKind::Module {
            name: name.to_string(),
            region: region.map(|s| s.to_string()),
        });
        self.lookup.insert(k, idx);
        idx
    }

    pub fn connect_design_to_module(&mut self, design: &str, module: &str) {
        let d = *self
            .lookup
            .get(&Self::key_design(design))
            .expect("design missing");
        let m = *self
            .lookup
            .get(&Self::key_module(module))
            .expect("module missing");
        self.graph.add_edge(d, m, EdgeKind::Instance);
    }

    pub fn connect_module_to_design_impl(&mut self, module: &str, impl_design: &str) {
        let m = *self
            .lookup
            .get(&Self::key_module(module))
            .expect("module missing");
        let d = *self
            .lookup
            .get(&Self::key_design(impl_design))
            .expect("impl design missing");
        self.graph.add_edge(m, d, EdgeKind::Implement);
    }

    pub fn get_child_nodes(&self, name: &str, is_design: bool) -> Vec<NodeKind> {
        let key = if is_design {
            Self::key_design(name)
        } else {
            Self::key_module(name)
        };
        let mut out = Vec::new();

        if let Some(&idx) = self.lookup.get(&key) {
            for e in self.graph.edges_directed(idx, Direction::Outgoing) {
                let node = &self.graph[e.target()];
                out.push(node.clone());
            }
        }
        out
    }

    pub fn get_parent_nodes(&self, name: &str, is_design: bool) -> Vec<NodeKind> {
        let key = if is_design {
            Self::key_design(name)
        } else {
            Self::key_module(name)
        };
        let mut out = Vec::new();

        if let Some(&idx) = self.lookup.get(&key) {
            for e in self.graph.edges_directed(idx, Direction::Incoming) {
                let node = &self.graph[e.source()];
                out.push(node.clone());
            }
        }
        out
    }
}
