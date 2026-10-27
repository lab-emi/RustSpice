//! Symbolic Forward-Mode (Jacobian) and Reverse-Mode (Adjoint) Automatic Differentiation
//! over the [`IrGraph`] SSA intermediate representation.

use rustc_hash::FxHashMap;

use crate::ssa::{BinaryOp, ExprId, IrGraph, IrNode, UnaryOp};

/// Specifies the differentiation variable target (either an input voltage or a device parameter).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiffTarget {
    /// Differentiate with respect to an input terminal voltage by name.
    Input(String),
    /// Differentiate with respect to a model/instance parameter by name.
    Param(String),
}

/// Map holding reverse-mode adjoint gradient expressions keyed by input and parameter names.
#[derive(Clone, Debug, Default)]
pub struct AdjointGradMap {
    pub input_grads: FxHashMap<String, ExprId>,
    pub param_grads: FxHashMap<String, ExprId>,
}

impl IrGraph {
    /// Computes the symbolic forward-mode partial derivative $\frac{\partial \text{expr}}{\partial \text{target}}$
    /// and appends the resulting derivative nodes into the SSA graph.
    pub fn differentiate_forward(&mut self, expr: ExprId, target: &DiffTarget) -> ExprId {
        let num_nodes = expr.0 + 1;
        let mut derivs = vec![None::<ExprId>; num_nodes];
        let zero = self.const_val(0.0);
        let one = self.const_val(1.0);
        let two = self.const_val(2.0);
        let half = self.const_val(0.5);

        for idx in 0..num_nodes {
            let node = self.nodes[idx].clone();
            let d_id = match node {
                IrNode::Const(_) => zero,
                IrNode::Input { ref name, .. } => match target {
                    DiffTarget::Input(t) if t == name => one,
                    _ => zero,
                },
                IrNode::Param { ref name, .. } => match target {
                    DiffTarget::Param(t) if t == name => one,
                    _ => zero,
                },
                IrNode::Unary { op, arg } => {
                    let dx = derivs[arg.0].unwrap_or(zero);
                    if dx == zero {
                        zero
                    } else {
                        match op {
                            UnaryOp::Neg => self.neg(dx),
                            UnaryOp::Exp => {
                                let exp_x = ExprId(idx);
                                self.mul(exp_x, dx)
                            }
                            UnaryOp::Ln => self.div(dx, arg),
                            UnaryOp::Sqrt => {
                                let sqrt_x = ExprId(idx);
                                let denom = self.mul(two, sqrt_x);
                                self.div(dx, denom)
                            }
                            UnaryOp::Sin => {
                                let cos_x = self.cos(arg);
                                self.mul(cos_x, dx)
                            }
                            UnaryOp::Cos => {
                                let sin_x = self.sin(arg);
                                let neg_sin = self.neg(sin_x);
                                self.mul(neg_sin, dx)
                            }
                            UnaryOp::Tanh => {
                                let tanh_x = ExprId(idx);
                                let tanh_sq = self.mul(tanh_x, tanh_x);
                                let sech_sq = self.sub(one, tanh_sq);
                                self.mul(sech_sq, dx)
                            }
                            UnaryOp::Abs => {
                                let cond = self.gt(arg, zero);
                                let neg_dx = self.neg(dx);
                                self.select(cond, dx, neg_dx)
                            }
                            UnaryOp::Recip => {
                                let x_sq = self.mul(arg, arg);
                                let neg_dx = self.neg(dx);
                                self.div(neg_dx, x_sq)
                            }
                        }
                    }
                }
                IrNode::Binary { op, lhs, rhs } => {
                    let da = derivs[lhs.0].unwrap_or(zero);
                    let db = derivs[rhs.0].unwrap_or(zero);
                    if da == zero && db == zero {
                        zero
                    } else {
                        match op {
                            BinaryOp::Add => self.add(da, db),
                            BinaryOp::Sub => self.sub(da, db),
                            BinaryOp::Mul => {
                                let t1 = self.mul(da, rhs);
                                let t2 = self.mul(lhs, db);
                                self.add(t1, t2)
                            }
                            BinaryOp::Div => {
                                // d(a/b) = (da * b - a * db) / (b * b)
                                let t1 = self.mul(da, rhs);
                                let t2 = self.mul(lhs, db);
                                let num = self.sub(t1, t2);
                                let den = self.mul(rhs, rhs);
                                self.div(num, den)
                            }
                            BinaryOp::Pow => {
                                // d(a^b) = a^(b-1) * b * da + a^b * ln(a) * db
                                let b_minus_1 = self.sub(rhs, one);
                                let a_pow_bm1 = self.pow(lhs, b_minus_1);
                                let term1_a = self.mul(rhs, a_pow_bm1);
                                let term1 = self.mul(term1_a, da);
                                if db == zero {
                                    term1
                                } else {
                                    let f_id = ExprId(idx);
                                    let ln_a = self.ln(lhs);
                                    let term2_a = self.mul(f_id, ln_a);
                                    let term2 = self.mul(term2_a, db);
                                    self.add(term1, term2)
                                }
                            }
                            BinaryOp::Max => {
                                let cond = self.gt(lhs, rhs);
                                self.select(cond, da, db)
                            }
                            BinaryOp::Min => {
                                let cond = self.gt(rhs, lhs);
                                self.select(cond, da, db)
                            }
                            BinaryOp::Gt => zero,
                        }
                    }
                }
                IrNode::Select {
                    cond,
                    if_true,
                    if_false,
                } => {
                    let dt = derivs[if_true.0].unwrap_or(zero);
                    let df = derivs[if_false.0].unwrap_or(zero);
                    if dt == zero && df == zero {
                        zero
                    } else {
                        self.select(cond, dt, df)
                    }
                }
                IrNode::Fma { a, b, c } => {
                    let da = derivs[a.0].unwrap_or(zero);
                    let db = derivs[b.0].unwrap_or(zero);
                    let dc = derivs[c.0].unwrap_or(zero);
                    let t1 = self.mul(da, b);
                    let t2 = self.mul(a, db);
                    let ab = self.add(t1, t2);
                    self.add(ab, dc)
                }
            };
            let _ = half;
            derivs[idx] = Some(d_id);
        }

        derivs[expr.0].unwrap_or(zero)
    }

