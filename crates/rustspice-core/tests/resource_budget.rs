//! Hierarchy-expansion budgets and non-finite literal validation (AGENTS.md Rule 9).
//! Oracles are closed forms (card counts of a fan-out tree, the flattened-name grammar), not the parser.

use rustspice_core::{parse_netlist, parse_netlist_with_limits, ResourceLimits, SimError};

/// `levels`-deep fan-out-10 tree: `l0` holds one resistor, `lk` instantiates `l(k-1)` ten times.
fn fanout_deck(levels: usize) -> String {
    let mut s = String::from("fanout tree\n.subckt l0 a b\nR1 a b 1k\n.ends\n");
    for k in 1..=levels {
        s.push_str(&format!(".subckt l{k} a b\n"));
        for j in 0..10 {
            s.push_str(&format!("X{j} a b l{}\n", k - 1));
        }
        s.push_str(".ends\n");
    }
    s.push_str(&format!("V1 1 0 1\nXt 1 0 l{levels}\n.op\n.end\n"));
    s
}

/// Cards visited while flattening [`fanout_deck`]: 4 top-level cards (`V1`, `Xt`, `.op`, `.end`) plus the
/// bodies of 10^0 + ... instances: sum_{j=1}^{levels} 10^j X cards plus 10^levels resistor cards.
fn fanout_cards(levels: u32) -> usize {
    let x: usize = (1..=levels).map(|j| 10usize.pow(j)).sum();
    4 + x + 10usize.pow(levels)
}

fn chain_deck(depth: usize) -> String {
    let mut s = String::from("chain\n");
    for k in 0..depth {
        s.push_str(&format!(
            ".subckt s{k} a b\nX1 a m s{}\nR1 m b 1\n.ends\n",
            k + 1
        ));
    }
    s.push_str(&format!(".subckt s{depth} a b\nR1 a b 1\n.ends\n"));
    s.push_str("V1 1 0 1\nXt 1 0 s0\n.op\n.end\n");
    s
}

fn limit_resource(e: &SimError) -> &'static str {
    match e {
        SimError::ResourceLimit { resource, .. } => resource,
        other => panic!("expected ResourceLimit, got {other:?}"),
    }
}

#[test]
fn flattened_card_budget_boundary_matches_closed_form() {
    for levels in 1..=3u32 {
        let deck = fanout_deck(levels as usize);
        let need = fanout_cards(levels);
        let mut lim = ResourceLimits {
            max_expanded_cards: need,
            ..Default::default()
        };
        let nl = parse_netlist_with_limits(&deck, &lim).expect("exact budget must pass");
        assert_eq!(nl.resistors.len(), 10usize.pow(levels));
        lim.max_expanded_cards = need - 1;
        let e = parse_netlist_with_limits(&deck, &lim).expect_err("one card less must be refused");
        assert_eq!(limit_resource(&e), "expanded_cards");
    }
}

#[test]
fn expansion_bomb_is_refused_by_default_budget_and_stays_small() {
    // 10^9 resistors (the bench deck rob_hs_sub_expansion_bomb_1e9 has 9 levels); 7 levels already exceed 2e6
    let deck = fanout_deck(7);
    let e = parse_netlist(&deck).expect_err("1.1e7 cards must exceed the default budget");
    assert_eq!(limit_resource(&e), "expanded_cards");
}

#[test]
fn depth_budget_boundary() {
    let lim = ResourceLimits {
        max_subckt_depth: 20,
        ..Default::default()
    };
    // chain of N subckts instantiated from the top: instantiation depth of the innermost is N + 1
    let ok = parse_netlist_with_limits(&chain_deck(19), &lim).expect("depth 20 fits");
    assert_eq!(ok.resistors.len(), 20);
    let e = parse_netlist_with_limits(&chain_deck(20), &lim).expect_err("depth 21 must be refused");
    assert_eq!(limit_resource(&e), "subckt_depth");
}

#[test]
fn deep_chain_does_not_use_the_call_stack() {
    // 5000 levels with a raised budget: the former recursive expander overflowed the 8 MiB stack at ~15000
    // levels (and a 2 MiB test thread much earlier); the explicit stack has no such limit.
    let lim = ResourceLimits {
        max_subckt_depth: 5000,
        ..Default::default()
    };
    let nl = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            parse_netlist_with_limits(&chain_deck(4000), &lim).map(|n| n.resistors.len())
        })
        .unwrap()
        .join()
        .expect("no stack overflow on a 256 KiB stack");
    assert_eq!(nl.unwrap(), 4001);
}

#[test]
fn flattened_text_budget() {
    let lim = ResourceLimits {
        max_expanded_bytes: 500,
        ..Default::default()
    };
    let e = parse_netlist_with_limits(&fanout_deck(2), &lim)
        .expect_err("100 resistors exceed 500 bytes");
    assert_eq!(limit_resource(&e), "expanded_bytes");
}

