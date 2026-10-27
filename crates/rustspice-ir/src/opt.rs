//! Compiler optimization passes over [`IrGraph`]:
//! 1. Constant Folding & Algebraic Simplification
//! 2. Hash-Consing Common Subexpression Elimination (CSE)
//! 3. Dead Code Elimination (DCE) & Topological Compaction

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::ssa::{BinaryOp, ExprId, IrGraph, IrNode, NamedOutput, UnaryOp};

/// Summary metrics reported after running [`optimize_graph`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationStats {
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub cse_eliminated: usize,
    pub dce_eliminated: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum CanonicalKey {
    Const(u64),
    Input(usize),
    Param(usize),
    Unary(UnaryOp, ExprId),
    Binary(BinaryOp, ExprId, ExprId),
    Select(ExprId, ExprId, ExprId),
    Fma(ExprId, ExprId, ExprId),
}

fn canonical_f64_bits(v: f64) -> u64 {
    if v == 0.0 {
        0.0_f64.to_bits()
    } else {
        v.to_bits()
    }
}

/// Runs Constant Folding, Algebraic Simplification, Common Subexpression Elimination (CSE),
/// and Dead Code Elimination (DCE) on `graph`, returning [`OptimizationStats`].
pub fn optimize_graph(graph: &mut IrGraph) -> OptimizationStats {
    let nodes_before = graph.nodes.len();

    // Pass 1: Constant folding + Algebraic identities + Hash-consed CSE
    let mut cse_graph = IrGraph {
        device_name: graph.device_name.clone(),
        nodes: Vec::with_capacity(nodes_before),
        input_names: graph.input_names.clone(),
        param_names: graph.param_names.clone(),
        outputs: Vec::with_capacity(graph.outputs.len()),
    };

    let mut table: FxHashMap<CanonicalKey, ExprId> = FxHashMap::default();
    let mut const_vals: Vec<Option<f64>> = Vec::with_capacity(nodes_before);
    let mut remap: Vec<ExprId> = Vec::with_capacity(nodes_before);
    let mut cse_eliminated = 0usize;

    let intern_node = |g: &mut IrGraph,
                       tbl: &mut FxHashMap<CanonicalKey, ExprId>,
                       cvals: &mut Vec<Option<f64>>,
                       node: IrNode,
                       key: CanonicalKey,
                       cval: Option<f64>,
                       cse_count: &mut usize|
     -> ExprId {
        if let Some(&existing) = tbl.get(&key) {
            *cse_count += 1;
            return existing;
        }
        let id = g.push_node(node);
        cvals.push(cval);
        tbl.insert(key, id);
        id
    };

    for old_node in &graph.nodes {
        let new_id = match old_node {
            IrNode::Const(c) => {
                let key = CanonicalKey::Const(canonical_f64_bits(*c));
                intern_node(
                    &mut cse_graph,
                    &mut table,
                    &mut const_vals,
                    IrNode::Const(*c),
                    key,
                    Some(*c),
                    &mut cse_eliminated,
                )
            }
            IrNode::Input { name, index } => {
                let key = CanonicalKey::Input(*index);
                intern_node(
                    &mut cse_graph,
                    &mut table,
                    &mut const_vals,
                    IrNode::Input {
                        name: name.clone(),
                        index: *index,
                    },
                    key,
                    None,
                    &mut cse_eliminated,
                )
            }
            IrNode::Param {
                name,
                index,
                default_val,
            } => {
                let key = CanonicalKey::Param(*index);
                intern_node(
                    &mut cse_graph,
                    &mut table,
                    &mut const_vals,
                    IrNode::Param {
                        name: name.clone(),
                        index: *index,
                        default_val: *default_val,
                    },
                    key,
                    None,
                    &mut cse_eliminated,
                )
            }
            IrNode::Unary { op, arg } => {
                let m_arg = remap[arg.0];
                if let Some(c) = const_vals[m_arg.0] {
                    let folded = match op {
                        UnaryOp::Neg => -c,
                        UnaryOp::Exp => c.clamp(-80.0, 80.0).exp(),
                        UnaryOp::Ln => c.max(1e-30).ln(),
                        UnaryOp::Sqrt => c.max(0.0).sqrt(),
                        UnaryOp::Sin => c.sin(),
                        UnaryOp::Cos => c.cos(),
                        UnaryOp::Tanh => c.tanh(),
                        UnaryOp::Abs => c.abs(),
                        UnaryOp::Recip => 1.0 / c,
                    };
                    let key = CanonicalKey::Const(canonical_f64_bits(folded));
                    intern_node(
                        &mut cse_graph,
                        &mut table,
                        &mut const_vals,
                        IrNode::Const(folded),
                        key,
                        Some(folded),
                        &mut cse_eliminated,
                    )
                } else if *op == UnaryOp::Neg {
                    if let IrNode::Unary {
                        op: UnaryOp::Neg,
                        arg: inner,
                    } = cse_graph.nodes[m_arg.0]
                    {
                        inner
                    } else {
                        let key = CanonicalKey::Unary(*op, m_arg);
                        intern_node(
                            &mut cse_graph,
                            &mut table,
                            &mut const_vals,
                            IrNode::Unary {
                                op: *op,
                                arg: m_arg,
                            },
                            key,
                            None,
                            &mut cse_eliminated,
                        )
                    }
                } else {
                    let key = CanonicalKey::Unary(*op, m_arg);
                    intern_node(
                        &mut cse_graph,
                        &mut table,
                        &mut const_vals,
                        IrNode::Unary {
                            op: *op,
                            arg: m_arg,
                        },
                        key,
                        None,
                        &mut cse_eliminated,
                    )
                }
            }
            IrNode::Binary { op, lhs, rhs } => {
                let mut l = remap[lhs.0];
                let mut r = remap[rhs.0];
                let cl = const_vals[l.0];
                let cr = const_vals[r.0];

                if let (Some(a), Some(b)) = (cl, cr) {
                    let folded = match op {
                        BinaryOp::Add => a + b,
                        BinaryOp::Sub => a - b,
                        BinaryOp::Mul => a * b,
                        BinaryOp::Div => a / b,
                        BinaryOp::Pow => a.powf(b),
                        BinaryOp::Max => a.max(b),
                        BinaryOp::Min => a.min(b),
                        BinaryOp::Gt => {
                            if a > b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                    };
                    let key = CanonicalKey::Const(canonical_f64_bits(folded));
                    intern_node(
                        &mut cse_graph,
                        &mut table,
                        &mut const_vals,
                        IrNode::Const(folded),
                        key,
                        Some(folded),
                        &mut cse_eliminated,
                    )
                } else {
                    // Algebraic simplifications
                    match op {
                        BinaryOp::Add => {
                            if cl == Some(0.0) {
                                r
                            } else if cr == Some(0.0) {
                                l
                            } else {
                                if l.0 > r.0 {
                                    std::mem::swap(&mut l, &mut r);
                                }
                                let key = CanonicalKey::Binary(BinaryOp::Add, l, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Binary {
                                        op: BinaryOp::Add,
                                        lhs: l,
                                        rhs: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            }
                        }
                        BinaryOp::Sub => {
                            if cr == Some(0.0) {
                                l
                            } else if l == r {
                                let key = CanonicalKey::Const(canonical_f64_bits(0.0));
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Const(0.0),
                                    key,
                                    Some(0.0),
                                    &mut cse_eliminated,
                                )
                            } else if cl == Some(0.0) {
                                let key = CanonicalKey::Unary(UnaryOp::Neg, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Unary {
                                        op: UnaryOp::Neg,
                                        arg: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            } else {
                                let key = CanonicalKey::Binary(BinaryOp::Sub, l, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Binary {
                                        op: BinaryOp::Sub,
                                        lhs: l,
                                        rhs: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            }
                        }
                        BinaryOp::Mul => {
                            if cl == Some(0.0) || cr == Some(0.0) {
                                let key = CanonicalKey::Const(canonical_f64_bits(0.0));
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Const(0.0),
                                    key,
                                    Some(0.0),
                                    &mut cse_eliminated,
                                )
                            } else if cl == Some(1.0) {
                                r
                            } else if cr == Some(1.0) {
                                l
                            } else if cl == Some(-1.0) {
                                let key = CanonicalKey::Unary(UnaryOp::Neg, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Unary {
                                        op: UnaryOp::Neg,
                                        arg: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            } else if cr == Some(-1.0) {
                                let key = CanonicalKey::Unary(UnaryOp::Neg, l);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Unary {
                                        op: UnaryOp::Neg,
                                        arg: l,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            } else {
                                if l.0 > r.0 {
                                    std::mem::swap(&mut l, &mut r);
                                }
                                let key = CanonicalKey::Binary(BinaryOp::Mul, l, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Binary {
                                        op: BinaryOp::Mul,
                                        lhs: l,
                                        rhs: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            }
                        }
                        BinaryOp::Div => {
                            if cl == Some(0.0) {
                                let key = CanonicalKey::Const(canonical_f64_bits(0.0));
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Const(0.0),
                                    key,
                                    Some(0.0),
                                    &mut cse_eliminated,
                                )
                            } else if cr == Some(1.0) {
                                l
                            } else if l == r {
                                let key = CanonicalKey::Const(canonical_f64_bits(1.0));
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Const(1.0),
                                    key,
                                    Some(1.0),
                                    &mut cse_eliminated,
                                )
                            } else {
                                let key = CanonicalKey::Binary(BinaryOp::Div, l, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Binary {
                                        op: BinaryOp::Div,
                                        lhs: l,
                                        rhs: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            }
                        }
                        BinaryOp::Pow => {
                            if cr == Some(0.0) {
                                let key = CanonicalKey::Const(canonical_f64_bits(1.0));
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Const(1.0),
                                    key,
                                    Some(1.0),
                                    &mut cse_eliminated,
                                )
                            } else if cr == Some(1.0) {
                                l
                            } else if cr == Some(2.0) {
                                let key = CanonicalKey::Binary(BinaryOp::Mul, l, l);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Binary {
                                        op: BinaryOp::Mul,
                                        lhs: l,
                                        rhs: l,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            } else {
                                let key = CanonicalKey::Binary(BinaryOp::Pow, l, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Binary {
                                        op: BinaryOp::Pow,
                                        lhs: l,
                                        rhs: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            }
                        }
                        BinaryOp::Max | BinaryOp::Min => {
                            if l == r {
                                l
                            } else {
                                if l.0 > r.0 {
                                    std::mem::swap(&mut l, &mut r);
                                }
                                let key = CanonicalKey::Binary(*op, l, r);
                                intern_node(
                                    &mut cse_graph,
                                    &mut table,
                                    &mut const_vals,
                                    IrNode::Binary {
                                        op: *op,
                                        lhs: l,
                                        rhs: r,
                                    },
                                    key,
                                    None,
                                    &mut cse_eliminated,
                                )
                            }
                        }
                        BinaryOp::Gt => {
                            let key = CanonicalKey::Binary(BinaryOp::Gt, l, r);
                            intern_node(
                                &mut cse_graph,
                                &mut table,
                                &mut const_vals,
                                IrNode::Binary {
                                    op: BinaryOp::Gt,
                                    lhs: l,
                                    rhs: r,
                                },
                                key,
                                None,
                                &mut cse_eliminated,
                            )
                        }
                    }
                }
            }
            IrNode::Select {
                cond,
                if_true,
                if_false,
            } => {
                let mc = remap[cond.0];
                let mt = remap[if_true.0];
                let mf = remap[if_false.0];
                if mt == mf {
                    mt
                } else if let Some(cval) = const_vals[mc.0] {
                    if cval > 0.0 {
                        mt
                    } else {
                        mf
                    }
                } else {
                    let key = CanonicalKey::Select(mc, mt, mf);
                    intern_node(
                        &mut cse_graph,
                        &mut table,
                        &mut const_vals,
                        IrNode::Select {
                            cond: mc,
                            if_true: mt,
                            if_false: mf,
                        },
                        key,
                        None,
                        &mut cse_eliminated,
                    )
                }
            }
            IrNode::Fma { a, b, c } => {
                let mut ma = remap[a.0];
                let mut mb = remap[b.0];
                let mc = remap[c.0];
                if ma.0 > mb.0 {
                    std::mem::swap(&mut ma, &mut mb);
                }
                let key = CanonicalKey::Fma(ma, mb, mc);
                intern_node(
                    &mut cse_graph,
                    &mut table,
                    &mut const_vals,
                    IrNode::Fma {
                        a: ma,
                        b: mb,
                        c: mc,
                    },
                    key,
                    None,
                    &mut cse_eliminated,
                )
            }
        };
        remap.push(new_id);
    }

    for out in &graph.outputs {
        cse_graph.outputs.push(NamedOutput {
            name: out.name.clone(),
            expr: remap[out.expr.0],
        });
    }

    // Pass 2: Dead Code Elimination (DCE) via backward reachability from outputs
    let cse_len = cse_graph.nodes.len();
    let mut live = vec![false; cse_len];
    for out in &cse_graph.outputs {
        live[out.expr.0] = true;
    }

    for idx in (0..cse_len).rev() {
        if !live[idx] {
            continue;
        }
        match &cse_graph.nodes[idx] {
            IrNode::Const(_) | IrNode::Input { .. } | IrNode::Param { .. } => {}
            IrNode::Unary { arg, .. } => {
                live[arg.0] = true;
            }
            IrNode::Binary { lhs, rhs, .. } => {
                live[lhs.0] = true;
                live[rhs.0] = true;
            }
            IrNode::Select {
                cond,
                if_true,
                if_false,
            } => {
                live[cond.0] = true;
                live[if_true.0] = true;
                live[if_false.0] = true;
            }
            IrNode::Fma { a, b, c } => {
                live[a.0] = true;
                live[b.0] = true;
                live[c.0] = true;
            }
        }
    }

    let mut dce_remap = vec![ExprId(0); cse_len];
    let mut final_nodes = Vec::with_capacity(cse_len);
    for (idx, is_live) in live.iter().copied().enumerate() {
        if !is_live {
            continue;
        }
        let new_idx = ExprId(final_nodes.len());
        dce_remap[idx] = new_idx;
        let mapped_node = match &cse_graph.nodes[idx] {
            IrNode::Const(c) => IrNode::Const(*c),
            IrNode::Input { name, index } => IrNode::Input {
                name: name.clone(),
                index: *index,
            },
            IrNode::Param {
                name,
                index,
                default_val,
            } => IrNode::Param {
                name: name.clone(),
                index: *index,
                default_val: *default_val,
            },
            IrNode::Unary { op, arg } => IrNode::Unary {
                op: *op,
                arg: dce_remap[arg.0],
            },
            IrNode::Binary { op, lhs, rhs } => IrNode::Binary {
                op: *op,
                lhs: dce_remap[lhs.0],
                rhs: dce_remap[rhs.0],
            },
            IrNode::Select {
                cond,
                if_true,
                if_false,
            } => IrNode::Select {
                cond: dce_remap[cond.0],
                if_true: dce_remap[if_true.0],
                if_false: dce_remap[if_false.0],
            },
            IrNode::Fma { a, b, c } => IrNode::Fma {
                a: dce_remap[a.0],
                b: dce_remap[b.0],
                c: dce_remap[c.0],
            },
        };
        final_nodes.push(mapped_node);
    }

    let dce_eliminated = cse_len.saturating_sub(final_nodes.len());
    graph.nodes = final_nodes;
    graph.outputs = cse_graph
        .outputs
        .into_iter()
        .map(|o| NamedOutput {
            name: o.name,
            expr: dce_remap[o.expr.0],
        })
        .collect();

    OptimizationStats {
        nodes_before,
        nodes_after: graph.nodes.len(),
        cse_eliminated,
        dce_eliminated,
    }
}
