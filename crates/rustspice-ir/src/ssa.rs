//! Static Single Assignment (SSA) symbolic expression graph representation.

use serde::{Deserialize, Serialize};

/// Strongly-typed handle referencing a node inside an [`IrGraph`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExprId(pub usize);

/// Unary mathematical operations supported in the device equation IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Exp,
    Ln,
    Sqrt,
    Sin,
    Cos,
    Tanh,
    Abs,
    Recip,
}

/// Binary mathematical operations supported in the device equation IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Max,
    Min,
    /// Branchless comparison returning `1.0` when `lhs > rhs`, else `0.0`.
    Gt,
}

/// A single SSA operation node in the symbolic computation graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IrNode {
    /// Compile-time scalar floating-point constant.
    Const(f64),
    /// Terminal voltage difference or state variable input (e.g., `v_d`, `v_gs`).
    Input { name: String, index: usize },
    /// Model or instance parameter (e.g., `w`, `l`, `is`, `vto`).
    Param {
        name: String,
        index: usize,
        default_val: f64,
    },
    /// Unary mathematical operator.
    Unary { op: UnaryOp, arg: ExprId },
    /// Binary mathematical operator.
    Binary {
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    /// Branchless ternary selection: evaluates to `if_true` when `cond > 0.0`, else `if_false`.
    Select {
        cond: ExprId,
        if_true: ExprId,
        if_false: ExprId,
    },
    /// Hardware fused multiply-add: `a * b + c`.
    Fma { a: ExprId, b: ExprId, c: ExprId },
}

/// Named output entry in an [`IrGraph`] (e.g., current `i_d`, Jacobian `g_d`, or parameter sensitivity).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedOutput {
    pub name: String,
    pub expr: ExprId,
}

/// Symbolic SSA computation graph for semiconductor device physics equations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IrGraph {
    pub device_name: String,
    pub nodes: Vec<IrNode>,
    pub input_names: Vec<String>,
    pub param_names: Vec<(String, f64)>,
    pub outputs: Vec<NamedOutput>,
}

