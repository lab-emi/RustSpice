//! Procedural macro DSL (`#[spice_device]`) that parses pure Rust device physics equations,
//! lowers them into [`rustspice_ir::IrGraph`], performs compile-time symbolic Forward/Reverse
//! Automatic Differentiation and Common Subexpression Elimination (CSE), and emits inline
//! Rust Jacobian/Adjoint kernels alongside Apple Metal (`MSL`), WebGPU (`WGSL`), and
//! NVIDIA CUDA Tile compute shaders.

use std::collections::HashMap;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use rustspice_ir::{
    emit_cuda_tile_kernel, emit_msl_df64_kernel, emit_wgsl_df64_kernel, optimize_graph, BinaryOp,
    DiffTarget, ExprId, IrGraph, IrNode, UnaryOp,
};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    BinOp, Expr, FnArg, Ident, ItemFn, Lit, Pat, ReturnType, Stmt, Token, UnOp,
};

struct ParamSpec {
    name: Ident,
    default_val: f64,
}

#[derive(Default)]
struct DeviceMacroArgs {
    inputs: Vec<Ident>,
    params: Vec<ParamSpec>,
    outputs: Vec<Ident>,
}

impl Parse for DeviceMacroArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = DeviceMacroArgs::default();
        while !input.is_empty() {
            let section: Ident = input.parse()?;
            let content;
            syn::parenthesized!(content in input);
            if section == "inputs" {
                let list: Punctuated<Ident, Token![,]> =
                    content.parse_terminated(Ident::parse, Token![,])?;
                args.inputs.extend(list);
            } else if section == "params" {
                while !content.is_empty() {
                    let name: Ident = content.parse()?;
                    let mut default_val = 1.0_f64;
                    if content.peek(Token![=]) {
                        let _: Token![=] = content.parse()?;
                        let expr: Expr = content.parse()?;
                        default_val = eval_const_lit(&expr)?;
                    }
                    args.params.push(ParamSpec { name, default_val });
                    if content.peek(Token![,]) {
                        let _: Token![,] = content.parse()?;
                    }
                }
            } else if section == "outputs" {
                let list: Punctuated<Ident, Token![,]> =
                    content.parse_terminated(Ident::parse, Token![,])?;
                args.outputs.extend(list);
            } else {
                return Err(syn::Error::new(
                    section.span(),
                    "Expected `inputs(...)`, `params(...)`, or `outputs(...)`",
                ));
            }
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }
        Ok(args)
    }
}

fn eval_const_lit(expr: &Expr) -> syn::Result<f64> {
    match expr {
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::Float(f) => f.base10_parse::<f64>(),
            Lit::Int(i) => i.base10_parse::<f64>(),
            _ => Err(syn::Error::new_spanned(
                expr,
                "Expected numeric literal in parameter default",
            )),
        },
        Expr::Unary(u) if matches!(u.op, UnOp::Neg(_)) => Ok(-eval_const_lit(&u.expr)?),
        _ => Err(syn::Error::new_spanned(
            expr,
            "Expected numeric constant expression",
        )),
    }
}

