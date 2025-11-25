use std::collections::{HashMap, VecDeque};

#[cfg(feature = "regex")]
use anyhow::Context;
use anyhow::bail;
use petgraph::{
    graph::NodeIndex,
    prelude::StableGraph,
    visit::{Dfs, Topo, Walker},
};

pub struct NodeWeight {
    pub name: String,
    pub short_end: usize,
}

impl std::fmt::Debug for NodeWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}

impl NodeWeight {
    pub fn short(&self) -> &str {
        &self.name[..self.short_end]
    }

    pub fn extra(&self) -> &str {
        &self.name[self.short_end + 1..]
    }

    pub fn full(&self) -> &str {
        &self.name
    }
}

pub fn normalize_sizes(graph: &StableGraph<NodeWeight, ()>, map: &mut HashMap<String, usize>) {
    let mut counts = HashMap::with_capacity(graph.node_count());
    for node in graph.node_weights() {
        *counts.entry(node.short()).or_default() += 1;
    }

    for (name, size) in map.iter_mut() {
        let count = counts.get(name.as_str()).copied().unwrap_or(1);
        *size /= count;
    }
}

pub fn cum_sums(
    graph: &StableGraph<NodeWeight, ()>,
    map: &HashMap<String, usize>,
) -> (Vec<usize>, f32) {
    let mut cum_sums = vec![0; graph.capacity().0];

    for (idx, size) in graph.node_indices().filter_map(|i| {
        let short_name = graph.node_weight(i).unwrap().short();
        map.get(short_name).copied().map(|s| (i.index(), s))
    }) {
        cum_sums[idx] = size;
    }

    let nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();

    for node in nodes.iter().rev() {
        let sources: Vec<_> = graph
            .neighbors_directed(*node, petgraph::Direction::Incoming)
            .collect();
        for source in &sources {
            cum_sums[source.index()] += cum_sums[node.index()] / sources.len();
        }
    }

    (cum_sums, 0.25)
}

pub fn dep_counts(graph: &StableGraph<NodeWeight, ()>) -> (Vec<usize>, f32) {
    let mut dep_counts = vec![0; graph.capacity().0];

    let nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();

    for node in nodes.iter().rev() {
        for target in graph.neighbors(*node) {
            dep_counts[node.index()] += dep_counts[target.index()] + 1;
        }
    }

    (dep_counts, 0.25)
}

pub fn rev_dep_counts(graph: &StableGraph<NodeWeight, ()>) -> (Vec<usize>, f32) {
    let mut rev_dep_counts = vec![0; graph.capacity().0];

    for node in Topo::new(&graph).iter(&graph) {
        for target in graph.neighbors(node) {
            rev_dep_counts[target.index()] += 1;
        }
    }

    (rev_dep_counts, 0.5)
}

pub fn node_classes(graph: &StableGraph<NodeWeight, ()>, is_dir_down: bool) -> Vec<Vec<usize>> {
    let mut classes = vec![Vec::new(); graph.capacity().0];
    let nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();

    if is_dir_down {
        for node in nodes.iter() {
            classes[node.index()].push(node.index());
            for target in graph.neighbors(*node) {
                // SAFETY: graph is known to be DAG, hence no reflexive edge
                let [source, target] = classes
                    .get_disjoint_mut([node.index(), target.index()])
                    .unwrap();
                target.extend_from_slice(source);
            }
        }
    } else {
        for node in nodes.iter().rev() {
            classes[node.index()].push(node.index());
            for target in graph.neighbors(*node) {
                // SAFETY: graph is known to be DAG, hence no reflexive edge
                let [source, target] = classes
                    .get_disjoint_mut([node.index(), target.index()])
                    .unwrap();
                source.extend_from_slice(target);
            }
        }
    }

    classes
}

