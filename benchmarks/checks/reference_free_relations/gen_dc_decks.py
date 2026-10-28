#!/usr/bin/env python3
"""Generator of the twin-block DC relation decks acc_rel_{reciprocity,superposition}_*.

The decks are committed as static .sp files in benchmarks/cases/accuracy; this script documents and reproduces how they were made
(its output is byte-identical to the committed decks).
Usage: gen_dc_decks.py <outdir>

Twin-block principle: the circuit under test is instantiated several times in ONE netlist (blocks a, b, c, ...).
A relation that must hold exactly (reciprocity, superposition) is evaluated INSIDE the
circuit with ideal VCCS cards (G elements draw no current from their control nodes) into a node

    r = 1 V + K * (relation expression in volts)

so the probe `v(r_...)` is 1.000... when the relation holds.  The reference value is 1 (ngspice and the dense
oracle both return 1 + O(K * 1e-15)); the harness gate |v(r) - ref| <= tol.abs is therefore a gate on the relation
residual (K * residual <= tol.abs).  Probes that are 1 V by construction are non-vacuous; the real node voltages are
probed as well so that an all-zero solution cannot satisfy the relation.
"""
import itertools
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rfr_common import fmt, mesh_links, shuffled  # noqa: E402

PAD = 0.01
PFX = "acc_rel_"


def block(tag, n, links, order_seed=None):
    """One n x n mesh copy: link resistors + 4 corner pads (0.01 ohm) to ground."""
    els = [f"r{tag}{a} {tag}n{b} {tag}n{c} {fmt(r)}" for a, b, c, r in links]
    corners = ["0_0", f"0_{n-1}", f"{n-1}_0", f"{n-1}_{n-1}"]
    els += [f"r{tag}pad{k} {tag}n{p} 0 {PAD}" for k, p in enumerate(corners)]
    if order_seed is not None:
        els = shuffled(els, order_seed)
    return els


def reciprocity(n, K, seed, inj, gain, pad_copies=0):
    """K copies of the same mesh; copy k gets 1 A at node inj[k]; relation r_jk = 1 + gain*(v_j(s_k) - v_k(s_j))."""
    links = mesh_links(n, seed, 1e-2, 1e2)
    tags = "abcdefgh"[:K]
    els = []
    for k in range(K):
        els += block(tags[k], n, links)
        els.append(f"i{tags[k]} 0 {tags[k]}n{inj[k]} DC 1")
    for k in range(pad_copies):  # disconnected padding copies: only the matrix size (solver tier) changes
        t = f"z{k}"
        els += block(t, n, links)
        els.append(f"i{t} 0 {t}n{inj[0]} DC 1")
    els.append("vref ref1 0 DC 1")
    rels, transfers = [], []
    for j, k in itertools.combinations(range(K), 2):
        name = f"r_{tags[j]}{tags[k]}"
        rels.append(name)
        els.append(f"rm{tags[j]}{tags[k]} {name} ref1 1")
        els.append(f"gp{tags[j]}{tags[k]} 0 {name} {tags[j]}n{inj[k]} {tags[k]}n{inj[j]} {gain:g}")
        transfers += [f"{tags[j]}n{inj[k]}", f"{tags[k]}n{inj[j]}"]
    return els, rels, transfers


def superposition(n, seed, gain, probes, i3node, v1=1.0, v2=-0.7, i3=0.5):
    """Copies f (V1+V2+I3), a (V1), b (V2), c (I3); relation r_x = 1 + gain*(v_f - v_a - v_b - v_c)(x)."""
    links = mesh_links(n, seed, 1e-2, 1e2)
    cfg = {"f": (v1, v2, i3), "a": (v1, 0, 0), "b": (0, v2, 0), "c": (0, 0, i3)}
    corners = ["0_0", f"0_{n-1}", f"{n-1}_0", f"{n-1}_{n-1}"]
    els = []
    for tag, (a1, a2, a3) in cfg.items():
        e = [f"r{tag}{a} {tag}n{b} {tag}n{c} {fmt(r)}" for a, b, c, r in links]
        e.append(f"r{tag}p0 {tag}p1 {tag}n{corners[0]} {PAD}")
        e.append(f"r{tag}p1 {tag}n{corners[1]} 0 {PAD}")
        e.append(f"r{tag}p2 {tag}n{corners[2]} 0 {PAD}")
        e.append(f"r{tag}p3 {tag}p2 {tag}n{corners[3]} {PAD}")
        e.append(f"v{tag}1 {tag}p1 0 DC {a1}")
        e.append(f"v{tag}2 {tag}p2 0 DC {a2}")
        e.append(f"i{tag}3 0 {tag}n{i3node} DC {a3}")
        els += e
    els.append("vref ref1 0 DC 1")
    rels = []
    for x in probes:
        name = f"r_{x}"
        rels.append(name)
        els.append(f"rm{x} {name} ref1 1")
        els.append(f"gf{x} 0 {name} fn{x} 0 {gain:g}")
        for t in "abc":
            els.append(f"g{t}{x} {name} 0 {t}n{x} 0 {gain:g}")
    return els, rels


