CMOS inverter chain with 50 stages, pulse input, 2e4 steps
* bench.id: perf_med_cmos_chain_tran_50
* bench.group: performance
* bench.title: CMOS inverter chain with 50 stages, pulse input, 2e4 steps
* bench.probes: v(n1), v(n25), v(n50), i(vdd)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 1p
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 54
* bench.est_rss_mib: 25
* bench.tags: nonlinear, tran, cmos, chain, medium
* bench.note: 100 MOSFETs with 200 fF loads, 54 unknowns above the JIT size limit; the input edge ripples down the chain for most of the 100 ns run
.model nm NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.model pm PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
VDD vdd 0 3
VIN in0 0 PULSE(0 3 1n 1n 1n 8n 20n)
MP1 n1 in0 vdd vdd pm W=20u L=1u
MN1 n1 in0 0 0 nm W=10u L=1u
CL1 n1 0 200f
MP2 n2 n1 vdd vdd pm W=20u L=1u
MN2 n2 n1 0 0 nm W=10u L=1u
CL2 n2 0 200f
MP3 n3 n2 vdd vdd pm W=20u L=1u
MN3 n3 n2 0 0 nm W=10u L=1u
CL3 n3 0 200f
MP4 n4 n3 vdd vdd pm W=20u L=1u
MN4 n4 n3 0 0 nm W=10u L=1u
CL4 n4 0 200f
MP5 n5 n4 vdd vdd pm W=20u L=1u
MN5 n5 n4 0 0 nm W=10u L=1u
CL5 n5 0 200f
MP6 n6 n5 vdd vdd pm W=20u L=1u
MN6 n6 n5 0 0 nm W=10u L=1u
CL6 n6 0 200f
MP7 n7 n6 vdd vdd pm W=20u L=1u
MN7 n7 n6 0 0 nm W=10u L=1u
CL7 n7 0 200f
MP8 n8 n7 vdd vdd pm W=20u L=1u
MN8 n8 n7 0 0 nm W=10u L=1u
CL8 n8 0 200f
MP9 n9 n8 vdd vdd pm W=20u L=1u
MN9 n9 n8 0 0 nm W=10u L=1u
CL9 n9 0 200f
MP10 n10 n9 vdd vdd pm W=20u L=1u
MN10 n10 n9 0 0 nm W=10u L=1u
CL10 n10 0 200f
MP11 n11 n10 vdd vdd pm W=20u L=1u
MN11 n11 n10 0 0 nm W=10u L=1u
CL11 n11 0 200f
MP12 n12 n11 vdd vdd pm W=20u L=1u
MN12 n12 n11 0 0 nm W=10u L=1u
CL12 n12 0 200f
MP13 n13 n12 vdd vdd pm W=20u L=1u
MN13 n13 n12 0 0 nm W=10u L=1u
CL13 n13 0 200f
MP14 n14 n13 vdd vdd pm W=20u L=1u
MN14 n14 n13 0 0 nm W=10u L=1u
CL14 n14 0 200f
MP15 n15 n14 vdd vdd pm W=20u L=1u
MN15 n15 n14 0 0 nm W=10u L=1u
CL15 n15 0 200f
MP16 n16 n15 vdd vdd pm W=20u L=1u
MN16 n16 n15 0 0 nm W=10u L=1u
CL16 n16 0 200f
MP17 n17 n16 vdd vdd pm W=20u L=1u
MN17 n17 n16 0 0 nm W=10u L=1u
CL17 n17 0 200f
MP18 n18 n17 vdd vdd pm W=20u L=1u
MN18 n18 n17 0 0 nm W=10u L=1u
CL18 n18 0 200f
MP19 n19 n18 vdd vdd pm W=20u L=1u
MN19 n19 n18 0 0 nm W=10u L=1u
CL19 n19 0 200f
MP20 n20 n19 vdd vdd pm W=20u L=1u
MN20 n20 n19 0 0 nm W=10u L=1u
CL20 n20 0 200f
MP21 n21 n20 vdd vdd pm W=20u L=1u
MN21 n21 n20 0 0 nm W=10u L=1u
CL21 n21 0 200f
MP22 n22 n21 vdd vdd pm W=20u L=1u
MN22 n22 n21 0 0 nm W=10u L=1u
CL22 n22 0 200f
MP23 n23 n22 vdd vdd pm W=20u L=1u
MN23 n23 n22 0 0 nm W=10u L=1u
CL23 n23 0 200f
MP24 n24 n23 vdd vdd pm W=20u L=1u
MN24 n24 n23 0 0 nm W=10u L=1u
CL24 n24 0 200f
MP25 n25 n24 vdd vdd pm W=20u L=1u
MN25 n25 n24 0 0 nm W=10u L=1u
CL25 n25 0 200f
MP26 n26 n25 vdd vdd pm W=20u L=1u
MN26 n26 n25 0 0 nm W=10u L=1u
CL26 n26 0 200f
MP27 n27 n26 vdd vdd pm W=20u L=1u
MN27 n27 n26 0 0 nm W=10u L=1u
CL27 n27 0 200f
MP28 n28 n27 vdd vdd pm W=20u L=1u
MN28 n28 n27 0 0 nm W=10u L=1u
CL28 n28 0 200f
MP29 n29 n28 vdd vdd pm W=20u L=1u
MN29 n29 n28 0 0 nm W=10u L=1u
CL29 n29 0 200f
MP30 n30 n29 vdd vdd pm W=20u L=1u
MN30 n30 n29 0 0 nm W=10u L=1u
CL30 n30 0 200f
MP31 n31 n30 vdd vdd pm W=20u L=1u
MN31 n31 n30 0 0 nm W=10u L=1u
CL31 n31 0 200f
MP32 n32 n31 vdd vdd pm W=20u L=1u
MN32 n32 n31 0 0 nm W=10u L=1u
CL32 n32 0 200f
MP33 n33 n32 vdd vdd pm W=20u L=1u
MN33 n33 n32 0 0 nm W=10u L=1u
CL33 n33 0 200f
MP34 n34 n33 vdd vdd pm W=20u L=1u
MN34 n34 n33 0 0 nm W=10u L=1u
CL34 n34 0 200f
MP35 n35 n34 vdd vdd pm W=20u L=1u
MN35 n35 n34 0 0 nm W=10u L=1u
CL35 n35 0 200f
MP36 n36 n35 vdd vdd pm W=20u L=1u
MN36 n36 n35 0 0 nm W=10u L=1u
CL36 n36 0 200f
MP37 n37 n36 vdd vdd pm W=20u L=1u
MN37 n37 n36 0 0 nm W=10u L=1u
CL37 n37 0 200f
MP38 n38 n37 vdd vdd pm W=20u L=1u
MN38 n38 n37 0 0 nm W=10u L=1u
CL38 n38 0 200f
MP39 n39 n38 vdd vdd pm W=20u L=1u
MN39 n39 n38 0 0 nm W=10u L=1u
CL39 n39 0 200f
MP40 n40 n39 vdd vdd pm W=20u L=1u
MN40 n40 n39 0 0 nm W=10u L=1u
CL40 n40 0 200f
MP41 n41 n40 vdd vdd pm W=20u L=1u
MN41 n41 n40 0 0 nm W=10u L=1u
CL41 n41 0 200f
MP42 n42 n41 vdd vdd pm W=20u L=1u
MN42 n42 n41 0 0 nm W=10u L=1u
CL42 n42 0 200f
MP43 n43 n42 vdd vdd pm W=20u L=1u
MN43 n43 n42 0 0 nm W=10u L=1u
CL43 n43 0 200f
MP44 n44 n43 vdd vdd pm W=20u L=1u
MN44 n44 n43 0 0 nm W=10u L=1u
CL44 n44 0 200f
MP45 n45 n44 vdd vdd pm W=20u L=1u
MN45 n45 n44 0 0 nm W=10u L=1u
CL45 n45 0 200f
MP46 n46 n45 vdd vdd pm W=20u L=1u
MN46 n46 n45 0 0 nm W=10u L=1u
CL46 n46 0 200f
MP47 n47 n46 vdd vdd pm W=20u L=1u
MN47 n47 n46 0 0 nm W=10u L=1u
CL47 n47 0 200f
MP48 n48 n47 vdd vdd pm W=20u L=1u
MN48 n48 n47 0 0 nm W=10u L=1u
CL48 n48 0 200f
MP49 n49 n48 vdd vdd pm W=20u L=1u
MN49 n49 n48 0 0 nm W=10u L=1u
CL49 n49 0 200f
MP50 n50 n49 vdd vdd pm W=20u L=1u
MN50 n50 n49 0 0 nm W=10u L=1u
CL50 n50 0 200f
.tran 5p 100.0025n
.end
