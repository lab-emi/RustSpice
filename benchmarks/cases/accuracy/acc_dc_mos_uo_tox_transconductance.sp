NMOS Level 1 with KP derived from UO=600 and TOX=100 nm (no KP given): id = 0.33 mA at Vgs = 2 V, Vds = 1 V
* bench.id: acc_dc_mos_uo_tox_transconductance
* bench.group: accuracy
* bench.title: NMOS Level 1 with KP derived from UO=600 and TOX=100 nm (no KP given): id = 0.33 mA at Vgs = 2 V, Vds = 1 V
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: UO is read as KP (600 A/V^2) and TOX dropped: RS returns i(vds) = -9600 A (exit 0, silent) instead of -3.315e-4 A; ngspice computes KP = UO*Cox = 2.07189e-5 A/V^2
* bench.tags: mos, level1, op, model_param
* bench.note: tol.abs 1e-10 A. Closed form: KP = UO[cm2/Vs]*1e-4*3.9*eps0/TOX = 2.07189e-5 A/V^2 and id = KP*W/L*(Vov*Vds - Vds^2/2) = 3.31501e-4 A with CODATA-2018 eps0 (tight ngspice 3.315018e-4 A: its effective eps0 is 3.06e-6 above CODATA, so a RustSpice using the CODATA constant would land 1.0e-9 A away, outside tol 4.3e-10 A, and the deck can only flip to gap_closed with ngspice's constant). Flip: explicit kp=2.0718863e-5 in RustSpice gives -3.315018058e-4 A, equal to ngspice to 1e-14 A. Same root cause as cap_dev_mos_tox_only, cap_dev_mos_u0 and cap_dev_mos_uo_tox
Vgs g 0 2
Vds d 0 1
M1 d g 0 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 uo=600 tox=1e-7)
.op
.end
