# RUN.md

To run the application on 3 nodes:

1. Update `Makefile` with all Internal IPs of all cluster memebers. 

2. On the build machine (one with Rust dependencies, defined in `INSTALL.md`), run:

```
$ make build-and-run-leader
```

3. On other machines (with no dependencies), run compiled binary:

```
$ make run-node
```


## Output

### Initial leader:
```
[2024-04-22T17:24:23Z INFO  raft_main::fsm_candidate] [candidate] no vote in progress, starting vote at 14887622471535 with term: 11 and election timeout: 1002004083
[2024-04-22T17:24:23Z INFO  raft_main::fsm_candidate] [candidate] Quorum reached after 198.19.249.40,14887622580118,118875, becoming leader with term: 11
[2024-04-22T17:24:24Z INFO  raft_main::fsm_leader] [leader] Simulating crash and becoming a follower
...
```


### Follower:
```
[2024-04-22T17:24:24Z INFO  raft_main] [XDP] [14889292195276] Updated node term to 11 (mine was 11).
[2024-04-22T17:24:24Z INFO  raft_main] [XDP] [14889292195276] Updated node term to 11 (mine was 11).
[2024-04-22T17:24:24Z INFO  raft_main] [XDP] [14889292195276] Updated node term to 11 (mine was 11).
[2024-04-22T17:24:24Z INFO  raft_main] [XDP] [14889292195276] Updated node term to 11 (mine was 11).
[2024-04-22T17:24:24Z INFO  raft_main] [XDP] [14889292195276] Updated node term to 11 (mine was 11).
[2024-04-22T17:24:24Z INFO  raft_main::fsm_follower] [follower] No communication received from the leader in 105 ms; becoming a candidate
[2024-04-22T17:24:24Z INFO  raft_main::fsm_candidate] [candidate] no vote in progress, starting vote at 14889399016954 with term: 12 and election timeout: 1001314994
[2024-04-22T17:24:24Z INFO  raft_main::fsm_candidate] [candidate] Quorum reached after 198.19.249.160,14889399102828,87750, becoming leader with term: 12
```


## Areas of interest

* `raft/main-ebpf/src/main.rs` - eBPF program handling UDP requests and responses for different ports.

* `raft/raft-main/src/fsm_single_thread.rs` - background process running different actions based on the node state.

* `raft/raft-main/src/routes.rs` - HTTP API for updating BPF maps.

