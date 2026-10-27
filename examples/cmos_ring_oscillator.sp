* 5-Stage CMOS Inverter Chain / Buffer Delay Benchmark Deck
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)

VDD vdd 0 DC 3.3
VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)

MP1 n1 in1 vdd vdd PMOD W=20u L=1u
MN1 n1 in1 0   0   NMOD W=10u L=1u
CL1 n1 0 20f

MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MN2 n2 n1 0   0   NMOD W=10u L=1u
CL2 n2 0 20f

MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0   0   NMOD W=10u L=1u
CL3 n3 0 20f

MP4 n4 n3 vdd vdd PMOD W=20u L=1u
MN4 n4 n3 0   0   NMOD W=10u L=1u
CL4 n4 0 20f

MP5 out n4 vdd vdd PMOD W=20u L=1u
MN5 out n4 0   0   NMOD W=10u L=1u
CL5 out 0 20f

.op
.tran 10p 4n
.end
