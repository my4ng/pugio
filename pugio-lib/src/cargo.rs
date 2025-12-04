use std::collections::{BTreeMap, HashMap};

use petgraph::{graph::NodeIndex, stable_graph::StableGraph};
use serde_json::Value;

use crate::graph::{EdgeWeight, Graph, NodeWeight};

pub(crate) fn get_size_map(cargo_bloat_output: &str) -> HashMap<String, usize> {
    let json: Value = serde_json::from_str(cargo_bloat_output).unwrap();
    let pairs: &Vec<Value> = json["crates"].as_array().unwrap();
    let map: HashMap<_, _> = pairs
        .iter()
        .map(|v| {
            let name = v["name"].as_str().unwrap().to_string();
            let size = v["size"].as_u64().unwrap() as usize;
            (name, size)
        })
        .collect();
    map
}

pub(crate) fn get_dep_graph(cargo_tree_output: &str) -> Graph {
    fn add_edge(
        stack: &Vec<(NodeIndex, Option<&str>)>,
        graph: &mut StableGraph<NodeWeight, EdgeWeight>,
        node_index: NodeIndex,
        feat: Option<&str>,
    ) {
        if let Some((back_index, back_feat)) = stack.last().copied()
            && back_index != node_index
        {
            let edge_index = graph.find_edge(back_index, node_index).unwrap_or_else(|| {
                graph.add_edge(
                    back_index,
                    node_index,
                    EdgeWeight {
                        features: BTreeMap::new(),
                    },
                )
            });

            // A feature "i"
            // |- A
            // |- B feature "j"
            //    |- B
            // Insert [i(j)] to edge A -> B, i.e. feature "i" of A enables feature "j" of B.
            if let Some(back_feat) = back_feat {
                let sub_feats = graph
                    .edge_weight_mut(edge_index)
                    .unwrap()
                    .features
                    .entry(back_feat.to_string())
                    .or_default();
                sub_feats.push(feat.unwrap().to_string());
            }
        }
    }

    let mut graph = Graph::default();
    let mut map: HashMap<&str, NodeIndex> = HashMap::new();

    let mut feat_lib_map: HashMap<(&str, &str), NodeIndex> = HashMap::new();

    let mut stack: Vec<(NodeIndex, Option<&str>)> = Vec::new();
    let mut last: (NodeIndex, Option<&str>) = (NodeIndex::new(0), None);
    let mut is_feat_first = false;

    for line in cargo_tree_output.lines() {
        let graph = &mut graph.inner;

        // "2is-wsl v0.4.0 (*)" / "2is-wsl feature "default""
        let split_at = line.find(char::is_alphabetic).unwrap();
        // ("2", "is-wsl v0.4.0 (*)") / ("2", "is-wsl feature "default"")
        let (depth, rest) = line.split_at(split_at);
        let depth: usize = depth.parse().unwrap();
        // "is-wsl v0.4.0" / "is-wsl feature "default""
        let lib = rest.trim_end_matches(" (*)");

        if depth < stack.len() {
            stack.truncate(depth);
        } else if depth == stack.len() + 1 && !is_feat_first {
            stack.push(last);
        }

        if let Some(feat_index) = lib.find(" feature \"") {
            // "default"
            let feat = &lib[feat_index + 10..lib.len() - 1];
            last.1 = Some(feat);
            if rest.ends_with("(*)") {
                // |- A feature (*)
                let short = &lib[..lib.find(' ').unwrap()];
                let node_index = *feat_lib_map.get(&(short, feat)).unwrap();
                add_edge(&stack, graph, node_index, last.1);
            } else {
                is_feat_first = true;
            }
        } else {
            let node_index = map.get(lib).copied().unwrap_or_else(|| {
                let short_end = lib.find(' ').unwrap();
                let (short, extra) = lib.split_at(short_end);
                let name = short.replace('-', "_") + extra;

                let node_index = graph.add_node(NodeWeight::new(name, short_end, BTreeMap::new()));
                map.insert(lib, node_index);
                node_index
            });

            if is_feat_first {
                let short = &lib[..lib.find(' ').unwrap()];
                feat_lib_map.insert((short, last.1.unwrap()), node_index);

                // A feature "i"
                // |- A
                // Add feature "i" to node A
                graph
                    .node_weight_mut(node_index)
                    .unwrap()
                    .features
                    .insert(last.1.unwrap().to_string(), Vec::new());

                if let Some((back_index, back_feat)) = stack.last().copied() {
                    // A feature "i"
                    // |- A
                    // |- A feature "j"
                    //    |- A
                    // Append feature "j" to sub-features of feature "i", i.e. [i(j), j] afterwards
                    if back_index == node_index
                        && let Some(back_feat) = back_feat
                    {
                        graph
                            .node_weight_mut(back_index)
                            .unwrap()
                            .features
                            .get_mut(back_feat)
                            .unwrap()
                            .push(last.1.unwrap().to_string())
                    }
                }
            } else {
                last.1 = None;
            }

            add_edge(&stack, graph, node_index, last.1);

            last.0 = node_index;
            if is_feat_first {
                stack.push(last);
                last.1 = None;
            }
            is_feat_first = false;
        }
    }

    graph
}
