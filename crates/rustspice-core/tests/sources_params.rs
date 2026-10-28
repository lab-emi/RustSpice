//! Source waveforms, the title line, `.param` expressions and subcircuit parameters (reader level).
//! Expected values are closed forms of the SPICE definitions, not reader output.

use rustspice_core::netlist::{parse_netlist, Netlist, SourceWaveform};

fn parse(src: &str) -> Netlist {
    parse_netlist(src).unwrap_or_else(|e| panic!("{e}\n{src}"))
}

fn codes(n: &Netlist) -> Vec<String> {
    n.diagnostics.iter().map(|d| d.code.clone()).collect()
}

fn resistance(n: &Netlist, name: &str) -> f64 {
    n.resistors
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no {name}"))
        .resistance
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * b.abs().max(1e-30)
}

#[test]
fn line_one_is_always_the_title() {
    let n = parse(".subckt looks_like_a_card a b\nV1 1 0 5\nR1 1 2 1k\nR2 2 0 1k\n.op\n.end\n");
    assert_eq!(n.title, ".subckt looks_like_a_card a b");
    assert_eq!(n.resistors.len(), 2);
    let n = parse("\nV1 in 0 DC 8\nR1 in out 3k\nR2 out 0 1k\n.op\n.end\n");
    assert_eq!(n.v_sources.len(), 1);
    assert_eq!(n.v_sources[0].waveform.dc_value(), 8.0);
}

#[test]
fn waveform_is_chosen_by_keyword_not_element_name() {
    let n = parse("t\nVpulse1 a 0 DC 1.5\nIsin2 0 b 2m\nR1 a 0 1k\nR2 b 0 1k\n.op\n");
    assert_eq!(n.v_sources[0].waveform, SourceWaveform::Dc(1.5));
    assert_eq!(n.i_sources[0].waveform, SourceWaveform::Dc(2e-3));
}

#[test]
fn explicit_dc_value_is_kept_next_to_a_waveform() {
    let n = parse("t\nV1 a 0 DC 2 SIN(1 1 1meg)\nV2 b 0 SIN(1 1 1meg)\nR1 a 0 1\nR2 b 0 1\n.op\n");
    assert_eq!(n.v_sources[0].waveform.dc_value(), 2.0);
    assert_eq!(n.v_sources[0].waveform.evaluate_at(0.0), 1.0);
    assert_eq!(n.v_sources[1].waveform.dc_value(), 1.0);
    assert!(!codes(&n).iter().any(|c| c == "dc-value-dropped"));
}