pub fn remove_small_deps(
    graph: &mut StableGraph<NodeWeight, ()>,
    cum_sums: &[usize],
    threshold: usize,
    std_idx: Option<NodeIndex>,
) {
    for (idx, sum) in cum_sums.iter().enumerate() {
        if *sum < threshold && Some(NodeIndex::new(idx)) != std_idx {
            graph.remove_node(NodeIndex::new(idx));
        }
    }
}

pub fn remove_deep_deps(
    graph: &mut StableGraph<NodeWeight, ()>,
    root_idx: NodeIndex,
    max_depth: usize,
    std_idx: Option<NodeIndex>,
) {
    // TODO: use petgraph#868 once merged
    let mut queue = VecDeque::from([(root_idx, 0)]);
    let mut has_visited = vec![false; graph.capacity().0];
    has_visited[root_idx.index()] = true;

    while let Some((node, depth)) = queue.pop_front()
        && depth < max_depth
    {
        for target in graph.neighbors(node) {
            if !has_visited[target.index()] {
                queue.push_back((target, depth + 1));
                has_visited[target.index()] = true;
            }
        }
    }

    remove_not_visited(graph, &has_visited, std_idx);
}

fn get_matched_node_indices(
    graph: &StableGraph<NodeWeight, ()>,
    pattern: &str,
) -> anyhow::Result<Vec<NodeIndex>> {
    #[cfg(feature = "regex")]
    let regex = regex_lite::Regex::new(pattern)
        .with_context(|| format!("failed to parse pattern as regex: \"{pattern}\""))?;

    let filter = |i: &NodeIndex| -> bool {
        let name = &graph.node_weight(*i).unwrap().name;
        cfg_if::cfg_if! {
            if #[cfg(feature = "regex")] {
                regex.is_match(name)
            } else {
                name.starts_with(pattern)
            }
        }
    };

    Ok(graph.node_indices().filter(filter).collect::<Vec<_>>())
}

fn remove_not_visited(
    graph: &mut StableGraph<NodeWeight, ()>,
    has_visited: &[bool],
    std_idx: Option<NodeIndex>,
) {
    for idx in has_visited.iter().enumerate().filter_map(|(i, b)| {
        if !b && Some(NodeIndex::new(i)) != std_idx {
            Some(i)
        } else {
            None
        }
    }) {
        graph.remove_node(NodeIndex::new(idx));
    }
}

pub fn remove_excluded_deps(
    graph: &mut StableGraph<NodeWeight, ()>,
    patterns: &[String],
    root_idx: NodeIndex,
    std_idx: Option<NodeIndex>,
) -> anyhow::Result<()> {
    let excludes = patterns
        .iter()
        .map(|p| get_matched_node_indices(graph, p))
        .collect::<anyhow::Result<Vec<_>>>()?;

    for exclude in excludes.iter().flatten() {
        graph.remove_node(*exclude);
    }

    let mut has_visited = vec![false; graph.capacity().0];
    for node_idx in Dfs::new(&*graph, root_idx).iter(&*graph) {
        has_visited[node_idx.index()] = true;
    }

    remove_not_visited(graph, &has_visited, std_idx);
    Ok(())
}

pub fn change_root(
    graph: &mut StableGraph<NodeWeight, ()>,
    pattern: &str,
) -> anyhow::Result<NodeIndex> {
    let new_roots = get_matched_node_indices(graph, pattern)?;

    if new_roots.is_empty() {
        bail!("dependency name pattern not found: \"{pattern}\"");
    } else if new_roots.len() > 1 {
        bail!(
            "dependency name pattern not unique: \"{pattern}\", possible full names: {}",
            new_roots
                .iter()
                .map(|n| format!(r#""{}""#, graph.node_weight(*n).unwrap().name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        let new_root = new_roots[0];
        let mut has_visited = vec![false; graph.capacity().0];
        for node_idx in Dfs::new(&*graph, new_root).iter(&*graph) {
            has_visited[node_idx.index()] = true;
        }

        remove_not_visited(graph, &has_visited, None);
        Ok(new_root)
    }
}
