use super::*;

#[test]
fn test_key_design() {
    let test_str = "teststr";
    let ret = HierarchyGraph::key_design(test_str);
    assert_eq!(ret, "D:teststr");
}

#[test]
fn test_key_module() {
    let test_str = "teststr";
    let ret = HierarchyGraph::key_module(test_str);
    assert_eq!(ret, "M:teststr");
}

#[test]
fn test_add_design() {
    let mut graph = HierarchyGraph::new();

    let idx1 = graph.add_design("top");

    let key = HierarchyGraph::key_design("top");
    assert!(graph.lookup.contains_key(&key));

    let node = &graph.graph[idx1];
    match node {
        NodeKind::Design { name } => assert_eq!(name, "top"),
        _ => panic!("Expected NodeKind::Design, got {:?}", node),
    }

    let idx2 = graph.add_design("top");
    assert_eq!(idx1, idx2, "Repeated add_design() must return same index");
}

#[test]
fn test_add_module() {
    let mut graph = HierarchyGraph::new();
    let clk_reg = "CLOCKREGION_X0Y0";

    let idx1 = graph.add_module("top", Some(clk_reg));

    let key = HierarchyGraph::key_module("top");
    assert!(graph.lookup.contains_key(&key));

    let node = &graph.graph[idx1];
    match node {
        NodeKind::Module { name, region } => {
            assert_eq!(name, "top");
            assert_eq!(region.as_deref(), Some(clk_reg));
        }
        _ => panic!("Expected NodeKind::Module, got {:?}", node),
    }

    // check with same region
    let idx2 = graph.add_module("top", Some(clk_reg));
    assert_eq!(idx1, idx2, "Repeated add_design() must return same index");

    let clk_reg2 = "CLOCKREGION_X0Y1";

    // check with a different region
    let idx3 = graph.add_module("top", Some(clk_reg2));
    assert_eq!(idx1, idx3, "Repeated add_design() must return same index");

    // but check if region changed;
    // once a node is added we don't change fields
    let node = &graph.graph[idx1];
    match node {
        NodeKind::Module { name, region } => {
            assert_eq!(name, "top");
            assert_ne!(region.as_deref(), Some(clk_reg2));
        }
        _ => panic!("Expected NodeKind::Module, got {:?}", node),
    }
}