#[test]
fn pwl_exp_sffm_and_sin_phase() {
    let n = parse(
        "t\nV1 a 0 PWL(0 0 1u 1 2u 0) r=0\nV2 b 0 EXP(0 1 1u 300n 2u 500n)\n\
         V3 c 0 SFFM(0 1 5meg 3 500k)\nV4 d 0 SIN(0 1 2meg 0 0 90)\nV5 e 0 PULSE 0 5 1u 100n 100n 1u 3u\n\
         I1 0 f PWL(0 3m 1u 0 2u 5m)\nV6 g 0 PULSE(0 5 1n 1n 1n 5n 10n\n\
         R1 a 0 1\nR2 b 0 1\nR3 c 0 1\nR4 d 0 1\nR5 e 0 1\nR6 f 0 1\nR7 g 0 1\n.tran 10n 5u\n",
    );
    let w = |k: usize| n.v_sources[k].waveform.clone();
    // PWL repeating from t = 0 with period 2 us: triangle
    assert!(close(w(0).evaluate_at(0.5e-6), 0.5));
    assert!(close(w(0).evaluate_at(2.5e-6), 0.5));
    assert!(close(w(0).evaluate_at(3.25e-6), 0.75));
    assert_eq!(w(0).next_breakpoint(2.1e-6), Some(3e-6));
    // EXP
    let tau = |t: f64| {
        let mut v = 0.0;
        if t > 1e-6 {
            v += 1.0 - (-(t - 1e-6) / 300e-9_f64).exp();
        }
        if t > 2e-6 {
            v -= 1.0 - (-(t - 2e-6) / 500e-9_f64).exp();
        }
        v
    };
    for t in [0.5e-6, 1.3e-6, 2.0e-6, 3.7e-6] {
        assert!((w(1).evaluate_at(t) - tau(t)).abs() < 1e-15);
    }
    assert_eq!(w(1).dc_value(), 0.0);
    // SFFM
    let pi2 = 2.0 * std::f64::consts::PI;
    let t = 1.234e-6_f64;
    let want = (pi2 * 5e6 * t + 3.0 * (pi2 * 5e5 * t).sin()).sin();
    assert!((w(2).evaluate_at(t) - want).abs() < 1e-12);
    assert_eq!(w(2).dc_value(), 0.0);
    // SIN with phase 90: a cosine, and the .op value is vo + va sin(phase)
    assert!((w(3).evaluate_at(0.1e-6) - (pi2 * 2e6 * 0.1e-6).cos()).abs() < 1e-12);
    assert!((w(3).dc_value() - 1.0).abs() < 1e-15);
    // PULSE without parentheses
    assert!(matches!(w(4), SourceWaveform::Pulse { v2, per, .. } if v2 == 5.0 && per == 3e-6));
    // PWL current source at .op: first value
    assert_eq!(n.i_sources[0].waveform.dc_value(), 3e-3);
    // unclosed PULSE( is read to the end of the card, with a diagnostic
    assert!(matches!(w(5), SourceWaveform::Pulse { v2, per, .. } if v2 == 5.0 && per == 10e-9));
    assert!(codes(&n).iter().any(|c| c == "malformed-input"));
}

#[test]
fn defaults_resolve_from_the_tran_card() {
    let n = parse("t\nV1 a 0 PULSE(0 1 1u)\nV2 b 0 SIN(0 1)\nR1 a 0 1\nR2 b 0 1\n.tran 0.1u 5u\n");
    let mut p = n.v_sources[0].waveform.clone();
    p.resolve_defaults(0.1e-6, 5e-6);
    // tr = tstep, pw = per = tstop
    assert!(close(p.evaluate_at(1.05e-6), 0.5));
    assert_eq!(p.evaluate_at(4e-6), 1.0);
    let mut s = n.v_sources[1].waveform.clone();
    s.resolve_defaults(0.1e-6, 5e-6);
    assert!((s.evaluate_at(1.25e-6) - 1.0).abs() < 1e-12);
}

#[test]
fn param_expressions_chains_and_functions() {
    let n = parse(
        "t\n.param rb={ra*2} ra = 1.5k\n.param k='sqrt(4)'\n.func dbl(x) {2*x}\n\
         R1 a 0 {rb}\nR2 a 0 '1k*k'\nR3 a 0 {dbl(ra)+ max(1, 2)}\nR4 a 0 { ra > 1k ? 10 : 20 }\nV1 a 0 1\n.op\n",
    );
    assert_eq!(resistance(&n, "R1"), 3e3);
    assert_eq!(resistance(&n, "R2"), 2e3);
    assert_eq!(resistance(&n, "R3"), 3002.0);
    assert_eq!(resistance(&n, "R4"), 10.0);
}

#[test]
fn subcircuit_params_defaults_and_overrides() {
    let n = parse(
        "t\n.param top=2\n.subckt div in out PARAMS: r1=1k r2={r1*top}\nRa in out {r1}\nRb out 0 {r2}\n.ends\n\
         .subckt noport\nRz z 0 5\n.ends\n\
         X1 a b div\nX2 a c div PARAMS: r1=3k\nX3 a d div r2 = 7k\nXn noport\nV1 a 0 1\n.op\n",
    );
    assert_eq!(resistance(&n, "Ra_X1"), 1e3);
    assert_eq!(resistance(&n, "Rb_X1"), 2e3);
    assert_eq!(resistance(&n, "Ra_X2"), 3e3);
    assert_eq!(resistance(&n, "Rb_X2"), 6e3);
    assert_eq!(resistance(&n, "Rb_X3"), 7e3);
    assert_eq!(resistance(&n, "Rz_Xn"), 5.0);
}