fn lower_expr_to_ir(
    expr: &Expr,
    graph: &mut IrGraph,
    env: &HashMap<String, ExprId>,
) -> syn::Result<ExprId> {
    match expr {
        Expr::Lit(lit) => {
            let val = eval_const_lit(expr)?;
            let _ = lit;
            Ok(graph.const_val(val))
        }
        Expr::Path(path) => {
            if let Some(ident) = path.path.get_ident() {
                let key = ident.to_string();
                if let Some(&id) = env.get(&key) {
                    return Ok(id);
                }
                return Err(syn::Error::new_spanned(
                    ident,
                    format!("Unknown variable `{key}` in #[spice_device] body"),
                ));
            }
            Err(syn::Error::new_spanned(path, "Unsupported path expression"))
        }
        Expr::Paren(p) => lower_expr_to_ir(&p.expr, graph, env),
        Expr::Unary(u) => {
            if let UnOp::Neg(_) = u.op {
                if let Ok(c) = eval_const_lit(expr) {
                    return Ok(graph.const_val(c));
                }
                let inner = lower_expr_to_ir(&u.expr, graph, env)?;
                Ok(graph.neg(inner))
            } else {
                Err(syn::Error::new_spanned(u, "Unsupported unary operator"))
            }
        }
        Expr::Binary(b) => {
            let lhs = lower_expr_to_ir(&b.left, graph, env)?;
            let rhs = lower_expr_to_ir(&b.right, graph, env)?;
            match b.op {
                BinOp::Add(_) => Ok(graph.add(lhs, rhs)),
                BinOp::Sub(_) => Ok(graph.sub(lhs, rhs)),
                BinOp::Mul(_) => Ok(graph.mul(lhs, rhs)),
                BinOp::Div(_) => Ok(graph.div(lhs, rhs)),
                BinOp::Gt(_) => Ok(graph.gt(lhs, rhs)),
                BinOp::Lt(_) => Ok(graph.gt(rhs, lhs)),
                _ => Err(syn::Error::new_spanned(
                    b,
                    "Unsupported binary operator in #[spice_device]",
                )),
            }
        }
        Expr::MethodCall(mc) => {
            let recv = lower_expr_to_ir(&mc.receiver, graph, env)?;
            let method = mc.method.to_string();
            match method.as_str() {
                "exp" => Ok(graph.exp(recv)),
                "ln" => Ok(graph.ln(recv)),
                "sqrt" => Ok(graph.sqrt(recv)),
                "sin" => Ok(graph.sin(recv)),
                "cos" => Ok(graph.cos(recv)),
                "tanh" => Ok(graph.tanh(recv)),
                "abs" => Ok(graph.abs(recv)),
                "recip" => Ok(graph.recip(recv)),
                "max" if mc.args.len() == 1 => {
                    let arg = lower_expr_to_ir(&mc.args[0], graph, env)?;
                    Ok(graph.max(recv, arg))
                }
                "min" if mc.args.len() == 1 => {
                    let arg = lower_expr_to_ir(&mc.args[0], graph, env)?;
                    Ok(graph.min(recv, arg))
                }
                "powf" if mc.args.len() == 1 => {
                    let arg = lower_expr_to_ir(&mc.args[0], graph, env)?;
                    Ok(graph.pow(recv, arg))
                }
                "mul_add" if mc.args.len() == 2 => {
                    let a1 = lower_expr_to_ir(&mc.args[0], graph, env)?;
                    let a2 = lower_expr_to_ir(&mc.args[1], graph, env)?;
                    Ok(graph.fma(recv, a1, a2))
                }
                _ => Err(syn::Error::new_spanned(
                    &mc.method,
                    format!("Unsupported method `{method}` in #[spice_device]"),
                )),
            }
        }
        Expr::If(expr_if) => {
            let cond = lower_expr_to_ir(&expr_if.cond, graph, env)?;
            let then_expr = extract_block_tail(&expr_if.then_branch)?;
            let t_id = lower_expr_to_ir(then_expr, graph, env)?;
            let (_, else_box) = expr_if.else_branch.as_ref().ok_or_else(|| {
                syn::Error::new_spanned(
                    expr_if,
                    "`if` in #[spice_device] requires an `else` branch",
                )
            })?;
            let f_id = match else_box.as_ref() {
                Expr::Block(b) => lower_expr_to_ir(extract_block_tail(&b.block)?, graph, env)?,
                other => lower_expr_to_ir(other, graph, env)?,
            };
            Ok(graph.select(cond, t_id, f_id))
        }
        Expr::Block(b) => lower_expr_to_ir(extract_block_tail(&b.block)?, graph, env),
        _ => Err(syn::Error::new_spanned(
            expr,
            "Unsupported expression form in #[spice_device]",
        )),
    }
}

fn extract_block_tail(block: &syn::Block) -> syn::Result<&Expr> {
    if let Some(Stmt::Expr(e, None)) = block.stmts.last() {
        Ok(e)
    } else {
        Err(syn::Error::new_spanned(
            block,
            "Expected trailing value expression in block",
        ))
    }
}