def emit(path, title, meta, els, tail=(".op", ".end")):
    lines = [title] + [f"* bench.{k}: {v}" for k, v in meta] + els + list(tail)
    open(path, "w").write("\n".join(lines) + "\n")


TAGS = "linear, op, mesh, relation, twin_block, ill_conditioned, reference_free"

# ------------------------------------------------------------------------------------------------ notes
# Measured numbers (RustSpice release build of the integration date, ngspice-47, python dense float64 oracle) are quoted in the notes;
# every number is reproduced by calibrate_dc_decks.py in this directory.
GAP_NOTES = {
    "reciprocity_randmesh22": (
        "the UnrolledSymbolic static-pivot LU is not symmetric on this natural-order 22x22 contrast-1e4 mesh: |r-1| = 3.3e-9, 1.5e-8, 1.1e-8 "
        "(relative asymmetry of the transfer voltage 5.7e-11..3.5e-10) against the gate 1e-9, while ngspice-tight gives <= 4.1e-12, the python dense oracle "
        "<= 2.7e-15 and RustSpice itself on the same relation netlist plus one disconnected padding copy (FaerSupernodal, deck "
        "acc_rel_reciprocity_randmesh22_faer) <= 4.9e-12"
    ),
}
NOTES = {
    "reciprocity_randmesh14": (
        "r_jk = 1 + 200*(v_j(s_k) - v_k(s_j)) built from non-loading VCCS cards, 1 A injected at s_k in copy k; the exact value of r is 1 (python dense "
        "oracle: |r-1| <= 3.3e-16, ngspice-tight <= 6.7e-13). tol.rel 0 / tol.abs 1e-9 makes the gate an asymmetry of <= 5e-12 V (<= 1e-10 of the smallest "
        "transfer voltage 0.052 V). RustSpice (UnrolledSymbolic) |r-1| <= 8.7e-11 (relative asymmetry <= 7.2e-12; separate single-mesh runs without the "
        "sensing network give 1.0e-12..2.7e-11), margin 11x. The 12 transfer voltages (0.052..0.447 V) are probed as well so that an all-zero solution "
        "cannot satisfy the relation. Natural card order."
    ),
    "reciprocity_randmesh22": (
        "Same construction and gate as reciprocity_randmesh14 (K=200, tol.abs 1e-9 = asymmetry <= 5e-12 V), 22x22 mesh, 3 copies, transfer voltages "
        "0.22-0.32 V. The transfer voltages themselves are wrong by only <= 5.1e-11 V (below the default node-voltage tolerance), so only the relation "
        "exposes the loss of symmetry. Natural card order (no shuffling needed). Control without the gap: reciprocity_randmesh22_faer."
    ),
    "reciprocity_randmesh22_faer": (
        "Tier control of reciprocity_randmesh22: byte-identical mesh copies, relation cards and gate plus ONE disconnected 22x22 padding copy (current "
        "source only) as the sole change; the matrix grows from 1457 to 1941 unknowns, RustSpice leaves UnrolledSymbolic (<= 500k symbolic LU ops) for "
        "FaerSupernodal and |r-1| drops from 1.5e-8 to <= 4.9e-12 (ngspice-tight <= 4.8e-12 here, python dense oracle <= 2.0e-15); relative asymmetry "
        "4e-14..1.1e-13. Regression anchor for the faer tier."
    ),
    "superposition_randmesh14": (
        "Copy f carries V1 = 1 V (corner pad), V2 = -0.7 V (opposite corner pad) and I3 = 0.5 A (interior node); copies a, b, c carry one source each "
        "(the others are 0 V / 0 A); r_x = 1 + (v_f - v_a - v_b - v_c)(x) with unit gain through non-loading VCCS cards, exact value 1 (python dense "
        "oracle |r-1| = 0, ngspice-tight <= 5.1e-15). Gate tol.abs 1e-10 V = 1e-10 of the 1 V full scale. RustSpice (UnrolledSymbolic) |r-1| <= 1.9e-12, "
        "margin 53x; the residual is circuit specific on this tier (the mesh with seed 11 gives ~2e-15). Probes also include v_f at the 4 nodes."
    ),
    "superposition_randmesh22_faer": (
        "Same relation as superposition_randmesh14 on a 22x22 mesh: 4 copies, 1958 unknowns, FaerSupernodal. Gate tol.abs 1e-12 V (full scale 1 V): "
        "RustSpice |r-1| <= 5.8e-15 (|rs - ngspice-tight| <= 1.5e-14), ngspice-tight <= 1.2e-14, python dense oracle 0, so the gate sits 68x above "
        "RustSpice and 80x above ngspice. Regression anchor for the faer tier."
    ),
}


