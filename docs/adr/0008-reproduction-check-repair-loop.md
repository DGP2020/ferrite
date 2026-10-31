# Two-Phase Reproduction Check for Counterexample Repair

Sending unverified counterexamples to the LLM wastes API tokens and compute on resource-constrained hardware. Ferrite executes the synthesized Rust test before invoking the repair prompt to confirm it reproduces the failure, and re-executes the test immediately after translation before launching any full differential fuzz campaign.
