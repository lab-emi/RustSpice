V card without a value (defaults to 0 V DC), in series with the load
* bench.id: rob_hs_syn_v_missing_value
* bench.group: robustness
* bench.title: V card without a value (defaults to 0 V DC)
* bench.probes: v(1), v(2), v(3), i(v1), i(v9)
* bench.expect: match
* bench.tags: hostile, syntax, parser, lenient_ok
* bench.est_rss_mib: 64
* bench.note: V9 sits in series between R1 and R2, so a value-less V card must act as a 0 V source (v(2)=v(3)=2.5 V, i(v9)=+2.5 mA); a dropped card would leave node 2 at 5 V and node 3 at 0 V, an invented default would shift every node. Redesigned in calibration: the earlier V9/R9 branch was isolated and every probe of it was 0 either way
V1 1 0 5
R1 1 2 1k
V9 2 3
R2 3 0 1k
.op
.end
