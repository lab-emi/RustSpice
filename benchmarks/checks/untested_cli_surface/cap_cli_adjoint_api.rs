// In-process harness for the adjoint LIBRARY surface that the CLI never requests (V/I source values, diode N/AREA, MOS L/KP, bad specs).
// Install: copy to crates/rustspice-solver/examples/cap_cli_adjoint_api.rs, then
//   CARGO_BUILD_JOBS=2 cargo build -p rustspice-solver --example cap_cli_adjoint_api
// Usage: cap_cli_adjoint_api <deck.sp> <target-node> <dev.param> [<dev.param> ...]
// Output: one `grad <dev.param> <value:.17e>` line per spec, or `ERR <message>` (exit 3) when the call fails as a whole.
use rustspice_solver::{ParameterGradSpec, Simulator};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = std::fs::read_to_string(&a[1]).expect("deck");
    let mut sim = Simulator::from_spice(&src).expect("compile");
    let specs: Vec<ParameterGradSpec> = a[3..]
        .iter()
        .map(|s| {
            let (d, p) = s.split_once('.').expect("dev.param");
            ParameterGradSpec::new(d, p)
        })
        .collect();
    match sim.run_dc_adjoint(&a[2], &specs) {
        Ok((_op, adj)) => {
            for (s, g) in a[3..].iter().zip(adj.gradients.iter()) {
                println!("grad {s} {g:.17e}");
            }
        }
        Err(e) => {
            println!("ERR {e}");
            std::process::exit(3);
        }
    }
}
