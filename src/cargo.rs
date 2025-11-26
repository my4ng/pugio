use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    process::{Command, Stdio},
};

use anyhow::{Context, bail};
use petgraph::graph::NodeIndex;
use serde_json::Value;

use crate::{
    config::Config,
    graph::{EdgeWeight, Graph, NodeWeight},
};

#[derive(Debug, Default)]
pub struct CargoOptions {
    pub package: Option<String>,
    pub bin: Option<String>,
    pub features: Option<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub release: bool,
}

impl From<&Config> for CargoOptions {
    fn from(value: &Config) -> Self {
        Self {
            package: value.package.clone(),
            bin: value.bin.clone(),
            features: value.features.clone(),
            all_features: value.all_features,
            no_default_features: value.no_default_features,
            release: value.release,
        }
    }
}

// TODO: Add features support
pub fn cargo_tree_output(options: &CargoOptions) -> anyhow::Result<String> {
    let mut command = Command::new("cargo");
    command
        .stdout(Stdio::piped())
        .arg("tree")
        .arg("--edges=no-build,no-proc-macro,no-dev,features")
        .arg("--prefix=depth")
        .arg("--color=never");

    if let Some(package) = &options.package {
        command.arg(format!("--package={package}"));
    }

    if let Some(features) = &options.features {
        command.arg(format!("--features={features}"));
    }

    if options.all_features {
        command.arg("--all-features");
    }

    if options.no_default_features {
        command.arg("--no-default-features");
    }

    command
        .spawn()
        .context("failed to execute cargo-tree")?
        .wait_with_output()
        .map(|o| String::from_utf8(o.stdout).unwrap())
        .context("failed to wait on cargo-tree")
}

pub fn cargo_bloat_output(options: &CargoOptions) -> anyhow::Result<String> {
    let mut command = Command::new("cargo");
    command
        .stdout(Stdio::piped())
        .arg("bloat")
        .arg("-n0")
        .arg("--message-format=json")
        .arg("--crates");

    if let Some(package) = &options.package {
        command.arg(format!("--package={package}"));
    }

    if let Some(binary) = &options.bin {
        command.arg(format!("--bin={binary}"));
    }

    if let Some(features) = &options.features {
        command.arg(format!("--features={features}"));
    }

    if options.all_features {
        command.arg("--all-features");
    }

    if options.no_default_features {
        command.arg("--no-default-features");
    }

    if options.release {
        command.arg("--release");
    }

    command
        .spawn()
        .context("failed to execute cargo-bloat")?
        .wait_with_output()
        .map(|o| String::from_utf8(o.stdout).unwrap())
        .context("failed to wait on cargo-bloat")
}

pub fn get_size_map(json: &str) -> anyhow::Result<HashMap<String, usize>> {
    let json: Value = serde_json::from_str(json)?;
    let pairs: &Vec<Value> = json["crates"].as_array().unwrap();
    let map: HashMap<_, _> = pairs
        .iter()
        .map(|v| {
            let name = v["name"].as_str().unwrap().to_string();
            let size = v["size"].as_u64().unwrap() as usize;
            (name, size)
        })
        .collect();
    Ok(map)
}

pub fn get_dep_graph(output: &str) -> anyhow::Result<Graph> {
    fn add_edge(
        stack: &VecDeque<(NodeIndex, Option<&str>)>,
        graph: &mut Graph,
        node_index: NodeIndex,
    ) {
        if let Some((back_index, back_feat)) = stack.back().copied()
            && back_index != node_index
        {
            let edge_index = graph.find_edge(back_index, node_index).unwrap_or_else(|| {
                graph.add_edge(
                    back_index,
                    node_index,
                    EdgeWeight {
                        features: BTreeSet::new(),
                    },
                )
            });

            if let Some(back_feat) = back_feat {
                graph
                    .edge_weight_mut(edge_index)
                    .unwrap()
                    .features
                    .insert(back_feat.to_string());
            }
        }
    }

    let mut graph = Graph::new();
    let mut map: HashMap<&str, NodeIndex> = HashMap::new();

    let mut feat_lib_map: HashMap<(&str, &str), NodeIndex> = HashMap::new();

    let mut stack: VecDeque<(NodeIndex, Option<&str>)> = VecDeque::new();
    let mut last: (NodeIndex, Option<&str>) = (NodeIndex::new(0), None);
    let mut is_feat_first = false;

    for line in output.lines() {
        if line.is_empty() {
            bail!("one and only one package must be specified");
        }

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
            stack.push_back(last);
        }

        if let Some(feat_idx) = lib.find(" feature \"") {
            // "default"
            let feat = &lib[feat_idx + 10..lib.len() - 1];
            last.1 = Some(feat);
            if rest.ends_with("(*)") {
                // |- A feature (*)
                let short = &lib[..lib.find(' ').unwrap()];
                let node_index = *feat_lib_map.get(&(short, feat)).unwrap();
                add_edge(&stack, &mut graph, node_index);
            } else {
                is_feat_first = true;
            }
        } else {
            let node_index = map.get(lib).copied().unwrap_or_else(|| {
                let short_end = lib.find(' ').unwrap();
                let (short, extra) = lib.split_at(short_end);
                let name = short.replace('-', "_") + extra;

                let node_index = graph.add_node(NodeWeight {
                    name,
                    short_end,
                    features: BTreeMap::new(),
                });
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

                if let Some((back_index, back_feat)) = stack.back().copied() {
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

            add_edge(&stack, &mut graph, node_index);

            last.0 = node_index;
            if is_feat_first {
                stack.push_back(last);
                last.1 = None;
            }
            is_feat_first = false;
        }
    }

    Ok(graph)
}
