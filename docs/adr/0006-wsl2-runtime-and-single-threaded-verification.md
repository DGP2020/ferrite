# WSL2 Linux Runtime and Single-Threaded Verification Execution

Cross-filesystem file I/O from `/mnt/c` degrades compilation speed significantly, while Windows host memory is constrained to 8 GB. Ferrite runs natively inside WSL2 in the Linux home directory (`~/ferrite`) with a 5 GB RAM / 4 GB swap cap, allowing concurrent LLM API calls but strictly serializing heavy compilation and fuzz jobs (with `-rss_limit_mb=1024` for libFuzzer).
