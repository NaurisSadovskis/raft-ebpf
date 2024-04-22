PEERS = 198.19.249.40,198.19.249.160,198.19.249.93
RAFT_DIR = raft
LOG_LEVEL = info
INTERFACE = eth0

build-and-run-leader:
	cd $(RAFT_DIR)/ && RUST_LOG=$(LOG_LEVEL) PEERS=$(PEERS) cargo xtask run -- --iface $(INTERFACE)

run-node:
	RUST_LOG=$(LOG_LEVEL) PEERS=$(PEERS) ./$(RAFT_DIR)/target/debug/raft-main --iface $(INTERFACE)

fmt:
	cd $(RAFT_DIR)/ && cargo fmt 

clippy:
	cd $(RAFT_DIR)/ && cargo clippy 