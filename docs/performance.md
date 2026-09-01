# Performance budgets (alpha.1)

Targets (not yet load-tested at 100 servers — that is a beta gate):

| Surface | Budget |
|---|---|
| `GET /health` | < 20 ms local |
| `GET /v1/nodes` with 100 rows | < 100 ms local excluding network |
| Dashboard first paint | usable < 2 s on ordinary broadband |
| Argon2id login (prod params) | < 500 ms on Xeon E3-1270 v3 |

Commands:

```bash
cargo test -p fps-updater --release
# later: tests/performance with 100 simulated servers
```

Measure before optimizing. Record results here when collected.
