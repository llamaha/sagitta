For builds use `cargo build --release --all --features cuda`.
For tests use `cargo build --release --all --features cuda`.
To check if this compiles just use `cargo check --release --all --features cuda`.
Do not create files in /tmp because you don't have permissions.
If making test scripts in bash, reuse the same filename each time so I don't have to keep approving the changes and execution.

