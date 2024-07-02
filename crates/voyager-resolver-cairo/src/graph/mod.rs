use crate::model::{CairoModule, ModulePath};

use anyhow::Context;
use anyhow::Result;
use petgraph::dot::{Config, Dot};
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeWeight(());

impl Display for EdgeWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "")
    }
}

/// Creates a directed graph of module dependencies from a list of Cairo modules.
/// For each module in the input list, a node is added to the graph with the module's path as the node label.
/// Edges are then added between nodes based on the imports of each module.
/// An edge is added from the source module to the target module if the target module is imported by the source module.
/// # Arguments
///
/// * `modules` - A vector of references to CairoModule structs representing the modules to create a graph of.
///
/// # Returns
///
/// A directed graph of module dependencies represented by a `Graph` struct.
pub fn create_graph(modules: &Vec<CairoModule>) -> Graph<ModulePath, EdgeWeight> {
    let mut graph = Graph::<ModulePath, EdgeWeight>::new();

    // Create nodes for each file
    let mut file_nodes: HashMap<ModulePath, _> = HashMap::new();
    for module in modules {
        let module_path = module.path.clone();
        let node = graph.add_node(module_path.clone());
        file_nodes.insert(module_path, node);
    }

    // Add edges based on the imports
    for module in modules {
        let module_path = module.path.clone();
        for import in module.imports.iter() {
            let import_path = import.get_import_module();

            for target_module in modules.iter() {
                let target_module = &target_module.path;

                if *target_module == import_path {
                    if let (Some(&src), Some(&dst)) = (
                        file_nodes.get(&module_path.clone()),
                        file_nodes.get(target_module),
                    ) {
                        graph.add_edge(src, dst, EdgeWeight(()));
                    }
                    break;
                }
            }
        }
    }

    graph
}

pub fn _display_graphviz(graph: &Graph<ModulePath, EdgeWeight>) {
    println!("{:#?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
}

/// Extracts the source and destination nodes from each edge in a directed graph,
/// and returns a vector of unique node labels representing the required files for compilation.
/// The function takes a reference to a `Graph` struct representing the directed graph of module dependencies.
/// The function returns a vector of unique `String` node labels representing the required files for compilation. The node labels are sorted and deduplicated.
/// # Arguments
///
/// * `graph` - A reference to a `Graph` struct representing the directed graph of module dependencies.
///
/// # Returns
///
/// A vector of unique `String` node labels representing the required files for compilation.
#[allow(dead_code)]
pub fn get_required_project_modules(
    graph: &Graph<ModulePath, EdgeWeight>,
) -> Result<Vec<ModulePath>> {
    let mut required_files: Vec<ModulePath> = Vec::new();

    for edge in graph.edge_indices() {
        let (src, dst) = graph
            .edge_endpoints(edge)
            .with_context(|| format!("Couldn't get edge endpoints {}", edge.index()))?;
        required_files.push(graph[src].clone());
        required_files.push(graph[dst].clone());
    }

    required_files.sort();
    required_files.dedup();
    Ok(required_files)
}

/// This function retrieves the required dependency modules paths for a set of CairoModules
/// from a dependency graph.
/// The object returned also contains the path to the contract modules themselves.
///
/// # Arguments
///
/// * `graph` - A reference to the dependency graph from which to retrieve the required modules.
/// * `contracts_modules` - A `Vec` of `&CairoModule` references representing the modules containing the contracts.
///
/// # Returns
///
/// * A `Vec` of `String`s representing the required modules if successful.
///
pub fn get_required_module_for_contracts(
    graph: &Graph<ModulePath, EdgeWeight>,
    contracts_modules: &Vec<&CairoModule>,
) -> Result<Vec<ModulePath>> {
    let mut required_modules: Vec<ModulePath> = Vec::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();

    for contract_module in contracts_modules {
        let module_path = contract_module.path.clone();
        required_modules.push(module_path.clone());
        // Find the node for the contract module
        let contract_node = graph
            .node_indices()
            .find(|&index| graph[index] == module_path)
            .with_context(|| format!("Couldn't find corresponding module for {module_path}"))?;

        // Push the neighbors of the contract node to the queue
        for neighbor in graph.neighbors(contract_node) {
            queue.push_back(neighbor);
        }
    }

    // Traverse the graph in a BFS order, adding each node to the required modules list
    while let Some(node) = queue.pop_front() {
        let node_name = &graph[node];

        // Skip nodes that are already in the required modules list
        if required_modules.contains(node_name) {
            continue;
        }

        // Add the node to the required modules list and push its neighbors to the queue
        required_modules.push(node_name.clone());
        for neighbor in graph.neighbors(node) {
            queue.push_back(neighbor);
        }
    }

    required_modules.sort();
    Ok(required_modules)
}

#[cfg(test)]
mod tests {
    use crate::graph::{create_graph, get_required_project_modules, EdgeWeight};
    use crate::model::ModulePath;
    use crate::utils::test_utils::setup_simple_modules;

    #[test]
    fn test_create_graph() {
        let modules = setup_simple_modules();
        let graph = create_graph(&modules);
        let _required_modules = get_required_project_modules(&graph).unwrap();
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 1);

        let node_indices = graph.node_indices().collect::<Vec<_>>();
        assert_eq!(
            graph.edge_weight(
                graph
                    .find_edge(node_indices[3], node_indices[2])
                    .unwrap_or_else(|| panic!("Couldn't find edge"))
            ),
            Some(&EdgeWeight(()))
        );
    }

    #[test]
    fn test_list_edges_and_required_files() {
        let modules = setup_simple_modules();
        let graph = create_graph(&modules);
        let required_modules = get_required_project_modules(&graph).unwrap();
        assert_eq!(required_modules.len(), 2);
        assert!(required_modules.contains(&ModulePath::new("test::contract")));
        assert!(required_modules.contains(&ModulePath::new("test::submod::subsubmod")));
    }
}
