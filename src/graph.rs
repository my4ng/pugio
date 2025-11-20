use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::{
    graph::NodeIndex,
    prelude::StableGraph,
    visit::{Topo, Walker},
};

pub fn cum_sums(
    graph: &StableGraph<String, ()>,
    map: &HashMap<String, usize>,
) -> (Vec<usize>, f32) {
    // TODO: currently the same size is used for all nodes with the same name, change?
    let mut cum_sums: Vec<_> = graph
        .node_weights()
        .map(|n| map.get(n).copied().unwrap_or_default())
        .collect();

    let mut nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();
    nodes.reverse();

    for node in nodes {
        let sources: Vec<_> = graph
            .neighbors_directed(node, petgraph::Direction::Incoming)
            .collect();
        for source in &sources {
            cum_sums[source.index()] += cum_sums[node.index()] / sources.len();
        }
    }

    (cum_sums, 0.25)
}

pub fn dep_counts(graph: &StableGraph<String, ()>) -> (Vec<usize>, f32) {
    let mut dep_counts: Vec<_> = vec![0; graph.node_count()];

    let mut nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();
    nodes.reverse();

    for node in nodes {
        for target in graph.neighbors(node) {
            dep_counts[node.index()] += dep_counts[target.index()] + 1;
        }
    }

    (dep_counts, 0.25)
}

pub fn rev_dep_counts(graph: &StableGraph<String, ()>) -> (Vec<usize>, f32) {
    let mut rev_dep_counts: Vec<_> = vec![0; graph.node_count()];

    for node in Topo::new(&graph).iter(&graph) {
        for target in graph.neighbors(node) {
            rev_dep_counts[target.index()] += 1;
        }
    }

    (rev_dep_counts, 0.5)
}

pub fn remove_small_deps(
    graph: &mut StableGraph<String, ()>,
    cum_sums: &[usize],
    threshold: usize,
) {
    for (idx, sum) in cum_sums.iter().enumerate() {
        if *sum < threshold {
            graph.remove_node(NodeIndex::new(idx));
        }
    }
}

pub fn remove_deep_deps(graph: &mut StableGraph<String, ()>, max_depth: usize) {
    // TODO: use petgraph#868 once merged
    let mut queue = VecDeque::from([(NodeIndex::new(0), 0)]);
    let mut visited = HashSet::from([NodeIndex::new(0)]);

    while let Some((node, depth)) = queue.pop_front()
        && depth < max_depth
    {
        for target in graph.neighbors(node) {
            if !visited.contains(&target) {
                queue.push_back((target, depth + 1));
                visited.insert(target);
            }
        }
    }

    for node in graph.node_indices().collect::<Vec<_>>() {
        if !visited.contains(&node) {
            graph.remove_node(node);
        }
    }
}