    /// Performs Reverse-Mode Automatic Differentiation (Backpropagation / Adjoint Pullback)
    /// across the SSA graph in a single reverse-topological pass.
    ///
    /// Given a slice of `(output_expr, adjoint_seed_expr)` pairs $\{(y_i, \bar{y}_i)\}$,
    /// computes the symbolic adjoint sensitivities:
    /// $$\bar{x}_j = \sum_i \bar{y}_i \frac{\partial y_i}{\partial x_j}, \quad
    ///   \bar{\theta}_p = \sum_i \bar{y}_i \frac{\partial y_i}{\partial \theta_p}$$
    pub fn differentiate_reverse(&mut self, output_seeds: &[(ExprId, ExprId)]) -> AdjointGradMap {
        let num_nodes = self.nodes.len();
        let zero = self.const_val(0.0);
        let one = self.const_val(1.0);
        let two = self.const_val(2.0);

        let mut adj = vec![zero; num_nodes];
        for &(out_id, seed_id) in output_seeds {
            if out_id.0 < num_nodes {
                if adj[out_id.0] == zero {
                    adj[out_id.0] = seed_id;
                } else {
                    adj[out_id.0] = self.add(adj[out_id.0], seed_id);
                }
            }
        }

        let accumulate = |graph: &mut Self, slot: &mut ExprId, delta: ExprId| {
            if delta == zero {
                return;
            }
            if *slot == zero {
                *slot = delta;
            } else {
                *slot = graph.add(*slot, delta);
            }
        };

        for idx in (0..num_nodes).rev() {
            let bar = adj[idx];
            if bar == zero {
                continue;
            }
            let node = self.nodes[idx].clone();
            match node {
                IrNode::Const(_) | IrNode::Input { .. } | IrNode::Param { .. } => {}
                IrNode::Unary { op, arg } => {
                    let contrib = match op {
                        UnaryOp::Neg => self.neg(bar),
                        UnaryOp::Exp => {
                            let exp_x = ExprId(idx);
                            self.mul(bar, exp_x)
                        }
                        UnaryOp::Ln => self.div(bar, arg),
                        UnaryOp::Sqrt => {
                            let sqrt_x = ExprId(idx);
                            let denom = self.mul(two, sqrt_x);
                            self.div(bar, denom)
                        }
                        UnaryOp::Sin => {
                            let cos_x = self.cos(arg);
                            self.mul(bar, cos_x)
                        }
                        UnaryOp::Cos => {
                            let sin_x = self.sin(arg);
                            let neg_sin = self.neg(sin_x);
                            self.mul(bar, neg_sin)
                        }
                        UnaryOp::Tanh => {
                            let tanh_x = ExprId(idx);
                            let tanh_sq = self.mul(tanh_x, tanh_x);
                            let sech_sq = self.sub(one, tanh_sq);
                            self.mul(bar, sech_sq)
                        }
                        UnaryOp::Abs => {
                            let cond = self.gt(arg, zero);
                            let neg_bar = self.neg(bar);
                            self.select(cond, bar, neg_bar)
                        }
                        UnaryOp::Recip => {
                            let x_sq = self.mul(arg, arg);
                            let neg_bar = self.neg(bar);
                            self.div(neg_bar, x_sq)
                        }
                    };
                    accumulate(self, &mut adj[arg.0], contrib);
                }
                IrNode::Binary { op, lhs, rhs } => match op {
                    BinaryOp::Add => {
                        accumulate(self, &mut adj[lhs.0], bar);
                        accumulate(self, &mut adj[rhs.0], bar);
                    }
                    BinaryOp::Sub => {
                        accumulate(self, &mut adj[lhs.0], bar);
                        let neg_bar = self.neg(bar);
                        accumulate(self, &mut adj[rhs.0], neg_bar);
                    }
                    BinaryOp::Mul => {
                        let dl = self.mul(bar, rhs);
                        let dr = self.mul(bar, lhs);
                        accumulate(self, &mut adj[lhs.0], dl);
                        accumulate(self, &mut adj[rhs.0], dr);
                    }
                    BinaryOp::Div => {
                        let dl = self.div(bar, rhs);
                        let num = self.mul(bar, lhs);
                        let den = self.mul(rhs, rhs);
                        let ratio = self.div(num, den);
                        let dr = self.neg(ratio);
                        accumulate(self, &mut adj[lhs.0], dl);
                        accumulate(self, &mut adj[rhs.0], dr);
                    }
                    BinaryOp::Pow => {
                        let b_minus_1 = self.sub(rhs, one);
                        let a_pow_bm1 = self.pow(lhs, b_minus_1);
                        let b_a = self.mul(rhs, a_pow_bm1);
                        let dl = self.mul(bar, b_a);

                        let f_id = ExprId(idx);
                        let ln_a = self.ln(lhs);
                        let f_ln_a = self.mul(f_id, ln_a);
                        let dr = self.mul(bar, f_ln_a);

                        accumulate(self, &mut adj[lhs.0], dl);
                        accumulate(self, &mut adj[rhs.0], dr);
                    }
                    BinaryOp::Max => {
                        let cond = self.gt(lhs, rhs);
                        let dl = self.select(cond, bar, zero);
                        let dr = self.select(cond, zero, bar);
                        accumulate(self, &mut adj[lhs.0], dl);
                        accumulate(self, &mut adj[rhs.0], dr);
                    }
                    BinaryOp::Min => {
                        let cond = self.gt(rhs, lhs);
                        let dl = self.select(cond, bar, zero);
                        let dr = self.select(cond, zero, bar);
                        accumulate(self, &mut adj[lhs.0], dl);
                        accumulate(self, &mut adj[rhs.0], dr);
                    }
                    BinaryOp::Gt => {}
                },
                IrNode::Select {
                    cond,
                    if_true,
                    if_false,
                } => {
                    let dt = self.select(cond, bar, zero);
                    let df = self.select(cond, zero, bar);
                    accumulate(self, &mut adj[if_true.0], dt);
                    accumulate(self, &mut adj[if_false.0], df);
                }
                IrNode::Fma { a, b, c } => {
                    let da = self.mul(bar, b);
                    let db = self.mul(bar, a);
                    accumulate(self, &mut adj[a.0], da);
                    accumulate(self, &mut adj[b.0], db);
                    accumulate(self, &mut adj[c.0], bar);
                }
            }
        }

        let mut map = AdjointGradMap::default();
        for (idx, &grad_id) in adj.iter().enumerate().take(num_nodes) {
            match &self.nodes[idx] {
                IrNode::Input { name, .. } => {
                    map.input_grads.insert(name.clone(), grad_id);
                }
                IrNode::Param { name, .. } => {
                    map.param_grads.insert(name.clone(), grad_id);
                }
                _ => {}
            }
        }
        map
    }
}
