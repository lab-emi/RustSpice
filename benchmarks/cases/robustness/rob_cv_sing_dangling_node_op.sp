Resistor leading to a node with a single connection (dangling stub)
* bench.id: rob_cv_sing_dangling_node_op
* bench.group: robustness
* bench.title: Resistor leading to a node with a single connection (dangling stub)
* bench.probes: v(2), v(3)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: linear, dangling, op
* bench.note: Regression anchor: a single-connection node carries no current; v(3)=v(2)=2.5 V in both.
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R3 2 3 1k
.op
.end
