#!/usr/bin/env python3
"""Generator of the transient relation decks acc_rel_tran_*.

Usage: gen_tran_decks.py <outdir>

tran_superposition_rcladder30  twin-block: three copies of one RC ladder driven by u1+u2 / u1 / u2; the relation node
                               r_x(t) = 1 + K*(v_f - v_a - v_b)(x, t) must stay at 1 V (linearity of the time stepper).
tran_rest_equals_op_cmos5      three DC-driven 5-stage CMOS chains; the transient must stay at the operating point
                               (ngspice's flat waveform is the reference, the swing floor makes it a gate on |v - op|).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

PFX = "acc_rel_"


def emit(path, title, meta, els, tail):
    lines = [title] + [f"* bench.{k}: {v}" for k, v in meta] + els + list(tail)
    open(path, "w").write("\n".join(lines) + "\n")


def ladder_superposition(nst=30, R=100, C="100f", gain=1.0, probes=(10, 30)):
    src = {
        "f": ("PULSE(0 1 0.2n 0.1n 0.1n 1.5n 3n)", "SIN(0 0.5 1.3G)"),
        "a": ("PULSE(0 1 0.2n 0.1n 0.1n 1.5n 3n)", "DC 0"),
        "b": ("DC 0", "SIN(0 0.5 1.3G)"),
    }
    els = []
    for t, (s1, s2) in src.items():
        els.append(f"v{t}1 {t}a 0 {s1}")
        els.append(f"v{t}2 {t}0 {t}a {s2}")  # series stack: v({t}0) = u1 + u2
        prev = f"{t}0"
        for k in range(1, nst + 1):
            els.append(f"r{t}{k} {prev} {t}{k} {R}")
            els.append(f"c{t}{k} {t}{k} 0 {C}")
            prev = f"{t}{k}"
    els.append("vref ref1 0 DC 1")
    rels = []
    for x in probes:
        name = f"r_{x}"
        rels.append(name)
        els.append(f"rm{x} {name} ref1 1")
        els.append(f"gf{x} 0 {name} f{x} 0 {gain:g}")
        els.append(f"ga{x} {name} 0 a{x} 0 {gain:g}")
        els.append(f"gb{x} {name} 0 b{x} 0 {gain:g}")
    return els, rels


def cmos_chain(tag, vin, nst=5):
    els = [f"vin{tag} {tag}0 0 DC {vin}"]
    prev = f"{tag}0"
    for k in range(1, nst + 1):
        n = f"{tag}{k}"
        els += [
            f"mp{tag}{k} {n} {prev} vdd vdd PMOD W=20u L=1u",
            f"mn{tag}{k} {n} {prev} 0 0 NMOD W=10u L=1u",
            f"cl{tag}{k} {n} 0 20f",
        ]
        prev = n
    return els


def build_all(out):
    os.makedirs(out, exist_ok=True)
    # ------------------------------------------------------------------ tran superposition
    els, rels = ladder_superposition()
    probes = ", ".join(f"v({r})" for r in rels) + ", v(f10), v(f30)"
    emit(
        f"{out}/{PFX}tran_superposition_rcladder30.sp",
        "Transient superposition on three 30-stage RC ladders: PULSE+SIN versus PULSE alone plus SIN alone",
        [
            ("id", PFX + "tran_superposition_rcladder30"),
            ("group", "accuracy"),
            ("title", "Transient superposition: v(u1+u2) - v(u1) - v(u2) = 0 on 30-stage RC ladders (R 100, C 100 fF), 3 copies, relation nodes at stages 10 and 30"),
            ("probes", probes),
            ("tol.norm", "5e-3"),
            ("floor", "5e-11"),
            ("expect", "match"),
            ("size", "107"),
            ("tags", "linear, tran, rc_ladder, relation, twin_block, reference_free, superposition"),
            ("est_rss_mib", "16"),
            ("note", 'Copy f is driven by u1 + u2 (PULSE(0 1 0.2n 0.1n 0.1n 1.5n 3n) in series with SIN(0 0.5 1.3G)), copy a by u1 alone, copy b by u2 alone; r_x(t) = 1 + (v_f - v_a - v_b)(x, t) at stages 10 and 30 through non-loading VCCS cards, exactly 1 for a linear time-invariant system. RustSpice (fixed step, UnrolledSymbolic, 107 unknowns, 501 rows): max |r-1| = 1.6e-15 (stage 10) / 1.8e-15 (stage 30); ngspice reference (10029 rows, tmax = tstep/20) 3.6e-15 / 3.3e-15. The relation nodes sit at 1 V (non-vacuous); their gate is tol.norm 5e-3 x floor 5e-11 = 2.5e-13 V absolute (140x above RustSpice). The waveform probes v(f10), v(f30) are normalised by their own swing (0.706 V, 0.402 V): RustSpice vs ngspice reference 2.6e-4 / 9.0e-5 of swing at tstep 10 ps (tol.norm 5e-3), so a solution driven to zero cannot satisfy the relation.'),
        ],
        els,
        (".tran 10p 5n", ".end"),
    )

    # ------------------------------------------------------------------ tran rest == op
    els = ["vdd vdd 0 DC 3.3"]
    for t, v in (("a", 1.5), ("b", 1.65), ("c", 1.8)):
        els += cmos_chain(t, v)
    els += [
        ".model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)",
        ".model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)",
    ]
    probes = ["a1", "a3", "a5", "b1", "b2", "b3", "c1", "c2", "c4"]
    emit(
        f"{out}/{PFX}tran_rest_equals_op_cmos5.sp",
        "Three DC-driven 5-stage CMOS inverter chains (Vin 1.5 V, 1.65 V, 1.8 V): the transient must stay at the operating point",
        [
            ("id", PFX + "tran_rest_equals_op_cmos5"),
            ("group", "accuracy"),
            ("title", "Rest state: .tran of DC-driven CMOS chains stays at the .op values (Vin 1.5 V / 1.65 V trip point / 1.8 V, 20 fF per stage)"),
            ("probes", ", ".join(f"v({p})" for p in probes)),
            ("tol.norm", "1"),
            ("floor", "1e-8"),
            ("expect", "match"),
            ("size", "24"),
            ("tags", "nonlinear, tran, mos, cmos, rest_state, reference_free"),
            ("est_rss_mib", "16"),
            ("note", "Chains a, b, c are DC-driven at 1.5 V, 1.65 V (trip point of the symmetric inverter) and 1.8 V from a 3.3 V supply; the exact solution of .tran is the constant operating point. ngspice's flat waveform (tight reference) is the reference; floor 1e-8 V x tol.norm 1 makes the gate |v_rs(t) - v_ng(t)| <= 1e-8 V. Measured RustSpice deviation: <= 3.7e-12 V on a1, a3, a5, c1, c2, c4 and 2.8e-14 / 2.8e-12 / 2.5e-10 V on b1 / b2 / b3 (gate margin 41x on b3). Not probed: a2, a4, c3, c5 sit at ~1e-9 V (vacuous) and b4, b5, because the stage gain is ~100 at the trip point: the RustSpice operating point differs from tight ngspice by 9.8e-15, 1.0e-12, 1.1e-10, 1.2e-8, 1.3e-6 V on b1..b5 (Newton converged to 1e-6 relative) and the transient inherits it. The reference-free statement over all 18 nodes (tran vs RustSpice's own .op, max 3.8e-7 V at b5, gate reltol 1e-6) is checks/reference_free_relations/check_tran_rest_equals_op.py."),
        ],
        els,
        (".tran 10p 2n", ".end"),
    )


if __name__ == "__main__":
    build_all(sys.argv[1])
