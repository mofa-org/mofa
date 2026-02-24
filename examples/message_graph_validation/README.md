# message_graph_validation

Practical Phase-1 validation example for Task 19 (MessageGraph).

## What this example verifies

1. Real routing behavior:
   - Builds an order-routing MessageGraph
   - Evaluates route matches for a high-risk `order.created` message
2. Pre-runtime safety:
   - Builds an intentionally invalid graph
   - Confirms compile-time validation rejects missing node references

## Run

From repository root:

```bash
cargo run --manifest-path examples/Cargo.toml -p message_graph_validation
```
