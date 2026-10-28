Value suffix parsing: sixteen dividers, each with one suffixed resistor equal to its plain-number partner (every node must be 0.5 V)
* bench.id: acc_dc_lin_value_suffixes
* bench.group: accuracy
* bench.title: Value suffix parsing: sixteen dividers, each with one suffixed resistor equal to its plain-number partner (every node must be 0.5 V)
* bench.probes: v(n0), v(n1), v(n2), v(n3), v(n4), v(n5), v(n6), v(n7), v(n8), v(n9), v(n10), v(n11), v(n12), v(n13), v(n14), v(n15)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.tags: linear, op, parse, units
* bench.note: K, Meg/MEG, M (milli), mil, u, n, p, G, T, E-notation, trailing unit letters; every probe is 0.5 V if parsed correctly
V1 in 0 1
Ra0 in n0 1000
Rb0 n0 0 1K
Ra1 in n1 2200000
Rb1 n1 0 2.2Meg
Ra2 in n2 2200000
Rb2 n2 0 2.2MEG
Ra3 in n3 0.001
Rb3 n3 0 1M
Ra4 in n4 2.54e-05
Rb4 n4 0 1mil
Ra5 in n5 1e-06
Rb5 n5 0 1u
Ra6 in n6 1e-09
Rb6 n6 0 1n
Ra7 in n7 1000000000
Rb7 n7 0 1G
Ra8 in n8 1e+12
Rb8 n8 0 1T
Ra9 in n9 2500
Rb9 n9 0 2.5E3
Ra10 in n10 4700
Rb10 n10 0 4.7kohm
Ra11 in n11 3300
Rb11 n11 0 3.3k
Ra12 in n12 500
Rb12 n12 0 .5k
Ra13 in n13 1000
Rb13 n13 0 1e+3
Ra14 in n14 10000000
Rb14 n14 0 10MEGohm
Ra15 in n15 1e-10
Rb15 n15 0 100p
.op
.end