#[test]
fn bad_expressions_are_loud() {
    assert!(parse_netlist("t\nR1 a 0 {nope*2}\nV1 a 0 1\n.op\n").is_err());
    let e = parse_netlist("t\n.param a={b+1} b={a+1}\nV1 x 0 1\nR1 x 0 1\n.op\n").unwrap_err();
    assert!(e.to_string().contains("cycle"), "{e}");
}

#[test]
fn global_nodes_nodesets_gmin_and_models_inside_subcircuits() {
    let n = parse(
        "t\n.global vdd\nV1 vdd 0 3\nX1 out DRV\n.subckt DRV o\n.model DS D(IS=1e-12)\nR1 vdd o 1k\nD1 o 0 DS\n.ends\n\
         .nodeset v(out)=0.6\n.options gmin=1e-9\n.op\n",
    );
    // `.global vdd`: the subcircuit resistor hangs on the top-level vdd, not on a local `X1.vdd`
    let vdd = n.node_map["vdd"];
    assert!(n.resistors.iter().any(|r| r.n_pos == vdd || r.n_neg == vdd));
    assert!(
        !n.node_map.keys().any(|k| k.contains(".vdd")),
        "{:?}",
        n.node_map.keys()
    );
    // `.model` inside the body is defined (circuit-wide)
    assert!(
        n.diode_models.contains_key("ds"),
        "{:?}",
        n.diode_models.keys()
    );
    assert_eq!(n.nodesets.get(&n.node_map["out"]), Some(&0.6));
    assert_eq!(n.gmin, Some(1e-9));
    assert!(n.diagnostics.is_empty(), "{:?}", n.diagnostics);
}

