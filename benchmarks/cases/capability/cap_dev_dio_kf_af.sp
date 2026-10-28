Diode flicker-noise KF/AF (noise-only, must not change DC)
* bench.id: cap_dev_dio_kf_af
* bench.group: capability
* bench.title: Diode flicker-noise KF/AF (noise-only, must not change DC)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, model_param, noise_only
V1 a 0 0
D1 a 0 DM
.model DM D(IS=1e-14 KF=1e-12 AF=1.5)
.dc V1 0 0.8 0.05
.end
