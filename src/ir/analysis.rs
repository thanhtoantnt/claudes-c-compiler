//! Shared CFG and dominator tree analysis utilities.
//!
//! These functions compute control flow graph (CFG) information and dominator
//! trees using the Cooper-Harvey-Kennedy algorithm. They are used by mem2reg
//! for SSA construction and by optimization passes (e.g., GVN) that need
//! dominator information.
//!
//! Performance: The CFG is stored as a flat CSR (Compressed Sparse Row)
//! adjacency list (`FlatAdj`) instead of `Vec<Vec<usize>>`. This reduces
//! n+1 heap allocations to 2 per build_cfg call and improves cache locality,
//! which is critical since build_cfg is called per-function by GVN, LICM,
//! if_convert, and mem2reg.

use crate::common::fx_hash::{FxHashMap, FxHashSet};
use crate::ir::reexports::{BlockId, Instruction, IrFunction, Terminator};

// ── Flat adjacency list (CSR format) ──────────────────────────────────────────

/// A flat adjacency list using Compressed Sparse Row (CSR) format.
///
/// Stores `n` variable-length rows in two flat arrays:
/// - `offsets[i]..offsets[i+1]` is the range of indices into `data` for row i
/// - `data[offsets[i]..offsets[i+1]]` contains the neighbors of node i
///
/// This uses exactly 2 heap allocations regardless of the number of rows,
/// compared to n+1 for `Vec<Vec<usize>>`. The flat layout also provides
/// better cache locality when iterating over adjacency lists.
pub struct FlatAdj {
    /// offsets[i] is the start index in `data` for row i.
    /// offsets[n] is the total number of entries (= data.len()).
    /// Length: n + 1
    offsets: Vec<u32>,
    /// Flat storage of all adjacency entries.
    data: Vec<u32>,
}

impl FlatAdj {
    /// Get the adjacency list (neighbors) of node `i` as a slice.
    #[inline]
    pub fn row(&self, i: usize) -> &[u32] {
        let start = self.offsets[i] as usize;
        let end = self.offsets[i + 1] as usize;
        &self.data[start..end]
    }

    /// Get the number of neighbors of node `i`.
    #[inline]
    pub fn len(&self, i: usize) -> usize {
        (self.offsets[i + 1] - self.offsets[i]) as usize
    }

    /// Build a FlatAdj from `Vec<Vec<usize>>` for tests.
    #[cfg(test)]
    pub fn from_vecs_usize(vecs: &[Vec<usize>]) -> Self {
        let n = vecs.len();
        let mut offsets = Vec::with_capacity(n + 1);
        let total: usize = vecs.iter().map(|v| v.len()).sum();
        let mut data = Vec::with_capacity(total);

        let mut offset = 0u32;
        for v in vecs {
            offsets.push(offset);
            for &val in v {
                data.push(val as u32);
            }
            offset += v.len() as u32;
        }
        offsets.push(offset);

        FlatAdj { offsets, data }
    }

    /// Build a FlatAdj from a Vec<Vec<u32>> (used in the construction phase).
    fn from_vecs(vecs: Vec<Vec<u32>>) -> Self {
        let n = vecs.len();
        let mut offsets = Vec::with_capacity(n + 1);
        let total: usize = vecs.iter().map(|v| v.len()).sum();
        let mut data = Vec::with_capacity(total);

        let mut offset = 0u32;
        for v in &vecs {
            offsets.push(offset);
            data.extend_from_slice(v);
            offset += v.len() as u32;
        }
        offsets.push(offset);

        FlatAdj { offsets, data }
    }
}

// ── Label map ─────────────────────────────────────────────────────────────────

/// Build a map from block label to block index.
pub fn build_label_map(func: &IrFunction) -> FxHashMap<BlockId, usize> {
    func.blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.label, i))
        .collect()
}

// ── CFG construction ──────────────────────────────────────────────────────────