/// Compiles a Rust semiconductor physics function into an auto-differentiated, CSE-optimized
/// multi-backend kernel suite.
#[proc_macro_attribute]
pub fn spice_device(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as DeviceMacroArgs);
    let item_fn = parse_macro_input!(item as ItemFn);

    match expand_spice_device(args, &item_fn) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_spice_device(
    mut args: DeviceMacroArgs,
    item_fn: &ItemFn,
) -> syn::Result<proc_macro2::TokenStream> {
    let fn_name = &item_fn.sig.ident;
    let device_name = fn_name.to_string();

    // If inputs/params were not explicitly listed, infer inputs from function arguments
    if args.inputs.is_empty() && args.params.is_empty() {
        for arg in &item_fn.sig.inputs {
            if let FnArg::Typed(pat_type) = arg {
                if let Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                    args.inputs.push(pat_ident.ident.clone());
                }
            }
        }
    }
    if args.outputs.is_empty() {
        args.outputs.push(Ident::new("out", Span::call_site()));
    }

    let mut graph = IrGraph::new(&device_name);
    let mut env: HashMap<String, ExprId> = HashMap::new();

    for inp in &args.inputs {
        let name = inp.to_string();
        let id = graph.input(&name);
        env.insert(name, id);
    }
    for param in &args.params {
        let name = param.name.to_string();
        let id = graph.param(&name, param.default_val);
        env.insert(name, id);
    }

    let mut last_expr_id = None;
    for stmt in &item_fn.block.stmts {
        match stmt {
            Stmt::Local(local) => {
                let pat_ident = match &local.pat {
                    Pat::Ident(pi) => &pi.ident,
                    Pat::Type(pt) => {
                        if let Pat::Ident(pi) = pt.pat.as_ref() {
                            &pi.ident
                        } else {
                            return Err(syn::Error::new_spanned(
                                &local.pat,
                                "Expected simple identifier binding in `let`",
                            ));
                        }
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &local.pat,
                            "Expected simple identifier binding in `let`",
                        ))
                    }
                };
                let init = local.init.as_ref().ok_or_else(|| {
                    syn::Error::new_spanned(local, "`let` binding must have an initializer")
                })?;
                let id = lower_expr_to_ir(&init.expr, &mut graph, &env)?;
                env.insert(pat_ident.to_string(), id);
                last_expr_id = Some(id);
            }
            Stmt::Expr(expr, _) => {
                let id = lower_expr_to_ir(expr, &mut graph, &env)?;
                last_expr_id = Some(id);
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    stmt,
                    "Unsupported statement in #[spice_device]",
                ))
            }
        }
    }

    // Register primary outputs
    let mut primary_outputs = Vec::new();
    for out_ident in &args.outputs {
        let out_name = out_ident.to_string();
        let expr_id = if let Some(&id) = env.get(&out_name) {
            id
        } else if let Some(id) = last_expr_id {
            id
        } else {
            return Err(syn::Error::new_spanned(
                &item_fn.block,
                "#[spice_device] function body produced no output expression",
            ));
        };
        graph.add_output(&out_name, expr_id);
        primary_outputs.push((out_name, expr_id));
    }

    // Automatically differentiate every primary output w.r.t. every input (Jacobian)
    // and every parameter (Adjoint parameter sensitivity)
    for (out_name, out_id) in &primary_outputs {
        for inp in &args.inputs {
            let inp_name = inp.to_string();
            let d_id = graph.differentiate_forward(*out_id, &DiffTarget::Input(inp_name.clone()));
            graph.add_output(format!("d_{out_name}_d_{inp_name}"), d_id);
        }
        for param in &args.params {
            let p_name = param.name.to_string();
            let d_id = graph.differentiate_forward(*out_id, &DiffTarget::Param(p_name.clone()));
            graph.add_output(format!("d_{out_name}_d_{p_name}"), d_id);
        }
    }

    // Run Constant Folding + Algebraic Simplification + CSE + DCE at compile time!
    optimize_graph(&mut graph);

    // Emit multi-backend shader source strings at compile time
    let msl_source = emit_msl_df64_kernel(&graph);
    let wgsl_source = emit_wgsl_df64_kernel(&graph);
    let cuda_tile_source = emit_cuda_tile_kernel(&graph, 128);

    // Emit inline CSE-optimized Rust statements for `<fn_name>_eval_all`
    let mut ssa_stmts = Vec::with_capacity(graph.nodes.len());
    for (idx, node) in graph.nodes.iter().enumerate() {
        let var = format_ident!("v{}", idx);
        let stmt = match node {
            IrNode::Const(c) => quote! { let #var: f64 = #c; },
            IrNode::Input { index, .. } => quote! { let #var: f64 = inputs[#index]; },
            IrNode::Param { index, .. } => quote! { let #var: f64 = params[#index]; },
            IrNode::Unary { op, arg } => {
                let a = format_ident!("v{}", arg.0);
                match op {
                    UnaryOp::Neg => quote! { let #var: f64 = -#a; },
                    UnaryOp::Exp => quote! { let #var: f64 = #a.clamp(-80.0, 80.0).exp(); },
                    UnaryOp::Ln => quote! { let #var: f64 = #a.max(1e-30).ln(); },
                    UnaryOp::Sqrt => quote! { let #var: f64 = #a.max(0.0).sqrt(); },
                    UnaryOp::Sin => quote! { let #var: f64 = #a.sin(); },
                    UnaryOp::Cos => quote! { let #var: f64 = #a.cos(); },
                    UnaryOp::Tanh => quote! { let #var: f64 = #a.tanh(); },
                    UnaryOp::Abs => quote! { let #var: f64 = #a.abs(); },
                    UnaryOp::Recip => quote! { let #var: f64 = 1.0 / #a; },
                }
            }
            IrNode::Binary { op, lhs, rhs } => {
                let l = format_ident!("v{}", lhs.0);
                let r = format_ident!("v{}", rhs.0);
                match op {
                    BinaryOp::Add => quote! { let #var: f64 = #l + #r; },
                    BinaryOp::Sub => quote! { let #var: f64 = #l - #r; },
                    BinaryOp::Mul => quote! { let #var: f64 = #l * #r; },
                    BinaryOp::Div => quote! { let #var: f64 = #l / #r; },
                    BinaryOp::Pow => quote! { let #var: f64 = #l.powf(#r); },
                    BinaryOp::Max => quote! { let #var: f64 = #l.max(#r); },
                    BinaryOp::Min => quote! { let #var: f64 = #l.min(#r); },
                    BinaryOp::Gt => quote! { let #var: f64 = if #l > #r { 1.0 } else { 0.0 }; },
                }
            }
            IrNode::Select {
                cond,
                if_true,
                if_false,
            } => {
                let c = format_ident!("v{}", cond.0);
                let t = format_ident!("v{}", if_true.0);
                let f = format_ident!("v{}", if_false.0);
                quote! { let #var: f64 = if #c > 0.0 { #t } else { #f }; }
            }
            IrNode::Fma { a, b, c } => {
                let va = format_ident!("v{}", a.0);
                let vb = format_ident!("v{}", b.0);
                let vc = format_ident!("v{}", c.0);
                quote! { let #var: f64 = #va.mul_add(#vb, #vc); }
            }
        };
        ssa_stmts.push(stmt);
    }

    let num_outputs = graph.outputs.len();
    let output_idents: Vec<_> = graph
        .outputs
        .iter()
        .map(|o| format_ident!("v{}", o.expr.0))
        .collect();
    let output_names: Vec<_> = graph.outputs.iter().map(|o| o.name.clone()).collect();

    let eval_fn_ident = format_ident!("{}_eval_with_derivatives", fn_name);
    let names_fn_ident = format_ident!("{}_output_names", fn_name);
    let msl_fn_ident = format_ident!("{}_msl_shader", fn_name);
    let wgsl_fn_ident = format_ident!("{}_wgsl_shader", fn_name);
    let cuda_fn_ident = format_ident!("{}_cuda_tile_shader", fn_name);

    let _ = ReturnType::Default;

    Ok(quote! {
        #item_fn

        /// Evaluates the CSE-optimized SSA device equations, Forward-Mode Jacobians,
        /// and Parameter Sensitivities generated at compile time by `#[spice_device]`.
        #[inline]
        pub fn #eval_fn_ident(inputs: &[f64], params: &[f64]) -> [f64; #num_outputs] {
            #(#ssa_stmts)*
            [#(#output_idents),*]
        }

        /// Returns the ordered slice of output names (`[primary_outputs..., jacobians..., param_sensitivities...]`).
        pub const fn #names_fn_ident() -> &'static [&'static str; #num_outputs] {
            &[#(#output_names),*]
        }

        /// Returns the compile-time synthesized Apple Metal (`MSL`) `df64` compute shader.
        pub const fn #msl_fn_ident() -> &'static str {
            #msl_source
        }

        /// Returns the compile-time synthesized WebGPU (`WGSL`) `df64` compute shader.
        pub const fn #wgsl_fn_ident() -> &'static str {
            #wgsl_source
        }

        /// Returns the compile-time synthesized NVIDIA CUDA Tile kernel source.
        pub const fn #cuda_fn_ident() -> &'static str {
            #cuda_tile_source
        }
    })
}
