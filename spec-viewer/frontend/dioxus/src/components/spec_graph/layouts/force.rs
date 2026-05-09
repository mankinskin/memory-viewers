use std::collections::HashMap;

use viewer_api_dioxus::Node3D;

use crate::types::{SpecGraphEdge, SpecGraphNode};

use super::super::model::LayoutParams;

pub(super) fn layout_force(
    nodes: &[SpecGraphNode],
    edges: &[SpecGraphEdge],
    params: LayoutParams,
) -> Vec<Node3D> {
    let node_count = nodes.len();
    let index = node_index(nodes);
    let mut positions = initial_positions(node_count, params.spread);
    let indexed_edges = indexed_edges(edges, &index);
    let k = params.link_dist.max(0.1);
    let mut temperature = 0.6_f32 * params.spread;

    for _ in 0..params.iterations.max(1) {
        let mut displacement = vec![[0.0_f32; 3]; node_count];
        apply_repulsion(&positions, &mut displacement, params.repulsion, k);
        apply_attraction(&positions, &mut displacement, &indexed_edges, k);
        step_positions(&mut positions, &displacement, temperature);
        temperature *= 0.97;
    }

    nodes
        .iter()
        .enumerate()
        .map(|(i, node)| Node3D {
            id: node.id.clone(),
            label: node.title.clone(),
            state: node.state.clone(),
            x: positions[i][0],
            y: positions[i][1],
            z: positions[i][2],
        })
        .collect()
}

fn node_index(nodes: &[SpecGraphNode]) -> HashMap<&str, usize> {
    nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect()
}

fn initial_positions(node_count: usize, spread: f32) -> Vec<[f32; 3]> {
    (0..node_count)
        .map(|i| {
            let phi = (1.0 - 2.0 * (i as f32 + 0.5) / node_count as f32).acos();
            let theta = std::f32::consts::PI * (1.0 + 5.0_f32.sqrt()) * i as f32;
            let radius = 4.0 * spread;
            [
                radius * phi.sin() * theta.cos(),
                radius * phi.sin() * theta.sin(),
                radius * phi.cos(),
            ]
        })
        .collect()
}

fn indexed_edges(edges: &[SpecGraphEdge], index: &HashMap<&str, usize>) -> Vec<(usize, usize)> {
    edges
        .iter()
        .filter_map(|edge| {
            let &from = index.get(edge.from.as_str())?;
            let &to = index.get(edge.to.as_str())?;
            Some((from, to))
        })
        .collect()
}

fn apply_repulsion(
    positions: &[[f32; 3]],
    displacement: &mut [[f32; 3]],
    repulsion: f32,
    k: f32,
) {
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            let delta = [
                positions[i][0] - positions[j][0],
                positions[i][1] - positions[j][1],
                positions[i][2] - positions[j][2],
            ];
            let dist2 = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).max(0.01);
            let dist = dist2.sqrt();
            let force = repulsion * k * k / dist2;
            let unit = [delta[0] / dist, delta[1] / dist, delta[2] / dist];
            for axis in 0..3 {
                displacement[i][axis] += unit[axis] * force;
                displacement[j][axis] -= unit[axis] * force;
            }
        }
    }
}

fn apply_attraction(
    positions: &[[f32; 3]],
    displacement: &mut [[f32; 3]],
    edges: &[(usize, usize)],
    k: f32,
) {
    for &(from, to) in edges {
        if from == to {
            continue;
        }
        let delta = [
            positions[from][0] - positions[to][0],
            positions[from][1] - positions[to][1],
            positions[from][2] - positions[to][2],
        ];
        let dist = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt().max(0.01);
        let force = dist * dist / k;
        let unit = [delta[0] / dist, delta[1] / dist, delta[2] / dist];
        for axis in 0..3 {
            displacement[from][axis] -= unit[axis] * force;
            displacement[to][axis] += unit[axis] * force;
        }
    }
}

fn step_positions(positions: &mut [[f32; 3]], displacement: &[[f32; 3]], temperature: f32) {
    for (position, delta) in positions.iter_mut().zip(displacement.iter()) {
        let magnitude = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt().max(0.001);
        let step = magnitude.min(temperature);
        for axis in 0..3 {
            position[axis] += delta[axis] / magnitude * step;
        }
    }
}