#[test]
fn nested_dc_and_temperature_reevaluation() {
    let mut n = parse(
        "t\nV1 a 0 1\nV2 b 0 1\nR1 a 0 1k tc1=0.01\nR2 b 0 1k\n.dc V1 0 2 1 V2 0 1 0.5\n.end\n",
    );
    match &n.analyses[0] {
        rustspice_core::AnalysisCmd::Dc {
            source_name, outer, ..
        } => {
            assert_eq!(source_name, "V1");
            let o = outer.as_ref().expect("second sweep");
            assert_eq!(
                (o.name.as_str(), o.start, o.stop, o.step),
                ("V2", 0.0, 1.0, 0.5)
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(parse_netlist("t\nV1 a 0 1\nR1 a 0 1\n.dc V1 0 1 1 V2\n").is_err());
    // R(T) = R(Tnom) (1 + tc1 (T - Tnom)), re-evaluated from the nominal value at each temperature
    n.set_temperature(127.0).unwrap();
    assert!(close(resistance(&n, "R1"), 2e3));
    n.set_temperature(27.0).unwrap();
    assert!(close(resistance(&n, "R1"), 1e3));
    assert_eq!(resistance(&n, "R2"), 1e3);
}

#[test]
fn instance_multipliers_scale_the_device() {
    let n = parse(
        "t\nV1 a 0 1\nR1 a 0 1k m=4\nC1 a 0 1u m=2\nL1 a b 1u m=2\nR2 b 0 1\nG1 0 a a 0 2m m=3\n\
         D1 a 0 DM m=3\nD2 a 0 DM 2 m=3\nM1 a a 0 0 NM W=10u L=1u M=4\n.model DM D(IS=1e-14)\n\
         .model NM NMOS(LEVEL=1)\n.op\n",
    );
    assert_eq!(resistance(&n, "R1"), 250.0);
    assert_eq!(n.capacitors[0].capacitance, 2e-6);
    assert_eq!(n.inductors[0].inductance, 0.5e-6);
    assert_eq!(n.vccs[0].gm, 6e-3);
    assert_eq!(n.diodes[0].area, 3.0);
    assert_eq!(n.diodes[1].area, 6.0);
    assert_eq!(n.mosfets[0].m, 4.0);
    assert!(n.diagnostics.is_empty(), "{:?}", n.diagnostics);
}

#[test]
fn temperature_cards_and_linear_coefficients() {
    let n = parse(
        "t\nV1 a 0 1\nR1 a 0 1k tc1=0.01 tc2=1e-5 dtemp=50\nC1 a 0 1u tc1=0.01\nR2 a 0 1k tc1=0.01 temp=37\n\
         .temp 77\n.options tnom=27 reltol=1e-4\n.op\n",
    );
    assert_eq!(n.temp, Some(77.0));
    // dT is measured from tnom (27 C): instance 77 + 50 = 127 C
    assert!(close(
        resistance(&n, "R1"),
        1e3 * (1.0 + 0.01 * 100.0 + 1e-5 * 1e4)
    ));
    assert!(close(n.capacitors[0].capacitance, 1.5e-6));
    assert!(close(resistance(&n, "R2"), 1.1e3));
    assert!(!codes(&n).iter().any(|c| c == "ignored-temperature"));
    assert!(parse_netlist("t\nV1 a 0 1\nR1 a 0 1\n.temp -300\n.op\n").is_err());
    let n = parse("t\nV1 a 0 1\nR1 a 0 1\n.temp 0 27 85\n.op\n");
    assert_eq!(n.temp, None);
    assert!(codes(&n).iter().any(|c| c == "ignored-temperature"));
}

#[test]
fn diode_saturation_current_follows_temperature() {
    use rustspice_core::soa::compile_netlist;
    let is_at = |deck: &str| {
        let topo = compile_netlist(&parse(deck)).unwrap();
        (topo.soa.diode_is[0], topo.soa.diode_n[0] * topo.soa.temp_vt)
    };
    let base = "t\nV1 a 0 1\nD1 a 0 DM\n.model DM D(IS=1e-14)\n.op\n";
    assert_eq!(is_at(base), (1e-14, 0.025864917007155805));
    // closed form IS(T) = IS (T/Tn)^XTI exp((T/Tn - 1) EG / Vt(T)), 85 C
    let t = 358.15_f64;
    let vt = 0.025864917007155805 * t / 300.15;
    let want = 1e-14 * (t / 300.15).powi(3) * ((t / 300.15 - 1.0) * 1.11 / vt).exp();
    for deck in [
        "t\nV1 a 0 1\nD1 a 0 DM\n.model DM D(IS=1e-14)\n.temp 85\n.op\n",
        "t\nV1 a 0 1\nD1 a 0 DM temp=85\n.model DM D(IS=1e-14)\n.op\n",
        "t\nV1 a 0 1\nD1 a 0 DM dtemp=58\n.model DM D(IS=1e-14)\n.op\n",
        "t\n.options temp=85\nV1 a 0 1\nD1 a 0 DM\n.model DM D(IS=1e-14)\n.op\n",
    ] {
        let (is, nvt) = is_at(deck);
        assert!((is / want - 1.0).abs() < 1e-12, "{deck}: {is} vs {want}");
        assert!((nvt / vt - 1.0).abs() < 1e-14, "{deck}");
    }
    // TNOM = 50 C re-references IS down to 27 C
    let (is, _) = is_at("t\nV1 a 0 1\nD1 a 0 DM\n.model DM D(IS=1e-14 TNOM=50)\n.op\n");
    assert!(is < 1e-15 && is > 1e-16, "{is}");
}
