R value {((...1k...))} with 20 nested parentheses
* bench.id: rob_hs_expr_parens_depth_20
* bench.group: robustness
* bench.title: R value with 20 nested parentheses inside braces
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: RustSpice supports no parenthesised value expression at any depth (even {(1k)}) and refuses the card with 'Invalid resistance value'; ngspice-47 evaluates the expression (R9 = 1k, v(2)=1.667 V). The plain {1k} form gives identical numbers in both tools
* bench.tags: hostile, param, parser, expr
* bench.est_rss_mib: 64
* bench.note: companion of rob_hs_expr_deep_parens: ngspice-47 computes this expression up to depth 100 and refuses it from depth 1000, so a depth of 20 is a valid, computable input
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R9 2 0 {((((((((((((((((((((1k))))))))))))))))))))}
.op
.end