impl IrGraph {
    /// Creates a new empty [`IrGraph`] for the given device model name.
    pub fn new(device_name: impl Into<String>) -> Self {
        Self {
            device_name: device_name.into(),
            nodes: Vec::new(),
            input_names: Vec::new(),
            param_names: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Appends a raw [`IrNode`] and returns its [`ExprId`].
    #[inline]
    pub fn push_node(&mut self, node: IrNode) -> ExprId {
        let id = ExprId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    /// Inserts a constant literal into the SSA graph.
    #[inline]
    pub fn const_val(&mut self, val: f64) -> ExprId {
        self.push_node(IrNode::Const(val))
    }

    /// Declares or retrieves a terminal voltage input variable.
    pub fn input(&mut self, name: impl Into<String>) -> ExprId {
        let name = name.into();
        if let Some((idx, _)) = self
            .input_names
            .iter()
            .enumerate()
            .find(|(_, n)| **n == name)
        {
            for (node_idx, node) in self.nodes.iter().enumerate() {
                if let IrNode::Input { index, .. } = node {
                    if *index == idx {
                        return ExprId(node_idx);
                    }
                }
            }
            return self.push_node(IrNode::Input { name, index: idx });
        }
        let idx = self.input_names.len();
        self.input_names.push(name.clone());
        self.push_node(IrNode::Input { name, index: idx })
    }

    /// Declares or retrieves a device model/instance parameter with a default value.
    pub fn param(&mut self, name: impl Into<String>, default_val: f64) -> ExprId {
        let name = name.into();
        if let Some((idx, _)) = self
            .param_names
            .iter()
            .enumerate()
            .find(|(_, (n, _))| *n == name)
        {
            for (node_idx, node) in self.nodes.iter().enumerate() {
                if let IrNode::Param { index, .. } = node {
                    if *index == idx {
                        return ExprId(node_idx);
                    }
                }
            }
            return self.push_node(IrNode::Param {
                name,
                index: idx,
                default_val,
            });
        }
        let idx = self.param_names.len();
        self.param_names.push((name.clone(), default_val));
        self.push_node(IrNode::Param {
            name,
            index: idx,
            default_val,
        })
    }

    /// Registers a named output expression in the graph.
    pub fn add_output(&mut self, name: impl Into<String>, expr: ExprId) {
        let name = name.into();
        if let Some(existing) = self.outputs.iter_mut().find(|o| o.name == name) {
            existing.expr = expr;
        } else {
            self.outputs.push(NamedOutput { name, expr });
        }
    }

    /// Looks up a registered output expression by name.
    pub fn find_output(&self, name: &str) -> Option<ExprId> {
        self.outputs.iter().find(|o| o.name == name).map(|o| o.expr)
    }

    #[inline]
    pub fn neg(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Neg,
            arg,
        })
    }

    #[inline]
    pub fn exp(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Exp,
            arg,
        })
    }

    #[inline]
    pub fn ln(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Ln,
            arg,
        })
    }

    #[inline]
    pub fn sqrt(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Sqrt,
            arg,
        })
    }

    #[inline]
    pub fn sin(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Sin,
            arg,
        })
    }

    #[inline]
    pub fn cos(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Cos,
            arg,
        })
    }

    #[inline]
    pub fn tanh(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Tanh,
            arg,
        })
    }

    #[inline]
    pub fn abs(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Abs,
            arg,
        })
    }

    #[inline]
    pub fn recip(&mut self, arg: ExprId) -> ExprId {
        self.push_node(IrNode::Unary {
            op: UnaryOp::Recip,
            arg,
        })
    }

    #[inline]
    pub fn add(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Add,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn sub(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Sub,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn mul(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Mul,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn div(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Div,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn pow(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Pow,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn max(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Max,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn min(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Min,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn gt(&mut self, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.push_node(IrNode::Binary {
            op: BinaryOp::Gt,
            lhs,
            rhs,
        })
    }

    #[inline]
    pub fn select(&mut self, cond: ExprId, if_true: ExprId, if_false: ExprId) -> ExprId {
        self.push_node(IrNode::Select {
            cond,
            if_true,
            if_false,
        })
    }

    #[inline]
    pub fn fma(&mut self, a: ExprId, b: ExprId, c: ExprId) -> ExprId {
        self.push_node(IrNode::Fma { a, b, c })
    }

    /// Evaluates all SSA nodes in topological order for a given slice of inputs and parameters.
    /// Returns the vector of all registered [`NamedOutput`] values in registration order.
    pub fn evaluate(&self, inputs: &[f64], params: &[f64]) -> Vec<f64> {
        let vals = self.evaluate_all_nodes(inputs, params);
        self.outputs.iter().map(|o| vals[o.expr.0]).collect()
    }

    /// Evaluates every SSA node and returns the full value table indexed by [`ExprId`].
    pub fn evaluate_all_nodes(&self, inputs: &[f64], params: &[f64]) -> Vec<f64> {
        let mut vals = vec![0.0_f64; self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            vals[i] = match node {
                IrNode::Const(c) => *c,
                IrNode::Input { index, .. } => inputs.get(*index).copied().unwrap_or(0.0),
                IrNode::Param {
                    index, default_val, ..
                } => params.get(*index).copied().unwrap_or(*default_val),
                IrNode::Unary { op, arg } => {
                    let x = vals[arg.0];
                    match op {
                        UnaryOp::Neg => -x,
                        UnaryOp::Exp => x.clamp(-80.0, 80.0).exp(),
                        UnaryOp::Ln => x.max(1e-30).ln(),
                        UnaryOp::Sqrt => x.max(0.0).sqrt(),
                        UnaryOp::Sin => x.sin(),
                        UnaryOp::Cos => x.cos(),
                        UnaryOp::Tanh => x.tanh(),
                        UnaryOp::Abs => x.abs(),
                        UnaryOp::Recip => 1.0 / x,
                    }
                }
                IrNode::Binary { op, lhs, rhs } => {
                    let a = vals[lhs.0];
                    let b = vals[rhs.0];
                    match op {
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
                    }
                }
                IrNode::Select {
                    cond,
                    if_true,
                    if_false,
                } => {
                    if vals[cond.0] > 0.0 {
                        vals[if_true.0]
                    } else {
                        vals[if_false.0]
                    }
                }
                IrNode::Fma { a, b, c } => vals[a.0].mul_add(vals[b.0], vals[c.0]),
            };
        }
        vals
    }
}
