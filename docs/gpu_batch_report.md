# Batched DC operating points: CPU vs CUDA

RustEDA publishes one benchmark: a matched Ibex RTL-to-GDS comparison with LibreLane on IHP SG13G2 / EZ130. See the [RustEDA benchmarks](https://lab-emi.github.io/RustEDA/benchmarks/). This page describes how `rustspice bench-batch` times batched DC operating points, without published results.

## Workloads

- Circuits: the 32-diode cascade of b6 (MNA dimension 34) and the 100-stage CMOS inverter chain of b4 (200 level-1 MOSFETs with their bulk junctions, MNA dimension 104).
- Diode cascade, supply corners: V1 swept across the corners.
- Diode cascade, Monte Carlo corners: V1, every resistor and every diode IS varied (97 parameters per corner).
- CMOS inverter chain, supply and input corners: VDD and the input varied together.
- CMOS inverter chain, Monte Carlo corners: VDD, and VTO and KP of every MOSFET (402 parameters per corner).

## Method

- Times are end to end: from the `BatchCircuitInput` list to one solution vector per corner, including plan building, parameter mapping, transfers and CPU re-solves.
- CPU batch API: `Simulator::run_batch_dc_op` (rayon, one topology clone and one full CPU solve per corner). cpu_ref: the batch kernel's CPU build (rayon). CUDA: the same kernel source compiled to PTX by cuda-oxide, one corner per thread, FP64.
- The GPU's one-off initialisation (driver, context, PTX JIT) is measured once per process and reported separately and in the single-shot column.
- Differences are `max |a - b| / (1 + max(|a|, |b|))` over all corners and unknowns. Every corner the kernel does not converge is re-solved by the CPU solver and counted.
- The report records the CPU model, the rayon thread count, the host load average at start and end, and the GPU and CUDA driver.

## Recorded agreement

The last recorded run (`8f86bb4`, RTX 4090 Laptop GPU) agreed with the kernel's CPU build to at most `5.6e-16` (bit for bit on the MOSFET circuits) and with the CPU solver to at most `1.8e-9` (relative, inside its Newton tolerance). No corner needed a CPU re-solve.

## Reproduce

```sh
cargo build --release -p rustspice-cli
./target/release/rustspice bench-batch --output target/gpu-batch.md
```

`--corners`, `--mc-corners`, `--mos-corners` and `--mos-mc-corners` set the corner counts; `--reps` sets the repetitions. Without a usable CUDA GPU, the report says so and runs only the CPU columns.
