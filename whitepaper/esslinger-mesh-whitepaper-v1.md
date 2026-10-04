# Esslinger Mesh

## An Experimental, Verifiable Multi-Chain and Mesh Network

**Whitepaper v1 · 4 October 2026**  
**Author and operator:** Sven Normen Eßlinger, Esslinger Consulting (GitHub: [digitaldesignerjazz](https://github.com/digitaldesignerjazz))  
**Project codename:** Nexus · **Public network name:** Esslinger Mesh

> **Important legal notice — please read first.**
> Esslinger Mesh is an experimental research and test network. This document is **not** an offer of crypto-assets, not a prospectus or crypto-asset white paper within the meaning of the EU Markets in Crypto-Assets Regulation (MiCA), not a solicitation, and **not investment, legal or tax advice**. X-Coin, Q-Coin and all test denominations described here are **not offered, sold or listed** anywhere, have **no stated monetary value**, and no price, peg, yield or return is promised or implied. The software is experimental and **has not been independently audited**. A MiCA review would be required before any public offering or any publication of a value-related metric. See Section 11.

---

## Abstract

Esslinger Mesh is a single-operator, experimental network that combines two application chains (X-Coin and Q-Coin), two harvester chains and a separate public-test chain — all built with the Cosmos SDK and CosmWasm (wasmd) and connected over IBC by a Hermes relayer — with a private mesh-networking layer (Yggdrasil, Headscale/Tailscale) and a Rust edge node ("Onyx") running on the operator's own hardware in Hannover, Germany. An orchestrator and a set of AI agents ("Lumina") run alongside the chains.

The project's distinguishing focus is **verifiability rather than scale**: every claim about the network's health is meant to be backed by signed, append-only evidence that anyone can re-check with a single script, anchored in a public Git repository. As of 4 October 2026, six chain nodes are producing blocks locally, a phase-1 measurement re-verified 16,037 of 16,037 validator signatures (100%), and the first scored measurement epoch ("Epoch 1") is running with 70 pre-committed random probes against the Hannover node; its first probe passed. A hybrid price-*reference* concept exists only as an internal shadow calculation; nothing is published as a price.

This paper describes what exists today, what is only planned, and — explicitly — what the system cannot yet guarantee.

---

## 1. Problem and Motivation

Small, self-hosted blockchain and mesh experiments usually suffer from two problems:

1. **Unverifiable health claims.** "The node was online" or "the network is up" is typically self-reported. Without signed, time-bound evidence, such claims cannot be checked by outsiders — and often not even by the operator after the fact.
2. **Unsafe exposure.** Opening a node to the public internet, or attaching a price to a token, creates technical, financial and regulatory risk long before the system is mature.

Esslinger Mesh is an attempt to address both on a small scale:

- **Evidence first.** Measurements are challenge–response based, signed on both sides, written to append-only hash-chained logs, and anchored in a public repository, so that the operator cannot silently rewrite history after a push.
- **Gated exposure.** Nothing becomes public — no endpoint, no tunnel, no metric, no price — without an explicit approval step (operator decision and, for network-affecting changes, the project's Governance process).
- **Honest limits.** The documentation records rejected proposals, aborted tests and known weaknesses alongside successes.

---

## 2. Architecture Overview

```
+-------------------------------------------------------+
|  Operator host ("the Box", single machine)            |
|                                                       |
|  nexus-xcoin-1  <--IBC-->  nexus-qcoin-1 (+ AMM)      |
|       ^ IBC                      ^ IBC                |
|  xcoin-harvester-1         qcoin-harvester-1          |
|        (one Hermes relayer serves all IBC paths)      |
|                                                       |
|  esslinger-mesh-testnet-1 (validator + QNode)         |
|  RPC allowlist filter (local only, gated)             |
|                                                       |
|  Nexus orchestrator | Lumina agents | Ollama          |
|  Measurement server (verifier) | Epoch 1              |
|  Yggdrasil | Headscale | Tailscale                    |
+---------------------------+---------------------------+
                            |
             encrypted mesh (Yggdrasil / libp2p)
                            |
              +-------------v--------------+
              |  Onyx PC, Hannover         |
              |  onyx-hannover-01 (Rust)   |
              |  answers signed challenges |
              +----------------------------+

Public, outside the Box:
  Cloudflare Workers (forums, free tier)
  GitHub: esslinger-mesh, esslinger-mesh-nachweise
```

### 2.1 Nexus Orchestrator and Lumina Agents

The **Nexus orchestrator** is a set of Python reference implementations (orchestration hub, mesh connector, agent swarm, task dispatch and heartbeats) used for rapid prototyping; it is explicitly a prototype layer. The **Lumina agents** are a group of cooperating software agents (an orchestrator, "Lumia", plus responder and helper agents) that run on the operator host and communicate over the mesh overlay. A local **Ollama** runtime is available for on-device language-model inference. These components support operations and experimentation; they do not hold validator keys and do not control the chains.

### 2.2 X-Coin and Q-Coin Chains

- **nexus-xcoin-1 (Chain A, X-Coin)** and **nexus-qcoin-1 (Chain B, Q-Coin)** are two independent chains built with the Cosmos SDK on **wasmd v0.61.15** (CosmWasm; Cosmos SDK v0.53.6, ibc-go v10.5.0), with the address prefix `nexus`.
- Each coin lives on its own chain and moves between them only through standard **ICS-20 IBC transfers**. X-Coin appears on Chain B only as an IBC voucher; the original supply stays escrowed on Chain A.
- Each chain uses a separate staking/gas denomination, so the X-Coin and Q-Coin supplies are **fixed**; inflation (x/mint) only creates the staking/gas denominations.
- Chain B hosts **nexus-amm**, a small, project-written CosmWasm constant-product pool (x·y = k, 0.3% fee, permissionless swap/provide/withdraw, no oracle). Its contract admin is the chain's on-chain governance module, so a migration is only possible through an on-chain governance proposal. The contract has **not been audited**.
- Each chain currently has **exactly one validator** on the operator host, and all ports are bound to the local machine. Technically this is **testnet status**: not decentralised, not public, and not joinable by third parties.

### 2.3 Harvester Chains

**xcoin-harvester-1** and **qcoin-harvester-1** are two further chains connected to Chain A and Chain B respectively over IBC. A one-shot harvester script (one epoch per invocation; no loop, no scheduler) distributes from a governance-approved reserve **in equal shares** to a fixed list of 42 registered recipients — the project's own bots and agents — with a per-wallet cap. It does **not** read performance data; any performance-based distribution would require a new Governance decision and is not planned for now.

### 2.4 Public-Test Chain: esslinger-mesh-testnet-1

A separate, fresh chain was created specifically for public testing, so that the X-Coin/Q-Coin and harvester chains are never used as a test playground.

- Genesis approved by Governance on 3 October 2026; node started manually right after the approval.
- Two nodes: one validator and one non-validating full node ("QNode"), synchronised over a local peer link.
- **Worthless test denominations only:** a non-transferable bonding stake, a test gas token, and testnet-only tX-Coin/tQ-Coin balances. A faucet with per-claim and per-day limits and no stake issuance.
- Hardening: CosmWasm code upload and instantiation restricted to governance; IBC transfer and interchain accounts initially disabled; unsafe RPC, pprof and CORS off.
- A small local trading test (atomic peer-to-peer swaps between six test accounts) was run on 3 October 2026; an AMM pool on this chain requires an on-chain governance proposal, which was still in its voting period at the time of writing.
- The testnet is **not currently publicly reachable** (see Section 7.1).

### 2.5 IBC Relayer

One **Hermes** relayer process (Informal Systems, Apache-2.0, official release build with verified checksum) serves all IBC paths: X-Coin ↔ Q-Coin, X-Coin ↔ X-Coin harvester, and Q-Coin ↔ Q-Coin harvester.

### 2.6 Mesh Networking: Yggdrasil, Headscale, Tailscale

- **Yggdrasil** provides an end-to-end encrypted IPv6 overlay between the operator host and the Hannover node. Measurement probes travel over this overlay.
- **Headscale** (a self-hosted coordination server compatible with Tailscale clients) and **Tailscale** provide an additional private WireGuard-based underlay.
- No mesh addresses, peer identifiers or keys are published in this paper.

### 2.7 Onyx Edge Node (onyx-hannover-01)

**Onyx** is an open-source Rust edge node (Apache-2.0, alpha, v0.2.0-alpha.1) with a local-first identity: it uses libp2p (Gossipsub with signed messages, Noise, Yamux, QUIC) and an Ed25519 key whose public half is embedded in its libp2p peer identity. The instance **onyx-hannover-01** runs on the operator's own PC in Hannover. For measurement, it was extended with a one-shot `prove` command and a request–response protocol that signs verifier challenges with a domain-separated prefix. Onyx is **not a validator**: a proposal to make it one was **rejected by Governance on 3 October 2026**.

### 2.8 Measurement and Proof System ("Miner proof")

The measurement server runs on the operator host and acts as the **verifier**; the Onyx PC is the **measured subject**. Design rules: only verifiable evidence counts; self-reported numbers count for nothing; verifier-side failures are recorded as `verifier_outage` and never counted against the subject.

**Phase 1 — validator uptime (completed, not scored).** A read-only collector fetches every block header and commit over RPC and re-verifies offline: header hash = block ID, validator-set hash, an unbroken chain via `last_block_id`, and every commit signature with Ed25519. Results go into an append-only JSONL log with a hash chain and an RFC 6962 Merkle root. The trial run covering 3 October 2026 12:58 CEST to 4 October ~00:09 CEST found **16,037 of 16,037 signatures valid (8,002 blocks on nexus-xcoin-1, 8,035 on nexus-qcoin-1), no gaps above 60 s, uptime 100%** on both chains. Because the operator host verifies its own validators, this is explicitly labelled a *verifier self-measurement* and is not scored.

**Phase 2 — signed challenges and verifier key (completed).** The verifier has its own Ed25519 key (separate from all wallet, validator and mesh keys). Each challenge contains a fresh nonce bound to a recent block hash; Onyx signs the answer with its node key; the verifier signs every log record. A 34-case tamper test suite (forged, missing or foreign signatures, rewritten old records, truncated logs, modified genesis attestation, etc.) passed 34/34.

**Epoch 1 — the first scored epoch (running).**

| Item | Value |
|---|---|
| Window | 4 Oct 2026 01:30:08 – 11 Oct 2026 01:30:08 CEST (7 days) |
| Subject | onyx-hannover-01 only |
| Scored metric | Uptime / liveness only, via signed `ping` challenges (compute and capacity probes are a later phase) |
| Probes | 70 at random times (10 per day, ≥ 30 min apart), derived from a secret seed |
| Pre-commitment | Seed and schedule commitments signed and pushed to the public repository at 01:30:21 CEST, before the first probe |
| Rule | U = pass / (planned − verifier_outage); threshold **90%**; ≥ 42 planned probes; ≤ 10% verifier outage, otherwise the epoch is void |
| Retry | At most one retry 60 s after a failed probe |
| End | The process ends itself, reveals the seed and schedule, and anchors the final state; evaluation is done by hand |
| Status at 03:05 CEST, 4 Oct | First probe (03:04:51 CEST) **passed**; 1 of 70 recorded |

**Anchoring and public verification.** Evidence is committed to the public repository [`digitaldesignerjazz/esslinger-mesh-nachweise`](https://github.com/digitaldesignerjazz/esslinger-mesh-nachweise) (public since 4 October 2026). It contains only logs, proofs, the verifier's public key and verification scripts — no secrets; a secret scan runs before every commit. Anyone can verify without access to the operator host:

```bash
pip install cryptography
python3 verify_alles.py   # exit code 0 = all checks passed
```

`verify_alles.py` checks the attestation chain and verifier signatures, that attested log prefixes are unchanged, every Onyx answer signature and its binding to nonce and block hash, every validator commit signature, header hashes, contiguous heights, run-to-run continuity, and — after the reveal — the seed and probe schedule.

**Result use.** The epoch result feeds only an **internal shadow calculation** (Section 3.3). It does not affect payouts or any public figure.

---

## 3. Price-Reference Mechanism (Concept, Internal Only)

> **Status:** draft concept (internal draft v8, 4 October 2026). It is a *display and monitoring* concept, **not** a price, peg or stability mechanism. No USD or basket value is shown publicly, and nothing in this section is published as a price.

### 3.1 Principle

Price discovery for X-Coin and Q-Coin, if any, happens **only in the on-chain AMM pool**. External reference prices of BTC, ETH and SOL serve only as a *reference anchor* to indicate whether the pool ratio has drifted far from a fixed reference ratio. The mechanism triggers no forced trades, no mint/burn and no interventions from genesis or reserve accounts.

### 3.2 Hybrid Reference Baskets and Traffic Light

Because the AMM trades only X-Coin against Q-Coin, the only observable market quantity is the ratio *r* (Q-Coin per X-Coin). Each coin has its own fixed reference basket for the test phase:

| Basket | BTC | ETH | SOL |
|---|---|---|---|
| X-Coin | 60% | 30% | 10% |
| Q-Coin | 10% | 30% | 60% |

- Basket index: I(t) = Σ wᵢ · Pᵢ(t) / Pᵢ(t₀), with I(t₀) = 1
- Reference ratio: r\*(t) = r₀ · I_X(t) / I_Q(t), where r₀ is the pool ratio read at the base time
- Deviation: d = (r_pool − r\*) / r\*
- **Traffic light** (initial values, to be recalibrated): green |d| < 5% · yellow 5% ≤ |d| ≤ 15% · red |d| > 15%
- **Corridor** (prepared, not public): ±10% around r\*, with soft reactions only (a status flag and, if needed, a Governance discussion)

Calculation is manual and on demand only — no automation. If anything were ever shown publicly, it would be limited to the traffic-light colour, the deviation in percent and a timestamp, each release individually approved by the operator. A corridor variant would be published only after a legal (MiCA) review, because a band around a crypto basket could be misread as a stability promise.

### 3.3 Q-Coin Performance Index (Shadow Calculation)

A concept couples Q-Coin's *reference* to verified network performance — compute (50%) and capacity (50%), each the median of an epoch's samples — with uptime as a 90% minimum threshold rather than a weight. It runs **only as an internal shadow value** ("Q0"): it does not change the traffic light, does not affect harvester payouts, and is not published. Any reconsideration is foreseen at the earliest after four cleanly measured epochs. Epoch 1 measures uptime only.

---

## 4. Current Status (verified 4 October 2026, ~03:05 CEST)

| Component | Status |
|---|---|
| nexus-xcoin-1, nexus-qcoin-1 | Producing blocks (local, one validator each) |
| xcoin-harvester-1, qcoin-harvester-1 | Producing blocks (local) |
| esslinger-mesh-testnet-1 | Producing blocks; two nodes (validator + QNode); local only |
| Hermes IBC relayer | Running (one process for all paths) |
| Yggdrasil, Headscale, Tailscale | Running |
| Ollama, Nexus orchestrator, Lumina agents | Running |
| Onyx listener (operator host side) | Running |
| Validator uptime, phase 1 (trial, not scored) | 16,037 / 16,037 signatures valid, 100% |
| Epoch 1 (onyx-hannover-01) | Running; 70 pre-committed probes; first probe passed |
| Public evidence repository | Live; independently verifiable with `verify_alles.py` |
| Forums | Hosted on Cloudflare Workers (free tier) |
| Google Cloud node for Hannover-Primary | Setup package prepared and tested (14/14 local tests); **not deployed** |
| Public testnet access | **Not active**; gated by Governance (Section 7.1) |

Two observations are recorded for transparency: (1) on 4 October 2026 at 00:42–00:44 CEST, both local chains paused for ~98 s at the same moment; the host kernel log points to a virtual-machine pause/snapshot resume rather than a chain fault — in Epoch 1 such events count as verifier outages, not as subject failures; (2) the operator host has no native boot-time autostart, so after a host restart the stack is brought up by a start script.

---

## 5. Public Code

- [`digitaldesignerjazz/esslinger-mesh`](https://github.com/digitaldesignerjazz/esslinger-mesh) — public-source preparation: the minimal CosmWasm AMM (Apache-2.0) and neutral architecture notes. No live state, wallets, validator material, genesis files, credentials or access instructions.
- [`digitaldesignerjazz/esslinger-mesh-nachweise`](https://github.com/digitaldesignerjazz/esslinger-mesh-nachweise) — public evidence and verification scripts (Section 2.8).

---

## 6. Security Model

| Principle | Implementation |
|---|---|
| Local by default | All chain RPC/P2P listeners are bound to the local machine; no public seeds or peers. |
| Operator wallets are never touched | Measurement reads public chain data only; no validator or wallet keys are used to sign anything for measurement or analysis. Secrets live in an access-restricted vault and are never placed in repositories, logs or documents. |
| Separate keys per role | The verifier key, node keys, validator keys and test-chain keys are distinct; node signatures use domain-separation prefixes. |
| Simulation before execution | Transactions are prepared unsigned and simulated first (e.g. generate-only plus ABCI Simulate); test-chain transactions are validated offline before broadcast. Analyses that found no permitted transaction sent nothing. |
| No unattended automation | No cron jobs, timers or watchdogs without the operator's explicit approval. The documented exception is a supervisor for the Epoch 1 measurement process only, bounded in restarts and ending with the epoch. |
| Evidence is never deleted or rewritten | Evidence logs are append-only and created exclusively; finished files are write-protected; pre-key records were attested, not rewritten; backups use content-addressed hard links and nothing is pruned. |
| Fail closed | A stop file halts the measurement process within seconds; failed safety checks abort tests automatically. |

### 6.1 RPC Allowlist Filter

Public read access to the test chain is meant to pass through a project-written **RPC allowlist filter** in front of the node (never the X-Coin/Q-Coin or harvester chains). After the 3 October tunnel test (Section 7.1), the filter was hardened on 4 October 2026:

- the hard-coded pass-through for `check_tx` was removed;
- a config-independent **hard-deny list** (e.g. `check_tx`, all `broadcast_*` commit/evidence variants, subscriptions, consensus dumps, `net_info`, all `unsafe_*` and `dial_*`);
- `abci_query` paths containing simulation or broadcast services, or path traversal, are rejected;
- duplicate GET query parameters are rejected (prevents parser-differential bypasses);
- batch-size, page-size and rate limits; REST disabled on the node.

A local self-test (no tunnel) on 4 October 2026, 03:11–03:12 CEST passed **84/84 cases** (26 positive, 58 negative); an audit of all 319 logged requests found **zero** successful responses for non-allowlisted methods, and the test chain's mempool stayed empty. The hardened filter awaits Governance confirmation before any further public test.

---

## 7. Governance

- **Binding decisions** on network-affecting actions are made through the project's Governance process (a dedicated Governance agent working from versioned request and decision files). Every payout from genesis accounts — reserves, liquidity, fee grants — is prepared only as an **unsigned** transaction and executed only after an explicit APPROVE.
- **On-chain governance (x/gov)** is active and used for technical parameter changes and contract migrations (it is the AMM's contract admin).
- Governance **can and does say no**: the Onyx validator proposal was rejected on 3 October 2026; on the test chain, contract upload and instantiation are reserved to governance.
- **Operator decisions** (Sven Normen Eßlinger) govern publication: no public metric, status post or price-related figure is released without individual approval.

### 7.1 Gated Public Exposure: the 3 October Tunnel Test

On 3 October 2026 Governance approved a time-boxed (max. 30 min) test exposing the test chain through a temporary Cloudflare quick tunnel and the RPC filter, supervised by a bounded safety sampler with automatic abort rules. About 74 seconds into the publicly reachable phase, the sampler **aborted the test automatically** because `check_tx` had passed the filter (it was hard-coded, outside the approved allowlist). Assessment: no broadcast, unsafe or other state-changing method got through; `check_tx` does not add to the mempool; the node was never under load; all test processes ended; no unexpected external requests were observed. The fix described in Section 6.1 has since been applied and tested locally; a repeat test requires a fresh Governance approval. The tunnel is **not running**.

---

## 8. Hosting Overview

| Where | What |
|---|---|
| Operator host (single machine) | All chains, relayer, testnet, mesh daemons, orchestrator, agents, verifier |
| Onyx PC, Hannover | onyx-hannover-01 (measured subject) |
| Cloudflare Workers (free tier) | Community forums |
| GitHub | Public source and public evidence repository |
| Google Cloud | Nothing deployed (a free-tier, IPv6-only node package is prepared) |

---

## 9. Roadmap and Vision (Plans — Not Commitments)

> Everything in this section is a **plan or intention**. Items may change, be delayed or be dropped. None of it is a promise.

1. **Complete Epoch 1** (ends 11 October 2026), reveal the seed and schedule, and evaluate by hand against the 90% threshold.
2. **Phase 3 measurement:** verifiable compute probes (reference tasks with spot checks) and capacity probes (timed retrieval from a fresh 256 MB dataset per epoch).
3. **Stronger anchoring:** possibly OpenTimestamps; an on-chain memo transaction only for scored epochs and only with Governance approval; additional independent verifiers once there are external participants.
4. **Controlled public testnet access:** a repeat, time-boxed tunnel test after Governance approval; later, possibly a read-only proxy on Cloudflare's free tier in front of a separate full node (never the validator).
5. **Testnet AMM pool** via on-chain governance, seeded only from the test faucet with worthless test tokens.
6. **Bridge proof of concept (concept stage):** a Hyperlane-based route between the test chain and public testnets (e.g. Sepolia, Solana Devnet) with faucet tokens only — no real assets, no mainnet routes.
7. **Infrastructure independence:** deploy the prepared Google Cloud node; add independent participants only via Governance registration.
8. **Before any value-bearing or public use:** independent security audit, legal (MiCA) assessment, and Governance approval.

**Vision.** A small, transparent network in which participants can *prove* — not merely claim — that they contributed availability and resources, where every public statement about the network can be re-verified by anyone from published evidence, and where exposure grows only as fast as safety and legal review allow.

---

## 10. Risks and Limitations

- **Single operator, single machine.** Almost all components run on one host operated by one person. That host is a single point of failure; it has no native boot autostart, and VM pauses have been observed.
- **Not decentralised.** Each chain has one validator. The validator holds its own consensus keys and could, in principle, sign an alternative history; only external anchoring mitigates this.
- **Verifier and subject share an operator.** A signature proves *which key* answered, not *where* the machine is; outsourcing of work cannot be prevented. Timestamps come from the host clock and its own chains; the only external time evidence is GitHub's push record.
- **Experimental, unaudited software.** wasmd and Hermes are established open-source projects, but the AMM contract, RPC filter, measurement scripts and Onyx extensions are project-written, alpha-quality and not independently audited. The verifier and collector share a common library, so a bug there would affect both.
- **Small sample sizes.** Epoch 1 measures uptime of one node with 70 probes; results say nothing about compute, capacity or broader network performance.
- **Third-party dependence.** Cloudflare, GitHub and (if deployed) Google Cloud may change terms or free-tier limits.
- **Regulatory uncertainty.** Any value reference, corridor or performance coupling could be classified differently under MiCA depending on its design (Section 11).

---

## 11. Legal Notice

- This document is published for **informational and research purposes only**.
- It is **not** an offer to the public of crypto-assets, not an admission to trading, not a crypto-asset white paper under Regulation (EU) 2023/1114 (MiCA), and not a prospectus.
- It is **not** investment, financial, legal or tax advice. Nothing herein is a recommendation to acquire, hold or dispose of any asset.
- X-Coin, Q-Coin and all testnet denominations are **not offered, sold, listed or exchangeable** for money through this project. No price, value, peg, yield, return or profit is promised, implied or published. Test tokens are worthless by design.
- The reference-basket and performance-index concepts are internal calculations, not prices. They are not published as prices.
- A **MiCA assessment** (and any other applicable legal review) would be required **before** any public offering, admission to trading, or publication of a value-related metric.
- The software is experimental and provided without warranty. Use at your own risk.
- "Cosmos SDK", "CosmWasm", "Tailscale", "Cloudflare", "Google Cloud", "GitHub" and other names are trademarks of their respective owners; Esslinger Mesh is **built with** the Cosmos SDK and is not affiliated with or endorsed by those owners.

---

## 12. Conclusion

Esslinger Mesh is deliberately small and openly experimental: five Cosmos SDK-based chains (six nodes) connected by IBC, a private encrypted mesh, an edge node on the operator's own hardware, and a measurement system whose results anyone can re-verify from public evidence. Its main contribution so far is process rather than scale: pre-committed random probes, signatures on both sides, append-only evidence, public anchoring, gated exposure — and a record that includes rejections and an automatically aborted test. The next milestones are the completion of Epoch 1 on 11 October 2026, verifiable compute and capacity probes, and — only after Governance approval — carefully limited public test access.

---

*Esslinger Mesh Whitepaper v1 · 4 October 2026 · © Sven Normen Eßlinger, Esslinger Consulting. All figures are taken from project documents and live read-only checks on 4 October 2026 (~03:05 CEST). Times are Central European Summer Time (CEST, UTC+2).*