/// Build predecessor and successor lists from the function's CFG.
/// Returns (preds, succs) as flat adjacency lists (CSR format).
///
/// Uses only 4 heap allocations total (2 per FlatAdj) instead of 2*n+2 for
/// the old `Vec<Vec<usize>>` representation.
pub fn build_cfg(
    func: &IrFunction,
    label_to_idx: &FxHashMap<BlockId, usize>,
) -> (FlatAdj, FlatAdj) {
    let n = func.blocks.len();
    // Build using temporary Vec<Vec<u32>> then flatten to CSR.
    // The inner Vecs are tiny (usually 1-4 entries) so this is fast.
    let mut preds: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut succs: Vec<Vec<u32>> = vec![Vec::new(); n];

    for (i, block) in func.blocks.iter().enumerate() {
        let i32 = i as u32;
        match &block.terminator {
            Terminator::Branch(label) => {
                if let Some(&target) = label_to_idx.get(label) {
                    succs[i].push(target as u32);
                    preds[target].push(i32);
                }
            }
            Terminator::CondBranch { true_label, false_label, .. } => {
                if let Some(&t) = label_to_idx.get(true_label) {
                    succs[i].push(t as u32);
                    preds[t].push(i32);
                }
                if let Some(&f) = label_to_idx.get(false_label) {
                    let f32v = f as u32;
                    if !succs[i].contains(&f32v) {
                        succs[i].push(f32v);
                    }
                    preds[f].push(i32);
                }
            }
            Terminator::IndirectBranch { possible_targets, .. } => {
                for label in possible_targets {
                    if let Some(&t) = label_to_idx.get(label) {
                        let t32 = t as u32;
                        if !succs[i].contains(&t32) {
                            succs[i].push(t32);
                        }
                        preds[t].push(i32);
                    }
                }
            }
            Terminator::Switch { cases, default, .. } => {
                if let Some(&d) = label_to_idx.get(default) {
                    succs[i].push(d as u32);
                    preds[d].push(i32);
                }
                for (_, label) in cases {
                    if let Some(&t) = label_to_idx.get(label) {
                        let t32 = t as u32;
                        if !succs[i].contains(&t32) {
                            succs[i].push(t32);
                        }
                        preds[t].push(i32);
                    }
                }
            }
            Terminator::Return(_) | Terminator::Unreachable => {}
        }
        // InlineAsm goto_labels are implicit control flow edges.
        for inst in &block.instructions {
            if let Instruction::InlineAsm { goto_labels, .. } = inst {
                for (_, label) in goto_labels {
                    if let Some(&t) = label_to_idx.get(label) {
                        let t32 = t as u32;
                        if !succs[i].contains(&t32) {
                            succs[i].push(t32);
                        }
                        preds[t].push(i32);
                    }
                }
            }
        }
    }

    (FlatAdj::from_vecs(preds), FlatAdj::from_vecs(succs))
}

// ── Reverse postorder ─────────────────────────────────────────────────────────

/// Compute reverse postorder traversal of the CFG.
pub fn compute_reverse_postorder(num_blocks: usize, succs: &FlatAdj) -> Vec<usize> {
    let mut visited = vec![false; num_blocks];
    let mut postorder = Vec::with_capacity(num_blocks);

    fn dfs(node: usize, succs: &FlatAdj, visited: &mut Vec<bool>, postorder: &mut Vec<usize>) {
        visited[node] = true;
        for &succ in succs.row(node) {
            let s = succ as usize;
            if !visited[s] {
                dfs(s, succs, visited, postorder);
            }
        }
        postorder.push(node);
    }

    if num_blocks > 0 {
        dfs(0, succs, &mut visited, &mut postorder);
    }

    postorder.reverse();
    postorder
}

// ── Dominator computation ─────────────────────────────────────────────────────

/// Intersect two dominators using RPO numbering (Cooper-Harvey-Kennedy).
fn intersect(
    mut finger1: usize,
    mut finger2: usize,
    idom: &[usize],
    rpo_number: &[usize],
) -> usize {
    while finger1 != finger2 {
        while rpo_number[finger1] > rpo_number[finger2] {
            finger1 = idom[finger1];
        }
        while rpo_number[finger2] > rpo_number[finger1] {
            finger2 = idom[finger2];
        }
    }
    finger1
}