def build_all(out):
    os.makedirs(out, exist_ok=True)
    # ---------------------------------------------------------------- reciprocity 14 (UnrolledSymbolic, passes)
    inj14 = ["3_4", "10_9", "0_5", "12_2"]
    els, rels, tr = reciprocity(14, 4, 7, inj14, 200)
    emit(
        f"{out}/{PFX}reciprocity_randmesh14.sp",
        "Reciprocity of a random 14x14 resistor mesh: four injection copies, six symmetry relations",
        [
            ("id", PFX + "reciprocity_randmesh14"),
            ("group", "accuracy"),
            ("title", "Reciprocity v_j(s_k) = v_k(s_j) of a random 14x14 mesh (R log-uniform 1e-2..1e2): 4 copies, 6 relation nodes"),
            ("probes", ", ".join(f"v({x})" for x in rels + tr)),
            ("tol.rel", "0"),
            ("tol.abs", "1e-9"),
            ("expect", "match"),
            ("oracle", "dc_linear"),
            ("size", "792"),
            ("tags", TAGS + ", reciprocity"),
            ("est_rss_mib", "16"),
            ("note", NOTES["reciprocity_randmesh14"]),
        ],
        els,
    )

    # ---------------------------------------------------------------- reciprocity 22 (UnrolledSymbolic, gap) + faer control
    inj22 = ["5_6", "14_13", "2_17"]
    for pad, suffix, size in ((0, "reciprocity_randmesh22", "1457"), (1, "reciprocity_randmesh22_faer", "1941")):
        els, rels, tr = reciprocity(22, 3, 7, inj22, 200, pad_copies=pad)
        meta = [
            ("id", PFX + suffix),
            ("group", "accuracy"),
            ("title", "Reciprocity v_j(s_k) = v_k(s_j) of a random 22x22 mesh (R log-uniform 1e-2..1e2): 3 copies, 3 relation nodes"
             + (", plus one disconnected padding copy that moves the solver to the faer tier" if pad else "")),
            ("probes", ", ".join(f"v({x})" for x in rels + tr)),
            ("tol.rel", "0"),
            ("tol.abs", "1e-9"),
            ("expect", "match"),
        ]
        if not pad:
            meta += [("known_gap", "linear-static-pivot-accuracy"), ("gap.note", GAP_NOTES[suffix])]
        meta += [
            ("oracle", "dc_linear"),
            ("size", size),
            ("tags", TAGS + ", reciprocity" + (", tier_control" if pad else "")),
            ("est_rss_mib", "32"),
            ("note", NOTES[suffix]),
        ]
        emit(f"{out}/{PFX}{suffix}.sp", "Reciprocity of a random 22x22 resistor mesh: three injection copies" + (" and a padding copy" if pad else ""), meta, els)

    # ---------------------------------------------------------------- superposition (14 unrolled, 22 faer)
    for n, probes, i3node, tol, suffix, size, rss in (
        (14, ["3_3", "7_11", "11_7", "12_2"], "7_4", "1e-10", "superposition_randmesh14", "806", "16"),
        (22, ["5_5", "11_19", "19_11", "20_2"], "11_7", "1e-12", "superposition_randmesh22_faer", "1958", "32"),
    ):
        els, rels = superposition(n, 7, 1.0, probes, i3node)
        emit(
            f"{out}/{PFX}{suffix}.sp",
            f"Superposition on a random {n}x{n} resistor mesh: V1 + V2 + I3 versus the three single-source solves",
            [
                ("id", PFX + suffix),
                ("group", "accuracy"),
                ("title", f"Superposition v(V1+V2+I3) = v(V1) + v(V2) + v(I3) on a random {n}x{n} mesh (R log-uniform 1e-2..1e2): 4 copies, {len(rels)} relation nodes"),
                ("probes", ", ".join([f"v({x})" for x in rels] + [f"v(fn{x})" for x in probes])),
                ("tol.rel", "0"),
                ("tol.abs", tol),
                ("expect", "match"),
                ("oracle", "dc_linear"),
                ("size", size),
                ("tags", TAGS + ", superposition"),
                ("est_rss_mib", rss),
                ("note", NOTES[suffix]),
            ],
            els,
        )


if __name__ == "__main__":
    build_all(sys.argv[1])
