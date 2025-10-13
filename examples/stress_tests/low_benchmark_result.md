| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `axes --version` | 26.9 ± 3.4 | 21.0 | 37.0 | 1.00 |
| `just --version` | 47.7 ± 6.0 | 38.2 | 68.6 | 1.77 ± 0.32 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `axes ./low_test/script_0090` | 58.7 ± 6.4 | 47.1 | 80.7 | 1.00 |
| `just --justfile low_test/justfile receta_0090` | 68.6 ± 11.6 | 50.9 | 128.2 | 1.17 ± 0.24 |