/// Compute immediate dominators using the Cooper-Harvey-Kennedy algorithm.
/// Returns idom[i] = immediate dominator of block i (idom[0] = 0 for entry).
/// Uses usize::MAX as sentinel for undefined/unreachable blocks.
pub fn compute_dominators(
    num_blocks: usize,
    preds: &FlatAdj,
    succs: &FlatAdj,
) -> Vec<usize> {
    const UNDEF: usize = usize::MAX;

    let rpo = compute_reverse_postorder(num_blocks, succs);
    let mut rpo_number = vec![UNDEF; num_blocks];
    for (order, &block) in rpo.iter().enumerate() {
        rpo_number[block] = order;
    }

    let mut idom = vec![UNDEF; num_blocks];
    if rpo.is_empty() {
        return idom;
    }
    idom[rpo[0]] = rpo[0]; // Entry dominates itself

    let mut changed = true;
    while changed {
        changed = false;
        for &b in rpo.iter().skip(1) {
            if rpo_number[b] == UNDEF {
                continue;
            }

            let mut new_idom = UNDEF;
            for &p in preds.row(b) {
                let p = p as usize;
                if idom[p] != UNDEF {
                    new_idom = p;
                    break;
                }
            }

            if new_idom == UNDEF {
                continue;
            }

            for &p in preds.row(b) {
                let p = p as usize;
                if p == new_idom {
                    continue;
                }
                if idom[p] != UNDEF {
                    new_idom = intersect(new_idom, p, &idom, &rpo_number);
                }
            }

            if idom[b] != new_idom {
                idom[b] = new_idom;
                changed = true;
            }
        }
    }

    idom
}

// ── Dominance frontiers ───────────────────────────────────────────────────────

/// Compute dominance frontiers for each block.
/// DF(b) = set of blocks where b's dominance ends (join points).
pub fn compute_dominance_frontiers(
    num_blocks: usize,
    preds: &FlatAdj,
    idom: &[usize],
) -> Vec<FxHashSet<usize>> {
    let mut df = vec![FxHashSet::default(); num_blocks];

    for b in 0..num_blocks {
        if preds.len(b) < 2 {
            continue;
        }
        for &p in preds.row(b) {
            let mut runner = p as usize;
            while runner != idom[b] && runner != usize::MAX {
                df[runner].insert(b);
                if runner == idom[runner] {
                    break;
                }
                runner = idom[runner];
            }
        }
    }

    df
}

// ── Dominator tree ────────────────────────────────────────────────────────────

/// Build dominator tree children lists from idom array.
/// children[b] lists block indices whose immediate dominator is b.
pub fn build_dom_tree_children(num_blocks: usize, idom: &[usize]) -> Vec<Vec<usize>> {
    let mut children = vec![Vec::new(); num_blocks];
    for b in 1..num_blocks {
        if idom[b] != usize::MAX && idom[b] != b {
            children[idom[b]].push(b);
        }
    }
    children
}

// ── Cached analysis bundle ──────────────────────────────────────────────────

/// Pre-computed CFG analysis results shared across multiple passes within
/// a single pipeline iteration.
///
/// GVN, LICM, and IVSR all need the same CFG, dominator, and loop analysis.
/// Since GVN does not modify the CFG (it only replaces instruction operands),
/// these results remain valid across all three passes. Computing them once
/// and sharing avoids redundant `build_cfg` + `compute_dominators` +
/// `find_natural_loops` calls per function per iteration.
pub struct CfgAnalysis {
    pub preds: FlatAdj,
    pub succs: FlatAdj,
    pub idom: Vec<usize>,
    pub dom_children: Vec<Vec<usize>>,
    pub num_blocks: usize,
}

