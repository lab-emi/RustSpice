subcircuit nesting depth 500 (1 ohm per level), i(v1) = -1/500
* bench.id: rob_hs_sub_nest_500
* bench.group: robustness
* bench.title: subcircuit nesting depth 500 (1 ohm per level), i(v1) = -1/500
* bench.probes: v(1), i(v1)
* bench.expect: either
* bench.tags: subckt, hierarchy, dc, ngspice_crash
* bench.est_rss_mib: 64
* bench.note: ngspice-47 cannot run this deck (nesting of 23 or more levels: "unknown subckt", segfault under -r). Closed form: 500 x 1 ohm in series, i(v1) = -0.002 A; RustSpice returns -0.002000000000000454 (2.3e-13 relative; peak RSS 8.5 MiB, 20 ms), so RustSpice is right and ngspice crashes. expect either (info / ngspice_only_error) because the engine has no closed-form judge when ngspice fails
.subckt s0 a b
X1 a m s1
R1 m b 1
.ends
.subckt s1 a b
X1 a m s2
R1 m b 1
.ends
.subckt s2 a b
X1 a m s3
R1 m b 1
.ends
.subckt s3 a b
X1 a m s4
R1 m b 1
.ends
.subckt s4 a b
X1 a m s5
R1 m b 1
.ends
.subckt s5 a b
X1 a m s6
R1 m b 1
.ends
.subckt s6 a b
X1 a m s7
R1 m b 1
.ends
.subckt s7 a b
X1 a m s8
R1 m b 1
.ends
.subckt s8 a b
X1 a m s9
R1 m b 1
.ends
.subckt s9 a b
X1 a m s10
R1 m b 1
.ends
.subckt s10 a b
X1 a m s11
R1 m b 1
.ends
.subckt s11 a b
X1 a m s12
R1 m b 1
.ends
.subckt s12 a b
X1 a m s13
R1 m b 1
.ends
.subckt s13 a b
X1 a m s14
R1 m b 1
.ends
.subckt s14 a b
X1 a m s15
R1 m b 1
.ends
.subckt s15 a b
X1 a m s16
R1 m b 1
.ends
.subckt s16 a b
X1 a m s17
R1 m b 1
.ends
.subckt s17 a b
X1 a m s18
R1 m b 1
.ends
.subckt s18 a b
X1 a m s19
R1 m b 1
.ends
.subckt s19 a b
X1 a m s20
R1 m b 1
.ends
.subckt s20 a b
X1 a m s21
R1 m b 1
.ends
.subckt s21 a b
X1 a m s22
R1 m b 1
.ends
.subckt s22 a b
X1 a m s23
R1 m b 1
.ends
.subckt s23 a b
X1 a m s24
R1 m b 1
.ends
.subckt s24 a b
X1 a m s25
R1 m b 1
.ends
.subckt s25 a b
X1 a m s26
R1 m b 1
.ends
.subckt s26 a b
X1 a m s27
R1 m b 1
.ends
.subckt s27 a b
X1 a m s28
R1 m b 1
.ends
.subckt s28 a b
X1 a m s29
R1 m b 1
.ends
.subckt s29 a b
X1 a m s30
R1 m b 1
.ends
.subckt s30 a b
X1 a m s31
R1 m b 1
.ends
.subckt s31 a b
X1 a m s32
R1 m b 1
.ends
.subckt s32 a b
X1 a m s33
R1 m b 1
.ends
.subckt s33 a b
X1 a m s34
R1 m b 1
.ends
.subckt s34 a b
X1 a m s35
R1 m b 1
.ends
.subckt s35 a b
X1 a m s36
R1 m b 1
.ends
.subckt s36 a b
X1 a m s37
R1 m b 1
.ends
.subckt s37 a b
X1 a m s38
R1 m b 1
.ends
.subckt s38 a b
X1 a m s39
R1 m b 1
.ends
.subckt s39 a b
X1 a m s40
R1 m b 1
.ends
.subckt s40 a b
X1 a m s41
R1 m b 1
.ends
.subckt s41 a b
X1 a m s42
R1 m b 1
.ends
.subckt s42 a b
X1 a m s43
R1 m b 1
.ends
.subckt s43 a b
X1 a m s44
R1 m b 1
.ends
.subckt s44 a b
X1 a m s45
R1 m b 1
.ends
.subckt s45 a b
X1 a m s46
R1 m b 1
.ends
.subckt s46 a b
X1 a m s47
R1 m b 1
.ends
.subckt s47 a b
X1 a m s48
R1 m b 1
.ends
.subckt s48 a b
X1 a m s49
R1 m b 1
.ends
.subckt s49 a b
X1 a m s50
R1 m b 1
.ends
.subckt s50 a b
X1 a m s51
R1 m b 1
.ends
.subckt s51 a b
X1 a m s52
R1 m b 1
.ends
.subckt s52 a b
X1 a m s53
R1 m b 1
.ends
.subckt s53 a b
X1 a m s54
R1 m b 1
.ends
.subckt s54 a b
X1 a m s55
R1 m b 1
.ends
.subckt s55 a b
X1 a m s56
R1 m b 1
.ends
.subckt s56 a b
X1 a m s57
R1 m b 1
.ends
.subckt s57 a b
X1 a m s58
R1 m b 1
.ends
.subckt s58 a b
X1 a m s59
R1 m b 1
.ends
.subckt s59 a b
X1 a m s60
R1 m b 1
.ends
.subckt s60 a b
X1 a m s61
R1 m b 1
.ends
.subckt s61 a b
X1 a m s62
R1 m b 1
.ends
.subckt s62 a b
X1 a m s63
R1 m b 1
.ends
.subckt s63 a b
X1 a m s64
R1 m b 1
.ends
.subckt s64 a b
X1 a m s65
R1 m b 1
.ends
.subckt s65 a b
X1 a m s66
R1 m b 1
.ends
.subckt s66 a b
X1 a m s67
R1 m b 1
.ends
.subckt s67 a b
X1 a m s68
R1 m b 1
.ends
.subckt s68 a b
X1 a m s69
R1 m b 1
.ends
.subckt s69 a b
X1 a m s70
R1 m b 1
.ends
.subckt s70 a b
X1 a m s71
R1 m b 1
.ends
.subckt s71 a b
X1 a m s72
R1 m b 1
.ends
.subckt s72 a b
X1 a m s73
R1 m b 1
.ends
.subckt s73 a b
X1 a m s74
R1 m b 1
.ends
.subckt s74 a b
X1 a m s75
R1 m b 1
.ends
.subckt s75 a b
X1 a m s76
R1 m b 1
.ends
.subckt s76 a b
X1 a m s77
R1 m b 1
.ends
.subckt s77 a b
X1 a m s78
R1 m b 1
.ends
.subckt s78 a b
X1 a m s79
R1 m b 1
.ends
.subckt s79 a b
X1 a m s80
R1 m b 1
.ends
.subckt s80 a b
X1 a m s81
R1 m b 1
.ends
.subckt s81 a b
X1 a m s82
R1 m b 1
.ends
.subckt s82 a b
X1 a m s83
R1 m b 1
.ends
.subckt s83 a b
X1 a m s84
R1 m b 1
.ends
.subckt s84 a b
X1 a m s85
R1 m b 1
.ends
.subckt s85 a b
X1 a m s86
R1 m b 1
.ends
.subckt s86 a b
X1 a m s87
R1 m b 1
.ends
.subckt s87 a b
X1 a m s88
R1 m b 1
.ends
.subckt s88 a b
X1 a m s89
R1 m b 1
.ends
.subckt s89 a b
X1 a m s90
R1 m b 1
.ends
.subckt s90 a b
X1 a m s91
R1 m b 1
.ends
.subckt s91 a b
X1 a m s92
R1 m b 1
.ends
.subckt s92 a b
X1 a m s93
R1 m b 1
.ends
.subckt s93 a b
X1 a m s94
R1 m b 1
.ends
.subckt s94 a b
X1 a m s95
R1 m b 1
.ends
.subckt s95 a b
X1 a m s96
R1 m b 1
.ends
.subckt s96 a b
X1 a m s97
R1 m b 1
.ends
.subckt s97 a b
X1 a m s98
R1 m b 1
.ends
.subckt s98 a b
X1 a m s99
R1 m b 1
.ends
.subckt s99 a b
X1 a m s100
R1 m b 1
.ends
.subckt s100 a b
X1 a m s101
R1 m b 1
.ends
.subckt s101 a b
X1 a m s102
R1 m b 1
.ends
.subckt s102 a b
X1 a m s103
R1 m b 1
.ends
.subckt s103 a b
X1 a m s104
R1 m b 1
.ends
.subckt s104 a b
X1 a m s105
R1 m b 1
.ends
.subckt s105 a b
X1 a m s106
R1 m b 1
.ends
.subckt s106 a b
X1 a m s107
R1 m b 1
.ends
.subckt s107 a b
X1 a m s108
R1 m b 1
.ends
.subckt s108 a b
X1 a m s109
R1 m b 1
.ends
.subckt s109 a b
X1 a m s110
R1 m b 1
.ends
.subckt s110 a b
X1 a m s111
R1 m b 1
.ends
.subckt s111 a b
X1 a m s112
R1 m b 1
.ends
.subckt s112 a b
X1 a m s113
R1 m b 1
.ends
.subckt s113 a b
X1 a m s114
R1 m b 1
.ends
.subckt s114 a b
X1 a m s115
R1 m b 1
.ends
.subckt s115 a b
X1 a m s116
R1 m b 1
.ends
.subckt s116 a b
X1 a m s117
R1 m b 1
.ends
.subckt s117 a b
X1 a m s118
R1 m b 1
.ends
.subckt s118 a b
X1 a m s119
R1 m b 1
.ends
.subckt s119 a b
X1 a m s120
R1 m b 1
.ends
.subckt s120 a b
X1 a m s121
R1 m b 1
.ends
.subckt s121 a b
X1 a m s122
R1 m b 1
.ends
.subckt s122 a b
X1 a m s123
R1 m b 1
.ends
.subckt s123 a b
X1 a m s124
R1 m b 1
.ends
.subckt s124 a b
X1 a m s125
R1 m b 1
.ends
.subckt s125 a b
X1 a m s126
R1 m b 1
.ends
.subckt s126 a b
X1 a m s127
R1 m b 1
.ends
.subckt s127 a b
X1 a m s128
R1 m b 1
.ends
.subckt s128 a b
X1 a m s129
R1 m b 1
.ends
.subckt s129 a b
X1 a m s130
R1 m b 1
.ends
.subckt s130 a b
X1 a m s131
R1 m b 1
.ends
.subckt s131 a b
X1 a m s132
R1 m b 1
.ends
.subckt s132 a b
X1 a m s133
R1 m b 1
.ends
.subckt s133 a b
X1 a m s134
R1 m b 1
.ends
.subckt s134 a b
X1 a m s135
R1 m b 1
.ends
.subckt s135 a b
X1 a m s136
R1 m b 1
.ends
.subckt s136 a b
X1 a m s137
R1 m b 1
.ends
.subckt s137 a b
X1 a m s138
R1 m b 1
.ends
.subckt s138 a b
X1 a m s139
R1 m b 1
.ends
.subckt s139 a b
X1 a m s140
R1 m b 1
.ends
.subckt s140 a b
X1 a m s141
R1 m b 1
.ends
.subckt s141 a b
X1 a m s142
R1 m b 1
.ends
.subckt s142 a b
X1 a m s143
R1 m b 1
.ends
.subckt s143 a b
X1 a m s144
R1 m b 1
.ends
.subckt s144 a b
X1 a m s145
R1 m b 1
.ends
.subckt s145 a b
X1 a m s146
R1 m b 1
.ends
.subckt s146 a b
X1 a m s147
R1 m b 1
.ends
.subckt s147 a b
X1 a m s148
R1 m b 1
.ends
.subckt s148 a b
X1 a m s149
R1 m b 1
.ends
.subckt s149 a b
X1 a m s150
R1 m b 1
.ends
.subckt s150 a b
X1 a m s151
R1 m b 1
.ends
.subckt s151 a b
X1 a m s152
R1 m b 1
.ends
.subckt s152 a b
X1 a m s153
R1 m b 1
.ends
.subckt s153 a b
X1 a m s154
R1 m b 1
.ends
.subckt s154 a b
X1 a m s155
R1 m b 1
.ends
.subckt s155 a b
X1 a m s156
R1 m b 1
.ends
.subckt s156 a b
X1 a m s157
R1 m b 1
.ends
.subckt s157 a b
X1 a m s158
R1 m b 1
.ends
.subckt s158 a b
X1 a m s159
R1 m b 1
.ends
.subckt s159 a b
X1 a m s160
R1 m b 1
.ends
.subckt s160 a b
X1 a m s161
R1 m b 1
.ends
.subckt s161 a b
X1 a m s162
R1 m b 1
.ends
.subckt s162 a b
X1 a m s163
R1 m b 1
.ends
.subckt s163 a b
X1 a m s164
R1 m b 1
.ends
.subckt s164 a b
X1 a m s165
R1 m b 1
.ends
.subckt s165 a b
X1 a m s166
R1 m b 1
.ends
.subckt s166 a b
X1 a m s167
R1 m b 1
.ends
.subckt s167 a b
X1 a m s168
R1 m b 1
.ends
.subckt s168 a b
X1 a m s169
R1 m b 1
.ends
.subckt s169 a b
X1 a m s170
R1 m b 1
.ends
.subckt s170 a b
X1 a m s171
R1 m b 1
.ends
.subckt s171 a b
X1 a m s172
R1 m b 1
.ends
.subckt s172 a b
X1 a m s173
R1 m b 1
.ends
.subckt s173 a b
X1 a m s174
R1 m b 1
.ends
.subckt s174 a b
X1 a m s175
R1 m b 1
.ends
.subckt s175 a b
X1 a m s176
R1 m b 1
.ends
.subckt s176 a b
X1 a m s177
R1 m b 1
.ends
.subckt s177 a b
X1 a m s178
R1 m b 1
.ends
.subckt s178 a b
X1 a m s179
R1 m b 1
.ends
.subckt s179 a b
X1 a m s180
R1 m b 1
.ends
.subckt s180 a b
X1 a m s181
R1 m b 1
.ends
.subckt s181 a b
X1 a m s182
R1 m b 1
.ends
.subckt s182 a b
X1 a m s183
R1 m b 1
.ends
.subckt s183 a b
X1 a m s184
R1 m b 1
.ends
.subckt s184 a b
X1 a m s185
R1 m b 1
.ends
.subckt s185 a b
X1 a m s186
R1 m b 1
.ends
.subckt s186 a b
X1 a m s187
R1 m b 1
.ends
.subckt s187 a b
X1 a m s188
R1 m b 1
.ends
.subckt s188 a b
X1 a m s189
R1 m b 1
.ends
.subckt s189 a b
X1 a m s190
R1 m b 1
.ends
.subckt s190 a b
X1 a m s191
R1 m b 1
.ends
.subckt s191 a b
X1 a m s192
R1 m b 1
.ends
.subckt s192 a b
X1 a m s193
R1 m b 1
.ends
.subckt s193 a b
X1 a m s194
R1 m b 1
.ends
.subckt s194 a b
X1 a m s195
R1 m b 1
.ends
.subckt s195 a b
X1 a m s196
R1 m b 1
.ends
.subckt s196 a b
X1 a m s197
R1 m b 1
.ends
.subckt s197 a b
X1 a m s198
R1 m b 1
.ends
.subckt s198 a b
X1 a m s199
R1 m b 1
.ends
.subckt s199 a b
X1 a m s200
R1 m b 1
.ends
.subckt s200 a b
X1 a m s201
R1 m b 1
.ends
.subckt s201 a b
X1 a m s202
R1 m b 1
.ends
.subckt s202 a b
X1 a m s203
R1 m b 1
.ends
.subckt s203 a b
X1 a m s204
R1 m b 1
.ends
.subckt s204 a b
X1 a m s205
R1 m b 1
.ends
.subckt s205 a b
X1 a m s206
R1 m b 1
.ends
.subckt s206 a b
X1 a m s207
R1 m b 1
.ends
.subckt s207 a b
X1 a m s208
R1 m b 1
.ends
.subckt s208 a b
X1 a m s209
R1 m b 1
.ends
.subckt s209 a b
X1 a m s210
R1 m b 1
.ends
.subckt s210 a b
X1 a m s211
R1 m b 1
.ends
.subckt s211 a b
X1 a m s212
R1 m b 1
.ends
.subckt s212 a b
X1 a m s213
R1 m b 1
.ends
.subckt s213 a b
X1 a m s214
R1 m b 1
.ends
.subckt s214 a b
X1 a m s215
R1 m b 1
.ends
.subckt s215 a b
X1 a m s216
R1 m b 1
.ends
.subckt s216 a b
X1 a m s217
R1 m b 1
.ends
.subckt s217 a b
X1 a m s218
R1 m b 1
.ends
.subckt s218 a b
X1 a m s219
R1 m b 1
.ends
.subckt s219 a b
X1 a m s220
R1 m b 1
.ends
.subckt s220 a b
X1 a m s221
R1 m b 1
.ends
.subckt s221 a b
X1 a m s222
R1 m b 1
.ends
.subckt s222 a b
X1 a m s223
R1 m b 1
.ends
.subckt s223 a b
X1 a m s224
R1 m b 1
.ends
.subckt s224 a b
X1 a m s225
R1 m b 1
.ends
.subckt s225 a b
X1 a m s226
R1 m b 1
.ends
.subckt s226 a b
X1 a m s227
R1 m b 1
.ends
.subckt s227 a b
X1 a m s228
R1 m b 1
.ends
.subckt s228 a b
X1 a m s229
R1 m b 1
.ends
.subckt s229 a b
X1 a m s230
R1 m b 1
.ends
.subckt s230 a b
X1 a m s231
R1 m b 1
.ends
.subckt s231 a b
X1 a m s232
R1 m b 1
.ends
.subckt s232 a b
X1 a m s233
R1 m b 1
.ends
.subckt s233 a b
X1 a m s234
R1 m b 1
.ends
.subckt s234 a b
X1 a m s235
R1 m b 1
.ends
.subckt s235 a b
X1 a m s236
R1 m b 1
.ends
.subckt s236 a b
X1 a m s237
R1 m b 1
.ends
.subckt s237 a b
X1 a m s238
R1 m b 1
.ends
.subckt s238 a b
X1 a m s239
R1 m b 1
.ends
.subckt s239 a b
X1 a m s240
R1 m b 1
.ends
.subckt s240 a b
X1 a m s241
R1 m b 1
.ends
.subckt s241 a b
X1 a m s242
R1 m b 1
.ends
.subckt s242 a b
X1 a m s243
R1 m b 1
.ends
.subckt s243 a b
X1 a m s244
R1 m b 1
.ends
.subckt s244 a b
X1 a m s245
R1 m b 1
.ends
.subckt s245 a b
X1 a m s246
R1 m b 1
.ends
.subckt s246 a b
X1 a m s247
R1 m b 1
.ends
.subckt s247 a b
X1 a m s248
R1 m b 1
.ends
.subckt s248 a b
X1 a m s249
R1 m b 1
.ends
.subckt s249 a b
X1 a m s250
R1 m b 1
.ends
.subckt s250 a b
X1 a m s251
R1 m b 1
.ends
.subckt s251 a b
X1 a m s252
R1 m b 1
.ends
.subckt s252 a b
X1 a m s253
R1 m b 1
.ends
.subckt s253 a b
X1 a m s254
R1 m b 1
.ends
.subckt s254 a b
X1 a m s255
R1 m b 1
.ends
.subckt s255 a b
X1 a m s256
R1 m b 1
.ends
.subckt s256 a b
X1 a m s257
R1 m b 1
.ends
.subckt s257 a b
X1 a m s258
R1 m b 1
.ends
.subckt s258 a b
X1 a m s259
R1 m b 1
.ends
.subckt s259 a b
X1 a m s260
R1 m b 1
.ends
.subckt s260 a b
X1 a m s261
R1 m b 1
.ends
.subckt s261 a b
X1 a m s262
R1 m b 1
.ends
.subckt s262 a b
X1 a m s263
R1 m b 1
.ends
.subckt s263 a b
X1 a m s264
R1 m b 1
.ends
.subckt s264 a b
X1 a m s265
R1 m b 1
.ends
.subckt s265 a b
X1 a m s266
R1 m b 1
.ends
.subckt s266 a b
X1 a m s267
R1 m b 1
.ends
.subckt s267 a b
X1 a m s268
R1 m b 1
.ends
.subckt s268 a b
X1 a m s269
R1 m b 1
.ends
.subckt s269 a b
X1 a m s270
R1 m b 1
.ends
.subckt s270 a b
X1 a m s271
R1 m b 1
.ends
.subckt s271 a b
X1 a m s272
R1 m b 1
.ends
.subckt s272 a b
X1 a m s273
R1 m b 1
.ends
.subckt s273 a b
X1 a m s274
R1 m b 1
.ends
.subckt s274 a b
X1 a m s275
R1 m b 1
.ends
.subckt s275 a b
X1 a m s276
R1 m b 1
.ends
.subckt s276 a b
X1 a m s277
R1 m b 1
.ends
.subckt s277 a b
X1 a m s278
R1 m b 1
.ends
.subckt s278 a b
X1 a m s279
R1 m b 1
.ends
.subckt s279 a b
X1 a m s280
R1 m b 1
.ends
.subckt s280 a b
X1 a m s281
R1 m b 1
.ends
.subckt s281 a b
X1 a m s282
R1 m b 1
.ends
.subckt s282 a b
X1 a m s283
R1 m b 1
.ends
.subckt s283 a b
X1 a m s284
R1 m b 1
.ends
.subckt s284 a b
X1 a m s285
R1 m b 1
.ends
.subckt s285 a b
X1 a m s286
R1 m b 1
.ends
.subckt s286 a b
X1 a m s287
R1 m b 1
.ends
.subckt s287 a b
X1 a m s288
R1 m b 1
.ends
.subckt s288 a b
X1 a m s289
R1 m b 1
.ends
.subckt s289 a b
X1 a m s290
R1 m b 1
.ends
.subckt s290 a b
X1 a m s291
R1 m b 1
.ends
.subckt s291 a b
X1 a m s292
R1 m b 1
.ends
.subckt s292 a b
X1 a m s293
R1 m b 1
.ends
.subckt s293 a b
X1 a m s294
R1 m b 1
.ends
.subckt s294 a b
X1 a m s295
R1 m b 1
.ends
.subckt s295 a b
X1 a m s296
R1 m b 1
.ends
.subckt s296 a b
X1 a m s297
R1 m b 1
.ends
.subckt s297 a b
X1 a m s298
R1 m b 1
.ends
.subckt s298 a b
X1 a m s299
R1 m b 1
.ends
.subckt s299 a b
X1 a m s300
R1 m b 1
.ends
.subckt s300 a b
X1 a m s301
R1 m b 1
.ends
.subckt s301 a b
X1 a m s302
R1 m b 1
.ends
.subckt s302 a b
X1 a m s303
R1 m b 1
.ends
.subckt s303 a b
X1 a m s304
R1 m b 1
.ends
.subckt s304 a b
X1 a m s305
R1 m b 1
.ends
.subckt s305 a b
X1 a m s306
R1 m b 1
.ends
.subckt s306 a b
X1 a m s307
R1 m b 1
.ends
.subckt s307 a b
X1 a m s308
R1 m b 1
.ends
.subckt s308 a b
X1 a m s309
R1 m b 1
.ends
.subckt s309 a b
X1 a m s310
R1 m b 1
.ends
.subckt s310 a b
X1 a m s311
R1 m b 1
.ends
.subckt s311 a b
X1 a m s312
R1 m b 1
.ends
.subckt s312 a b
X1 a m s313
R1 m b 1
.ends
.subckt s313 a b
X1 a m s314
R1 m b 1
.ends
.subckt s314 a b
X1 a m s315
R1 m b 1
.ends
.subckt s315 a b
X1 a m s316
R1 m b 1
.ends
.subckt s316 a b
X1 a m s317
R1 m b 1
.ends
.subckt s317 a b
X1 a m s318
R1 m b 1
.ends
.subckt s318 a b
X1 a m s319
R1 m b 1
.ends
.subckt s319 a b
X1 a m s320
R1 m b 1
.ends
.subckt s320 a b
X1 a m s321
R1 m b 1
.ends
.subckt s321 a b
X1 a m s322
R1 m b 1
.ends
.subckt s322 a b
X1 a m s323
R1 m b 1
.ends
.subckt s323 a b
X1 a m s324
R1 m b 1
.ends
.subckt s324 a b
X1 a m s325
R1 m b 1
.ends
.subckt s325 a b
X1 a m s326
R1 m b 1
.ends
.subckt s326 a b
X1 a m s327
R1 m b 1
.ends
.subckt s327 a b
X1 a m s328
R1 m b 1
.ends
.subckt s328 a b
X1 a m s329
R1 m b 1
.ends
.subckt s329 a b
X1 a m s330
R1 m b 1
.ends
.subckt s330 a b
X1 a m s331
R1 m b 1
.ends
.subckt s331 a b
X1 a m s332
R1 m b 1
.ends
.subckt s332 a b
X1 a m s333
R1 m b 1
.ends
.subckt s333 a b
X1 a m s334
R1 m b 1
.ends
.subckt s334 a b
X1 a m s335
R1 m b 1
.ends
.subckt s335 a b
X1 a m s336
R1 m b 1
.ends
.subckt s336 a b
X1 a m s337
R1 m b 1
.ends
.subckt s337 a b
X1 a m s338
R1 m b 1
.ends
.subckt s338 a b
X1 a m s339
R1 m b 1
.ends
.subckt s339 a b
X1 a m s340
R1 m b 1
.ends
.subckt s340 a b
X1 a m s341
R1 m b 1
.ends
.subckt s341 a b
X1 a m s342
R1 m b 1
.ends
.subckt s342 a b
X1 a m s343
R1 m b 1
.ends
.subckt s343 a b
X1 a m s344
R1 m b 1
.ends
.subckt s344 a b
X1 a m s345
R1 m b 1
.ends
.subckt s345 a b
X1 a m s346
R1 m b 1
.ends
.subckt s346 a b
X1 a m s347
R1 m b 1
.ends
.subckt s347 a b
X1 a m s348
R1 m b 1
.ends
.subckt s348 a b
X1 a m s349
R1 m b 1
.ends
.subckt s349 a b
X1 a m s350
R1 m b 1
.ends
.subckt s350 a b
X1 a m s351
R1 m b 1
.ends
.subckt s351 a b
X1 a m s352
R1 m b 1
.ends
.subckt s352 a b
X1 a m s353
R1 m b 1
.ends
.subckt s353 a b
X1 a m s354
R1 m b 1
.ends
.subckt s354 a b
X1 a m s355
R1 m b 1
.ends
.subckt s355 a b
X1 a m s356
R1 m b 1
.ends
.subckt s356 a b
X1 a m s357
R1 m b 1
.ends
.subckt s357 a b
X1 a m s358
R1 m b 1
.ends
.subckt s358 a b
X1 a m s359
R1 m b 1
.ends
.subckt s359 a b
X1 a m s360
R1 m b 1
.ends
.subckt s360 a b
X1 a m s361
R1 m b 1
.ends
.subckt s361 a b
X1 a m s362
R1 m b 1
.ends
.subckt s362 a b
X1 a m s363
R1 m b 1
.ends
.subckt s363 a b
X1 a m s364
R1 m b 1
.ends
.subckt s364 a b
X1 a m s365
R1 m b 1
.ends
.subckt s365 a b
X1 a m s366
R1 m b 1
.ends
.subckt s366 a b
X1 a m s367
R1 m b 1
.ends
.subckt s367 a b
X1 a m s368
R1 m b 1
.ends
.subckt s368 a b
X1 a m s369
R1 m b 1
.ends
.subckt s369 a b
X1 a m s370
R1 m b 1
.ends
.subckt s370 a b
X1 a m s371
R1 m b 1
.ends
.subckt s371 a b
X1 a m s372
R1 m b 1
.ends
.subckt s372 a b
X1 a m s373
R1 m b 1
.ends
.subckt s373 a b
X1 a m s374
R1 m b 1
.ends
.subckt s374 a b
X1 a m s375
R1 m b 1
.ends
.subckt s375 a b
X1 a m s376
R1 m b 1
.ends
.subckt s376 a b
X1 a m s377
R1 m b 1
.ends
.subckt s377 a b
X1 a m s378
R1 m b 1
.ends
.subckt s378 a b
X1 a m s379
R1 m b 1
.ends
.subckt s379 a b
X1 a m s380
R1 m b 1
.ends
.subckt s380 a b
X1 a m s381
R1 m b 1
.ends
.subckt s381 a b
X1 a m s382
R1 m b 1
.ends
.subckt s382 a b
X1 a m s383
R1 m b 1
.ends
.subckt s383 a b
X1 a m s384
R1 m b 1
.ends
.subckt s384 a b
X1 a m s385
R1 m b 1
.ends
.subckt s385 a b
X1 a m s386
R1 m b 1
.ends
.subckt s386 a b
X1 a m s387
R1 m b 1
.ends
.subckt s387 a b
X1 a m s388
R1 m b 1
.ends
.subckt s388 a b
X1 a m s389
R1 m b 1
.ends
.subckt s389 a b
X1 a m s390
R1 m b 1
.ends
.subckt s390 a b
X1 a m s391
R1 m b 1
.ends
.subckt s391 a b
X1 a m s392
R1 m b 1
.ends
.subckt s392 a b
X1 a m s393
R1 m b 1
.ends
.subckt s393 a b
X1 a m s394
R1 m b 1
.ends
.subckt s394 a b
X1 a m s395
R1 m b 1
.ends
.subckt s395 a b
X1 a m s396
R1 m b 1
.ends
.subckt s396 a b
X1 a m s397
R1 m b 1
.ends
.subckt s397 a b
X1 a m s398
R1 m b 1
.ends
.subckt s398 a b
X1 a m s399
R1 m b 1
.ends
.subckt s399 a b
X1 a m s400
R1 m b 1
.ends
.subckt s400 a b
X1 a m s401
R1 m b 1
.ends
.subckt s401 a b
X1 a m s402
R1 m b 1
.ends
.subckt s402 a b
X1 a m s403
R1 m b 1
.ends
.subckt s403 a b
X1 a m s404
R1 m b 1
.ends
.subckt s404 a b
X1 a m s405
R1 m b 1
.ends
.subckt s405 a b
X1 a m s406
R1 m b 1
.ends
.subckt s406 a b
X1 a m s407
R1 m b 1
.ends
.subckt s407 a b
X1 a m s408
R1 m b 1
.ends
.subckt s408 a b
X1 a m s409
R1 m b 1
.ends
.subckt s409 a b
X1 a m s410
R1 m b 1
.ends
.subckt s410 a b
X1 a m s411
R1 m b 1
.ends
.subckt s411 a b
X1 a m s412
R1 m b 1
.ends
.subckt s412 a b
X1 a m s413
R1 m b 1
.ends
.subckt s413 a b
X1 a m s414
R1 m b 1
.ends
.subckt s414 a b
X1 a m s415
R1 m b 1
.ends
.subckt s415 a b
X1 a m s416
R1 m b 1
.ends
.subckt s416 a b
X1 a m s417
R1 m b 1
.ends
.subckt s417 a b
X1 a m s418
R1 m b 1
.ends
.subckt s418 a b
X1 a m s419
R1 m b 1
.ends
.subckt s419 a b
X1 a m s420
R1 m b 1
.ends
.subckt s420 a b
X1 a m s421
R1 m b 1
.ends
.subckt s421 a b
X1 a m s422
R1 m b 1
.ends
.subckt s422 a b
X1 a m s423
R1 m b 1
.ends
.subckt s423 a b
X1 a m s424
R1 m b 1
.ends
.subckt s424 a b
X1 a m s425
R1 m b 1
.ends
.subckt s425 a b
X1 a m s426
R1 m b 1
.ends
.subckt s426 a b
X1 a m s427
R1 m b 1
.ends
.subckt s427 a b
X1 a m s428
R1 m b 1
.ends
.subckt s428 a b
X1 a m s429
R1 m b 1
.ends
.subckt s429 a b
X1 a m s430
R1 m b 1
.ends
.subckt s430 a b
X1 a m s431
R1 m b 1
.ends
.subckt s431 a b
X1 a m s432
R1 m b 1
.ends
.subckt s432 a b
X1 a m s433
R1 m b 1
.ends
.subckt s433 a b
X1 a m s434
R1 m b 1
.ends
.subckt s434 a b
X1 a m s435
R1 m b 1
.ends
.subckt s435 a b
X1 a m s436
R1 m b 1
.ends
.subckt s436 a b
X1 a m s437
R1 m b 1
.ends
.subckt s437 a b
X1 a m s438
R1 m b 1
.ends
.subckt s438 a b
X1 a m s439
R1 m b 1
.ends
.subckt s439 a b
X1 a m s440
R1 m b 1
.ends
.subckt s440 a b
X1 a m s441
R1 m b 1
.ends
.subckt s441 a b
X1 a m s442
R1 m b 1
.ends
.subckt s442 a b
X1 a m s443
R1 m b 1
.ends
.subckt s443 a b
X1 a m s444
R1 m b 1
.ends
.subckt s444 a b
X1 a m s445
R1 m b 1
.ends
.subckt s445 a b
X1 a m s446
R1 m b 1
.ends
.subckt s446 a b
X1 a m s447
R1 m b 1
.ends
.subckt s447 a b
X1 a m s448
R1 m b 1
.ends
.subckt s448 a b
X1 a m s449
R1 m b 1
.ends
.subckt s449 a b
X1 a m s450
R1 m b 1
.ends
.subckt s450 a b
X1 a m s451
R1 m b 1
.ends
.subckt s451 a b
X1 a m s452
R1 m b 1
.ends
.subckt s452 a b
X1 a m s453
R1 m b 1
.ends
.subckt s453 a b
X1 a m s454
R1 m b 1
.ends
.subckt s454 a b
X1 a m s455
R1 m b 1
.ends
.subckt s455 a b
X1 a m s456
R1 m b 1
.ends
.subckt s456 a b
X1 a m s457
R1 m b 1
.ends
.subckt s457 a b
X1 a m s458
R1 m b 1
.ends
.subckt s458 a b
X1 a m s459
R1 m b 1
.ends
.subckt s459 a b
X1 a m s460
R1 m b 1
.ends
.subckt s460 a b
X1 a m s461
R1 m b 1
.ends
.subckt s461 a b
X1 a m s462
R1 m b 1
.ends
.subckt s462 a b
X1 a m s463
R1 m b 1
.ends
.subckt s463 a b
X1 a m s464
R1 m b 1
.ends
.subckt s464 a b
X1 a m s465
R1 m b 1
.ends
.subckt s465 a b
X1 a m s466
R1 m b 1
.ends
.subckt s466 a b
X1 a m s467
R1 m b 1
.ends
.subckt s467 a b
X1 a m s468
R1 m b 1
.ends
.subckt s468 a b
X1 a m s469
R1 m b 1
.ends
.subckt s469 a b
X1 a m s470
R1 m b 1
.ends
.subckt s470 a b
X1 a m s471
R1 m b 1
.ends
.subckt s471 a b
X1 a m s472
R1 m b 1
.ends
.subckt s472 a b
X1 a m s473
R1 m b 1
.ends
.subckt s473 a b
X1 a m s474
R1 m b 1
.ends
.subckt s474 a b
X1 a m s475
R1 m b 1
.ends
.subckt s475 a b
X1 a m s476
R1 m b 1
.ends
.subckt s476 a b
X1 a m s477
R1 m b 1
.ends
.subckt s477 a b
X1 a m s478
R1 m b 1
.ends
.subckt s478 a b
X1 a m s479
R1 m b 1
.ends
.subckt s479 a b
X1 a m s480
R1 m b 1
.ends
.subckt s480 a b
X1 a m s481
R1 m b 1
.ends
.subckt s481 a b
X1 a m s482
R1 m b 1
.ends
.subckt s482 a b
X1 a m s483
R1 m b 1
.ends
.subckt s483 a b
X1 a m s484
R1 m b 1
.ends
.subckt s484 a b
X1 a m s485
R1 m b 1
.ends
.subckt s485 a b
X1 a m s486
R1 m b 1
.ends
.subckt s486 a b
X1 a m s487
R1 m b 1
.ends
.subckt s487 a b
X1 a m s488
R1 m b 1
.ends
.subckt s488 a b
X1 a m s489
R1 m b 1
.ends
.subckt s489 a b
X1 a m s490
R1 m b 1
.ends
.subckt s490 a b
X1 a m s491
R1 m b 1
.ends
.subckt s491 a b
X1 a m s492
R1 m b 1
.ends
.subckt s492 a b
X1 a m s493
R1 m b 1
.ends
.subckt s493 a b
X1 a m s494
R1 m b 1
.ends
.subckt s494 a b
X1 a m s495
R1 m b 1
.ends
.subckt s495 a b
X1 a m s496
R1 m b 1
.ends
.subckt s496 a b
X1 a m s497
R1 m b 1
.ends
.subckt s497 a b
X1 a m s498
R1 m b 1
.ends
.subckt s498 a b
X1 a m s499
R1 m b 1
.ends
.subckt s499 a b
R1 a b 1
.ends
V1 1 0 1
Xtop 1 0 s0
.op
.end
