RUST_LOG=debug cargo run --release -- \
  --base-path /tmp/substrate-1_0-grandpa-example/bob \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/QmQZ8TjTqeDj3ciwr93EJ95hxfDsb9pEYDizUAbWpigtQN \
  --chain=local \
  --bob \
  --port 30334 \
  --validator



