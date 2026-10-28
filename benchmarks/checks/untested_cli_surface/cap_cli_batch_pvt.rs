// In-process harness for `Simulator::run_batch_dc_op` (API-only: the CLI has no batch entry).
// Install: copy to crates/rustspice-solver/examples/cap_cli_batch_pvt.rs, then
//   CARGO_BUILD_JOBS=2 cargo build -p rustspice-solver --example cap_cli_batch_pvt
// Usage: cap_cli_batch_pvt <deck.sp> <corners.txt>      corners.txt: one corner per line, `dev.param=value ...`
// Output: `nodes n1 n2 ...` then one `ok <iterations> <hex f64 bits of the MNA solution>...` line per corner,
//         or `ERR <message>` (exit 3) when the call fails (it is all-or-nothing: one bad corner fails the batch).
use rustspice_backend_cpu::BatchCircuitInput;
use rustspice_solver::Simulator;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = std::fs::read_to_string(&a[1]).expect("deck");
    let corners = std::fs::read_to_string(&a[2]).expect("corners");
    let sim = Simulator::from_spice(&src).expect("compile");
    let mut inputs = Vec::new();
    for line in corners.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut ov = Vec::new();
        for tok in line.split_whitespace() {
            let (k, v) = tok.split_once('=').expect("dev.param=value");
            let (d, p) = k.split_once('.').expect("dev.param");
            ov.push((d.to_string(), p.to_string(), v.parse::<f64>().expect("value")));
        }
        inputs.push(BatchCircuitInput { overrides: ov });
    }
    let names: Vec<String> = sim.netlist.node_names.iter().skip(1).map(|n| n.to_ascii_lowercase()).collect();
    println!("nodes {}", names.join(" "));
    match sim.run_batch_dc_op(&inputs) {
        Ok(outs) => {
            for o in outs {
                let hex: Vec<String> = o.solution.iter().map(|x| format!("{:016x}", x.to_bits())).collect();
                println!("ok {} {}", o.iterations, hex.join(" "));
            }
        }
        Err(e) => {
            println!("ERR {e}");
            std::process::exit(3);
        }
    }
}