impl CfgAnalysis {
    /// Build a complete CFG analysis bundle for a function.
    pub fn build(func: &IrFunction) -> Self {
        let num_blocks = func.blocks.len();
        let label_to_idx = build_label_map(func);
        let (preds, succs) = build_cfg(func, &label_to_idx);
        let idom = compute_dominators(num_blocks, &preds, &succs);
        let dom_children = build_dom_tree_children(num_blocks, &idom);
        CfgAnalysis {
            preds,
            succs,
            idom,
            dom_children,
            num_blocks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::VecDeque;

    fn graph_strategy(max_blocks: usize, max_row_len: usize) -> impl Strategy<Value = Vec<Vec<usize>>> {
        (0..=max_blocks).prop_flat_map(move |num_blocks| {
            prop::collection::vec(
                prop::collection::vec(0..max_blocks.max(1), 0..=max_row_len),
                num_blocks,
            )
            .prop_map(move |rows| {
                rows.into_iter()
                    .map(|row| {
                        if num_blocks == 0 {
                            Vec::new()
                        } else {
                            row.into_iter().map(|x| x % num_blocks).collect()
                        }
                    })
                    .collect()
            })
        })
    }

    fn reachable_from_zero(succs: &FlatAdj, num_blocks: usize) -> Vec<bool> {
        let mut reachable = vec![false; num_blocks];
        if num_blocks == 0 {
            return reachable;
        }

        let mut queue = VecDeque::new();
        reachable[0] = true;
        queue.push_back(0);

        while let Some(node) = queue.pop_front() {
            for &succ in succs.row(node) {
                let succ = succ as usize;
                if !reachable[succ] {
                    reachable[succ] = true;
                    queue.push_back(succ);
                }
            }
        }

        reachable
    }

    fn expected_rpo(num_blocks: usize, succs: &FlatAdj) -> Vec<usize> {
        fn dfs(node: usize, succs: &FlatAdj, visited: &mut [bool], postorder: &mut Vec<usize>) {
            visited[node] = true;
            for &succ in succs.row(node) {
                let succ = succ as usize;
                if !visited[succ] {
                    dfs(succ, succs, visited, postorder);
                }
            }
            postorder.push(node);
        }

        if num_blocks == 0 {
            return Vec::new();
        }

        let mut visited = vec![false; num_blocks];
        let mut postorder = Vec::with_capacity(num_blocks);
        dfs(0, succs, &mut visited, &mut postorder);
        postorder.reverse();
        postorder
    }

    fn dedup_rows(rows: &[Vec<usize>]) -> Vec<Vec<usize>> {
        rows.iter()
            .map(|row| {
                let mut deduped = Vec::new();
                for &succ in row {
                    if !deduped.contains(&succ) {
                        deduped.push(succ);
                    }
                }
                deduped
            })
            .collect()
    }

    proptest! {
        #[test]
        fn empty_graph_returns_empty_rpo(_unit in Just(())) {
            let succs = FlatAdj::from_vecs_usize(&[]);
            prop_assert_eq!(compute_reverse_postorder(0, &succs), Vec::<usize>::new());
        }

        #[test]
        fn rpo_matches_reference_dfs_on_random_graphs(rows in graph_strategy(8, 8)) {
            let num_blocks = rows.len();
            let succs = FlatAdj::from_vecs_usize(&rows);
            prop_assert_eq!(compute_reverse_postorder(num_blocks, &succs), expected_rpo(num_blocks, &succs));
        }

        #[test]
        fn rpo_lists_each_reachable_block_exactly_once(rows in graph_strategy(8, 8)) {
            let num_blocks = rows.len();
            let succs = FlatAdj::from_vecs_usize(&rows);
            let rpo = compute_reverse_postorder(num_blocks, &succs);
            let reachable = reachable_from_zero(&succs, num_blocks);

            prop_assert!(rpo.iter().all(|&node| node < num_blocks));
            prop_assert_eq!(rpo.len(), reachable.iter().filter(|&&seen| seen).count());

            let mut seen = vec![false; num_blocks];
            for &node in &rpo {
                prop_assert!(reachable[node]);
                prop_assert!(!seen[node]);
                seen[node] = true;
            }

            for node in 0..num_blocks {
                prop_assert_eq!(seen[node], reachable[node]);
            }
        }

        #[test]
        fn rpo_is_insensitive_to_duplicate_successors(rows in graph_strategy(8, 8)) {
            let num_blocks = rows.len();
            let succs = FlatAdj::from_vecs_usize(&rows);
            let deduped = FlatAdj::from_vecs_usize(&dedup_rows(&rows));

            prop_assert_eq!(compute_reverse_postorder(num_blocks, &succs), compute_reverse_postorder(num_blocks, &deduped));
        }

        #[test]
        fn non_empty_graphs_start_with_entry_block(rows in graph_strategy(8, 8)) {
            let num_blocks = rows.len();
            let succs = FlatAdj::from_vecs_usize(&rows);
            let rpo = compute_reverse_postorder(num_blocks, &succs);

            if num_blocks == 0 {
                prop_assert!(rpo.is_empty());
            } else {
                prop_assert_eq!(rpo.first().copied(), Some(0));
            }
        }
    }
}
