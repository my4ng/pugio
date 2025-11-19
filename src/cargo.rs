use std::{
    collections::{HashMap, VecDeque},
    process::{Command, Stdio},
};

use anyhow::Context;
use petgraph::{graph::NodeIndex, prelude::StableGraph};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct CargoOptions {
    pub package: Option<String>,
    pub binary: Option<String>,
    pub features: Option<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub release: bool,
}

// TODO: Add features support
pub fn cargo_tree_output(options: &CargoOptions) -> anyhow::Result<String> {
    let mut command = Command::new("cargo");
    command
        .stdout(Stdio::piped())
        .arg("tree")
        .arg("--edges=no-build,no-proc-macro,no-dev")
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

    if let Some(binary) = &options.binary {
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

pub fn get_dep_graph(output: &str, has_std: bool) -> StableGraph<String, ()> {
    let mut graph = StableGraph::new();
    let mut map: HashMap<&str, NodeIndex> = HashMap::new();

    let mut stack = VecDeque::new();
    let mut last = NodeIndex::new(0);

    for line in output.lines() {
        // "2is-wsl v0.4.0 (*)"
        let split_at = line.find(char::is_alphabetic).unwrap();
        // ("2", "is-wsl v0.4.0 (*)")
        let (depth, lib) = line.split_at(split_at);
        let depth = depth.parse().unwrap();
        // "is-wsl v0.4.0"
        let lib = lib.trim_end_matches(" (*)");

        let node_index = map.get(lib).copied().unwrap_or_else(|| {
            let node_index =
                // "is_wsl"
                graph.add_node(lib.split_whitespace().next().unwrap().replace('-', "_"));
            map.insert(lib, node_index);
            node_index
        });

        if depth == stack.len() + 1 {
            stack.push_back(last);
        } else if depth < stack.len() {
            stack.truncate(depth);
        }

        if !stack.is_empty() {
            graph.add_edge(*stack.back().unwrap(), node_index, ());
        }
        last = node_index;
    }

    if has_std {
        graph.add_node("std".to_owned());
    }

    graph
}