#[test]
fn direct_and_mutual_recursion_are_refused_with_the_cycle() {
    let direct =
        "t\n.subckt loop a b\nR1 a b 1k\nX1 a b loop\n.ends\nV1 1 0 1\nX9 1 0 loop\n.op\n.end\n";
    match parse_netlist(direct).expect_err("direct recursion") {
        SimError::ParseError { message, .. } => {
            assert!(message.contains("loop -> loop"), "{message}")
        }
        e => panic!("{e:?}"),
    }
    let mutual = "t\n.subckt ping a b\nX1 a b pong\n.ends\n.subckt pong a b\nX1 a b ping\n.ends\nV1 1 0 1\nX9 1 0 ping\n.op\n.end\n";
    match parse_netlist(mutual).expect_err("mutual recursion") {
        SimError::ParseError { message, .. } => {
            assert!(message.contains("ping -> pong -> ping"), "{message}")
        }
        e => panic!("{e:?}"),
    }
}

#[test]
fn unreferenced_recursive_definition_is_accepted() {
    let deck =
        "t\n.subckt loop a b\nR1 a b 1k\nX1 a b loop\n.ends\nV1 1 0 1\nR2 1 0 1k\n.op\n.end\n";
    assert_eq!(parse_netlist(deck).unwrap().resistors.len(), 1);
}

#[test]
fn same_subckt_twice_in_sequence_is_not_a_cycle() {
    // diamond: top instantiates `leaf` directly twice and through `mid` once; no cycle
    let deck =
        "t\n.subckt leaf a b\nR1 a b 1k\n.ends\n.subckt mid a b\nX1 a b leaf\nX2 a b leaf\n.ends\n\
                V1 1 0 1\nXa 1 0 leaf\nXb 1 0 leaf\nXc 1 0 mid\n.op\n.end\n";
    let nl = parse_netlist(deck).unwrap();
    let names: Vec<&str> = nl.resistors.iter().map(|r| r.name.as_str()).collect();
    // depth-first, definition order, `<card>_<instance path>` (the grammar of rewrite_subckt_line)
    assert_eq!(names, ["R1_Xa", "R1_Xb", "R1_Xc.X1", "R1_Xc.X2"]);
}

#[test]
fn flattened_names_and_node_mapping_are_hierarchical() {
    let deck = "t\n.subckt inner a b\nR1 a m 1k\nR2 m b 2k\n.ends\n.subckt outer p q\nX1 p n1 inner\nR3 n1 q 3k\n.ends\n\
                V1 in 0 1\nXo in out outer\nR4 out 0 4k\n.op\n.end\n";
    let nl = parse_netlist(deck).unwrap();
    let r: Vec<(&str, &str, &str)> = nl
        .resistors
        .iter()
        .map(|r| {
            (
                r.name.as_str(),
                nl.node_names[r.n_pos].as_str(),
                nl.node_names[r.n_neg].as_str(),
            )
        })
        .collect();
    assert_eq!(
        r,
        [
            ("R1_Xo.X1", "in", "Xo.X1.m"),
            ("R2_Xo.X1", "Xo.X1.m", "Xo.n1"),
            ("R3_Xo", "Xo.n1", "out"),
            ("R4", "out", "0"),
        ]
    );
}

#[test]
fn non_finite_source_and_analysis_literals_are_rejected() {
    for (deck, what) in [
        ("t\nV1 1 0 1e999\nR1 1 0 1k\n.op\n.end\n", "dc value"),
        (
            "t\nV1 1 0 SIN(0 1 1e999)\nR1 1 0 1k\n.tran 1n 1u\n.end\n",
            "SIN frequency",
        ),
        (
            "t\nV1 1 0 SIN(0 1e999 1k)\nR1 1 0 1k\n.tran 1n 1u\n.end\n",
            "SIN va",
        ),
        (
            "t\nV1 1 0 PULSE(0 1e999 0 1n 1n 1 2)\nR1 1 0 1k\n.tran 1n 1u\n.end\n",
            "PULSE v2",
        ),
        ("t\nV1 1 0 1\nR1 1 0 1k\n.tran 1n 1e999\n.end\n", "tstop"),
        ("t\nV1 1 0 1\nR1 1 0 1k\n.dc V1 0 1e999 1\n.end\n", "stop"),
    ] {
        match parse_netlist(deck) {
            Err(SimError::TopologyError(m)) => assert!(m.contains(what), "{what}: {m}"),
            other => panic!("{what}: {other:?}"),
        }
    }
}

#[test]
fn infinite_device_values_and_timing_fields_stay_accepted() {
    // R = 1e999 is an open circuit in both tools (bench deck rob_hs_val_r_1e999 passes today): not rejected
    assert!(parse_netlist("t\nV1 1 0 5\nR1 1 2 1k\nR2 2 0 1e999\n.op\n.end\n").is_ok());
    // infinite pulse width / period is a one-shot pulse
    assert!(parse_netlist(
        "t\nV1 1 0 PULSE(0 1 0 1n 1n 1e999 1e999)\nR1 1 0 1k\n.tran 1n 1u\n.end\n"
    )
    .is_ok());
    // finite extremes are accepted (the solver, not the parser, judges overflow)
    assert!(parse_netlist("t\nV1 1 0 1e308\nR1 1 0 1e-3\n.op\n.end\n").is_ok());
}
