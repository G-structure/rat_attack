#+BEGIN_SRC text
#+TITLE: RAT Remote Control via Local Websockets — Software Design Specification
#+SUBTITLE: Version 1.2.0
#+AUTHOR: RAT2E Working Group
#+DATE: 2025-09-18
#+OPTIONS: toc:3 num:t ^:nil
#+LANGUAGE: en
#+PROPERTY: header-args :results none :exports code
[BCP 14](https://www.rfc-editor.org/info/bcp14) [RFC 7322](https://datatracker.ietf.org/doc/html/rfc7322)

* Status of This Document
This is a living engineering specification intended for internal and partner implementation; it uses normative keywords per BCP 14 and distinguishes Normative vs Informative sections and is not an IETF standard. [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119) [RFC 8174](https://www.rfc-editor.org/info/bcp14) [RFC 7322](https://datatracker.ietf.org/doc/html/rfc7322)

* Abstract
This specification defines a local-first control path where a browser-based WebUI implemented in SolidJS connects to one or more host bridges over WebSockets and transports ACP messages as JSON-RPC for session management, file viewing and editing, terminal operations, and optional MCP proxying. A mobile-first WebUI coordinates multiple CT-BRIDGEs simultaneously, each brokering one or more ACP agents. Authentication flows requiring a browser click are executed in an embedded WebView that is tunneled through a local HTTP reverse proxy in the CT-BRIDGE, ensuring that egress traffic originates from the same host/IP as the CT-AGENT so provider-side “same device/IP” checks pass reliably. [RFC 6455](https://datatracker.ietf.org/doc/html/rfc6455) [JSON-RPC 2.0](https://www.jsonrpc.org/specification) [ACP Overview](https://agentclientprotocol.com/overview/introduction) [SolidJS Docs](https://docs.solidjs.com/)

* Terminology (Normative)
- CT-WEB: Browser WebUI (SolidJS) that speaks ACP over WebSocket(s) and renders UX.
- CT-BRIDGE: Local WS server/bridge on a host that forwards ACP JSON-RPC to agent processes and implements client-side methods (fs, permissions, terminal, auth proxy).
- CT-AGENT: ACP agent process, typically spawned by a bridge and speaking ACP over stdio.
- Subprotocol: The ~Sec-WebSocket-Protocol~ token used during upgrade to indicate ACP framing (~acp.jsonrpc.v1~).
- Project Root (PR): Absolute directory boundary used for file/terminal sandboxing.
- BridgeId (BRID): Stable, opaque identifier for a CT-BRIDGE instance (e.g., UUIDv4), unique across a CT-WEB session.
- Session Address: Tuple ~(BRID, SessionId)~ used to route ACP messages to the correct bridge and agent session.
- Auth WebView Proxy (AWP): The CT-BRIDGE’s embedded HTTP(S) reverse proxy used by CT-WEB to render third-party auth pages while guaranteeing egress from the bridge/agent host IP.

* Scope
Interfaces, behaviors, and conformance for CT-WEB, one or more CT-BRIDGEs, and CT-AGENTs operating over WS with JSON-RPC/ACP, including initialization, sessions, prompt streaming, permissioning, ACP-based file editor, terminal, optional MCP proxying, authentication UX with bridge-hosted webview proxy, multi-bridge multiplexing, and debug tooling.

* Conformance
** Conformance Targets
- CT-WEB: SolidJS WebUI that upgrades to one or more WS connections and speaks ACP as a client with bridge multiplexing.
- CT-BRIDGE: WS server that validates upgrades per-connection, forwards ACP, implements client FS/permissions/terminal, provides AWP, may spawn agents and proxy MCP.
- CT-AGENT: ACP agent implementing agent methods and streaming updates.

** Conformance Levels
“Compliant” satisfies all MUST requirements; “Conditionally Compliant” may omit features labeled OPTIONAL or MAY.

** Requirement IDs
Normative requirements are labeled ~RAT-LWS-REQ-###~. New requirements introduced for authentication AWP and multi-bridge are numbered ~200+~ and ~300+~ respectively.

* System Overview (Informative)
Multi-bridge topology: one CT-WEB connects to N CT-BRIDGEs; each bridge manages zero or more CT-AGENTs. Auth flows that require clicking a provider link happen inside CT-WEB’s embedded WebView, which loads a local AWP URL on the target bridge. The AWP reverse proxies the provider domain so outbound TLS originates from the bridge host, aligning the perceived IP with the agent’s host.

#+BEGIN_mermaid
flowchart LR
  subgraph WEB["CT-WEB (SolidJS, Mobile-first)"]
    UI["Chat • Plan • Diffs • Terminal • Editor • Auth WebView"]
    WS1["WS 'acp.jsonrpc.v1' → BRIDGE A"]
    WS2["WS 'acp.jsonrpc.v1' → BRIDGE B"]
  end
  subgraph BRA["CT-BRIDGE A"]
    UPa["Upgrade + Origin"]
    FWDa["ACP JSON-RPC Forwarder"]
    FSa["Client FS"]
    PERMa["Permissions"]
    TERMa["Terminal"]
    MCPa["MCP Proxy"]
    AWPa["Auth WebView Proxy (reverse proxy)"]
    SPAWNa["Agent Launcher"]
    AGa["CT-AGENT(s) over stdio"]
  end
  subgraph BRB["CT-BRIDGE B"]
    UPb["Upgrade + Origin"]
    FWDb["ACP JSON-RPC Forwarder"]
    AWpb["Auth WebView Proxy"]
    AGb["CT-AGENT(s)"]
  end
  UI --> WS1 --> UPa --> FWDa <--> AGa
  UI --> WS2 --> UPb --> FWDb <--> AGb
  UI -. Auth WebView .-> AWPa -->|"egress from Bridge A IP"| Provider[(Auth Provider)]
#+END_mermaid

* Transport and Admission (Normative)
- =RAT-LWS-REQ-001 (Upgrade + Origin)= Each CT-BRIDGE MUST implement an HTTP/1.1 WebSocket upgrade, validate ~Origin~ against an allow-list, and reply 403/426 on failure. [RFC 6455] [RFC 9110]
- =RAT-LWS-REQ-002 (Subprotocol Echo)= Each CT-BRIDGE MUST echo exactly one offered subprotocol token; token is ~acp.jsonrpc.v1~.
- =RAT-LWS-REQ-003 (Compression)= Bridges SHOULD NOT negotiate ~permessage-deflate~ by default; deployments MAY enable with sizing controls. [RFC 7692]
- =RAT-LWS-REQ-004 (No Cookies)= CT-WEB/CT-BRIDGE MUST NOT rely solely on ambient cookies for WS auth (mitigates CSWSH).
- =RAT-LWS-REQ-005 (Close Codes)= Bridges SHOULD use 1000 (normal), 1008 (policy), 1011 (internal), 1013 (try again later).

* Subprotocol and Message Model (Normative)
- =RAT-LWS-REQ-010 (Subprotocol)= Implementations MUST offer/echo ~acp.jsonrpc.v1~.
- =RAT-LWS-REQ-011 (JSON-RPC Transparency)= Frames MUST carry UTF-8 JSON via JSON-RPC 2.0 with unmodified ACP method/notification names; ids MUST correlate responses. [JSON-RPC 2.0]

* Initialization and Capabilities (Normative)
- =RAT-LWS-REQ-020 (initialize)= CT-WEB MUST send ~initialize~ per connection declaring ~fs.readTextFile~ and ~fs.writeTextFile~ when the editor is enabled; MAY declare ~terminal:true~.
- =RAT-LWS-REQ-021 (Agent Caps)= CT-AGENT SHOULD advertise ~loadSession~ when supported so CT-WEB can resume via ~session/load~.
- =RAT-LWS-REQ-022 (Auth Advertise)= If the agent requires auth, CT-WEB MUST complete ~authenticate~ before ~session/new~.

* Multi-Bridge Model (Normative)
- =RAT-LWS-REQ-300 (Bridge Identity)= Each CT-BRIDGE MUST expose a stable ~bridgeId~ (BRID, opaque string) in ~initialize.response._meta.bridgeId~.
- =RAT-LWS-REQ-301 (Connection Multiplex)= CT-WEB MUST support multiple concurrent WS connections to distinct CT-BRIDGEs. Each ACP request/notification MUST be routed on the WS connection that corresponds to the intended BRID.
- =RAT-LWS-REQ-302 (Session Addressing)= CT-WEB MUST address sessions as ~(BRID, SessionId)~. All session-scoped methods (e.g., ~session/prompt~, ~session/cancel~) MUST be sent on the connection for ~BRID~ that created the ~SessionId~.
- =RAT-LWS-REQ-303 (Isolation)= PR sandboxing, permission policies, terminals, and MCP servers are scoped to a BRID. CT-WEB MUST NOT mix resources across BRIDs.
- =RAT-LWS-REQ-304 (Reconnect)= CT-WEB SHOULD reconnect each BRID independently with exponential backoff + jitter; unsent user input MUST be preserved per-bridge.
- =RAT-LWS-REQ-305 (Discovery)= CT-WEB MAY allow static configuration of bridge endpoints, mDNS/Bonjour discovery on LAN, or manual entry; discovery MUST NOT auto-connect without explicit user consent.

* Sessions & Prompt Turn (Normative)
- =RAT-LWS-REQ-030 (Create/Load)= CT-WEB MUST use ~session/new~ and SHOULD use ~session/load~. Agents MUST replay ~session/update~ history on load.
- =RAT-LWS-REQ-031 (Streaming)= During ~session/prompt~, agents MUST stream ~session/update~ chunks until a final result with ~stopReason~. CT-WEB MUST render incrementally.
- =RAT-LWS-REQ-032 (Cancel)= CT-WEB MAY send ~session/cancel~; agents MUST conclude with ~stopReason:"cancelled"~; CT-WEB MUST auto-respond ~cancelled~ to pending permission requests.

* ACP Semantics Clarifications (Normative)
- Plans replace in full on each update; CT-WEB MUST re-render.
- Tool call statuses are finite and monotonic; CT-WEB MUST enforce valid transitions (see State Machine).
- File ~locations[]~ MUST use absolute paths and 1-based line numbers.

* File Editor over ACP (Normative)
- =RAT-LWS-REQ-040 (Read)= Bridges MUST implement ~fs/read_text_file~ with optional ~line/limit~.
- =RAT-LWS-REQ-041 (Write + Approvals)= Bridges MUST implement ~fs/write_text_file~ and gate writes via ~session/request_permission~.
- =RAT-LWS-REQ-042 (Editor UX)= CT-WEB MUST provide an editor view supporting open-by-path, diff preview, and approval-gated writes.
- =RAT-LWS-REQ-044 (Sandbox)= Reads/writes MUST be restricted to declared PRs; OOB access rejected.

* Terminal Extension (Normative)
- =RAT-LWS-REQ-060 (Capability)= If ~terminal:true~ is advertised, Bridges MAY expose terminal methods; CT-WEB MAY render a terminal panel.
- =RAT-LWS-REQ-062 (Approval)= Command execution MUST be permission-gated; working directory MUST be a PR or descendant.
- =RAT-LWS-REQ-063 (PTY + Stream)= When enabled, terminal output MUST stream via notifications and conclude with an exit code.

* MCP Proxying (Optional, Normative Where Implemented)
- =RAT-LWS-REQ-070 (Advertise)= CT-WEB MAY pass ~mcpServers[]~ in ~session/new~. CT-BRIDGE MAY operate as an MCP proxy for agents.
- =RAT-LWS-REQ-071 (Permission Boundary)= CT-BRIDGE MUST enforce the same permission model for MCP-sourced tool calls as for native ones (no bypass).
- =RAT-LWS-REQ-072 (Translation)= When proxying, responses MUST be converted to ACP content blocks faithfully; errors MUST include provenance.

* Authentication — WebView with Same-Host/IP Guarantee (Normative)
** Rationale (Informative)
Some providers bind CLI device authentication to the IP (and sometimes TLS/device characteristics) observed when the user clicks a link. If a mobile browser opens the link directly, the provider may see a different IP than the agent’s host and fail device binding. The AWP ensures egress originates from the bridge/agent host.

** Requirements
- =RAT-LWS-REQ-200 (AWP Existence)= Bridges MUST provide an Auth WebView Proxy (AWP): a local HTTP(S) reverse proxy endpoint that CT-WEB can load inside an embedded WebView to conduct provider flows.
- =RAT-LWS-REQ-201 (Egress from Bridge Host)= For proxied requests, the ultimate outbound TCP/TLS connection to the provider MUST originate on the bridge host using the system network stack (no upstream browser mediation). This provides the “same IP” as the CT-AGENT host.
- =RAT-LWS-REQ-202 (Target Restriction)= AWP MUST restrict navigation targets to a configured allow-list of hostnames (or exact URLs) provided by the bridge (e.g., OAuth/Console domains). Requests to non-allow-listed domains MUST be blocked with an explanatory page.
- =RAT-LWS-REQ-203 (Origin & CSRF)= AWP MUST set a strict ~Content-Security-Policy~ for its chrome pages, implement anti-open-redirect checks (deny ~next=~ style parameters unless allow-listed), and strip/deny ~Referer~ leakage from AWP chrome to third parties.
- =RAT-LWS-REQ-204 (State & Cookies)= Each AWP flow MUST have an ephemeral ~authSessionId~ with an isolated cookie jar, storage partition, and redirect state; jars MUST be destroyed when the flow completes or times out.
- =RAT-LWS-REQ-205 (Callback Handoff)= When a provider indicates completion (e.g., local HTTP callback, page content containing a code/token, or polling), AWP MUST extract the code/token server-side and deliver it to the agent/bridge over the local channel (not via WebView JS). Sensitive tokens MUST NOT be exposed to CT-WEB JS.
- =RAT-LWS-REQ-206 (TLS & HSTS)= AWP MUST not MITM TLS. It MUST perform origin-preserving reverse proxying: the TLS connection is bridge→provider; client→AWP is local HTTP(S). HSTS preload expectations must be honored; certificate validation MUST occur in AWP’s HTTP client.
- =RAT-LWS-REQ-207 (Windowing)= CT-WEB MUST embed AWP in an in-app WebView. Pop-ups requested by the provider MUST be handled by opening additional AWP-scoped views with the same ~authSessionId~.
- =RAT-LWS-REQ-208 (Link Entrypoint)= CT-WEB MUST request an AWP entry URL from the bridge (~POST /auth_webview/start {target_url, provider_hint}~). The bridge MAY issue a 302 to the actual provider target through its proxy path (~/_awp/{authSessionId}/...~).
- =RAT-LWS-REQ-209 (Progress Events)= AWP SHOULD stream server-sent events (SSE) or WS to CT-WEB with coarse status: ~starting~, ~waiting_user~, ~received_code~, ~exchanging_token~, ~complete~, ~error~. These are UX hints only.
- =RAT-LWS-REQ-210 (Same-Device Binding)= If the agent exposes a device binding token (e.g., CLI displays ~device_code~), AWP SHOULD attach it (via headers or query) when navigating to provider pages, if documented safe by the provider.
- =RAT-LWS-REQ-211 (Privacy Controls)= AWP MUST redact sensitive query params and headers from logs; any debugging mode MUST be opt-in and scrub secrets.

** Example AWP Flow (Informative)
1) Agent returns an auth link in ~authenticate~ response. 2) CT-WEB calls ~POST /auth_webview/start~ on the bridge (body: target URL, allow-listed). 3) Bridge creates ~authSessionId~, cookie jar, and returns ~http://bridge/_awp/{id}/~. 4) CT-WEB opens the WebView at that local URL; AWP 302’s to the provider and reverse proxies subsequent requests. 5) Provider completes; AWP extracts ~code~, exchanges for tokens (if required), and delivers credentials to the agent via an internal bridge channel. 6) AWP signals ~complete~; CT-WEB dismisses WebView and re-attempts ~initialize~ or ~session/new~.

* Permission Model (Normative)
- =RAT-LWS-REQ-090 (Protocol Flow)= Permission prompts via ~session/request_permission~ with ~(tool_call, options[])~; CT-WEB MUST present options and return a definitive outcome or ~cancelled~.
- =RAT-LWS-REQ-091 (Option Kinds)= CT-WEB MUST support ~allow_once~, ~allow_always~, ~reject_once~, ~reject_always~; decisions MAY be persisted client-side per-BRID.
- =RAT-LWS-REQ-092 (Policy Registry)= Each bridge MUST maintain a deny/allow policy (tool kind, absolute path globs, command patterns). Default deny unless explicitly allowed.
- =RAT-LWS-REQ-093 (Global “Always Allow”)= CT-WEB MAY expose a user-owned “always allow tool actions” switch (default false), stored per device, not project-local.
- =RAT-LWS-REQ-094 (Audit Trail)= Bridges SHOULD log permission prompts/outcomes with redaction.

* Tool Call Lifecycle — State Machine (Normative)
States: ~pending~ → ~waiting_for_confirmation~ → ~in_progress~ → (~completed~ | ~failed~ | ~rejected~ | ~cancelled~)
Transitions are monotone; CT-WEB MUST render distinct states and apply updates atomically. Cancellation propagates to ~waiting_for_confirmation~ entries as ~cancelled~.

* Mobile-First WebUI Requirements (Normative)
- =RAT-LWS-REQ-100 (Small Screens)= CT-WEB MUST support ≤ 414px width with responsive layout: sticky session header, bottom-sheet permission dialogs, and collapsible tool cards.
- =RAT-LWS-REQ-101 (Streaming UX)= Agent text MUST stream into a virtualized list to avoid layout jank; tool cards MUST update in place.
- =RAT-LWS-REQ-102 (Editor on Mobile)= Editor view MUST support soft-wrap, read-only diff preview, and explicit “Apply” buttons; keyboard overlays MUST not occlude approval actions.
- =RAT-LWS-REQ-103 (Per-Bridge Reconnect)= Independent reconnect per BRID with jitter; local drafts preserved.

* Security Considerations (Normative)
Origin validation; cookie-only WS auth forbidden; PMCE off by default; PR sandbox for fs/terminal; execute requires approval; canonicalize and deny symlink escapes; redact secrets. For AWP: allow-list only, no TLS MITM, isolated cookie jars, no general proxy mode, and strict CSP for chrome. For multi-bridge: each BRID maintains its own policy store and PR set.

* Interoperability Notes (Informative)
Architecture mirrors modern editor/agent patterns (JSON-RPC over stdio/WS, ACP protocol) while introducing AWP to satisfy provider same-IP/device expectations and first-class multi-bridge multiplexing for multi-host/mobile workflows.

* Bridge Responsibilities (Informative)
Per-connection admission, ACP forwarding, FS/permissions/terminal, AWP, optional MCP proxy, agent lifecycle (spawn/monitor/exit), per-BRID metrics and logs.

* Agent Launching (Informative)
Bridges spawn agents from configured commands (path/args/env) within a PR context. Stdio pipes are bound to the ACP transport. On agent exit, bridges propagate errors to CT-WEB and close sessions. Where agents expose CLI logins, bridges can also expose a convenience ~/auth/cli_login~ that prints guidance while recommending AWP for link flows.

* WebUI Responsibilities (Informative)
Manage multiple WS connections (one per bridge), correlate BRID/SessionId, prompt composition, streaming rendering, diff/terminal viewers, permission dialogs, AWP WebView lifecycle, a per-BRID switcher in the UI chrome, and an ACP I/O debug view.

* File Editor Behavior Details (Normative)
- =RAT-LWS-REQ-110 (Large Files)= Bridges MAY truncate reads over a threshold and MUST signal truncation (~meta.truncated:true~).
- =RAT-LWS-REQ-111 (Binary/Encoding)= Bridges MUST reject binary files for ~read_text_file~; unknown encodings MUST return clear errors.
- =RAT-LWS-REQ-112 (Unsaved Buffers)= If CT-WEB maintains unsaved buffers, it SHOULD prefer client state or mark remote as “disk view”.
- =RAT-LWS-REQ-113 (Diff Apply)= Diffs MUST preview; committing uses the same approval path as writes.

* Terminal Behavior Details (Normative)
- =RAT-LWS-REQ-120 (Output Caps)= Returned summaries MAY be capped (e.g., 16 KiB) while the live terminal streams fully; truncation MUST be labeled.
- =RAT-LWS-REQ-121 (Env/Dir)= Commands MUST run within PR; sanitized environment required. Network policy is deployment-defined and SHOULD default conservative.

* MCP Integration Details (Informative)
Bridges can enumerate configured MCP servers and offer them in ~session/new~; ACP↔MCP translation preserves content types; permission model remains enforced on the bridge boundary; failures include provenance for troubleshooting.

* Authentication UX Patterns (Informative)
- Method picker based on ~authMethods[]~.
- Link-click flows use AWP; AWP pages run inside CT-WEB WebView with reverse-proxied navigation and isolated cookies. This preserves the agent host IP and aligns device binding.
- CLI-only flows may use a hidden terminal with progress streaming; CT-WEB provides an informational panel.
- On success, CT-WEB re-attempts ~initialize~ or ~session/new~ against the same BRID.

* Observability & Debugging (Normative)
- =RAT-LWS-REQ-130 (Metrics)= Bridges SHOULD emit ~ws_open~, ~bytes_{rx,tx}~, ~fs_{read,write}~, ~perm_{asked,granted,denied}~, ~term_{started,exited}~, ~agent_{spawned,exited}~, ~awp_{sessions,complete,error}~ with BRID labels.
- =RAT-LWS-REQ-131 (ACP Log)= CT-WEB SHOULD offer a developer view showing ACP traffic per BRID, directionally, with secret redaction.
- =RAT-LWS-REQ-132 (Structured Errors)= JSON-RPC errors MUST include code/message and MAY include ~data.details~.

* Performance Targets (Informative)
Local WS attach p50 ≤ 300 ms per BRID; small read p50 ≤ 50 ms; permission latency dominated by user; AWP navigation cost bounded by provider RTT; streaming latency bounded by WS framing and agent cadence.

* Protocol Details (Normative)
** WebSocket Attach (per-BRID)
Clients connect with ~Origin~ allow-listed; servers echo ~acp.jsonrpc.v1~ or close with 1008.
** ACP over JSON-RPC
Method names/payloads follow ACP docs; ids unique per sender; notifications do not elicit responses.
** Editor Methods
~fs/read_text_file~, ~fs/write_text_file~; absolute paths; JSON-RPC error on failures.
** Permission Flow
~session/request_permission~ drives approvals; CT-WEB returns selected option or ~cancelled~.
** Terminal (when enabled)
Line-streamed notifications; exit code captured and displayed.
** AWP Control Plane
~POST /auth_webview/start~ → ~{ authSessionId, entryUrl }~; ~GET /_awp/{id}/…~ reverse-proxies allow-listed hosts; ~GET /_awp/{id}/events~ streams status; ~DELETE /_awp/{id}~ tears down jars.

* UI Element Requirements (Normative)
- =RAT-LWS-REQ-140 (Chat)= Stream user/agent chunks; maintain per-session transcript per BRID.
- =RAT-LWS-REQ-141 (Plan)= Display plans with replacements.
- =RAT-LWS-REQ-142 (Tool Calls)= Render tool calls with ~diff~/~terminal~/~locations[]~.
- =RAT-LWS-REQ-143 (Editor)= Open/diff/write via ACP FS with approval gating.
- =RAT-LWS-REQ-144 (Permission Dialog)= Render option kinds clearly.
- =RAT-LWS-REQ-145 (Bridge Switcher)= Provide an affordance to select active BRID, show connection state badges, and filter transcripts/resources per BRID.
- =RAT-LWS-REQ-146 (Auth WebView UX)= Render AWP WebView as a sheet/modal with provider logo, progress status, domain display, and a “Done” action that is disabled until ~complete~.

* Configuration (Informative)
** Bridge (example TOML)
#+begin_src toml
[server]
bind = "127.0.0.1:8137"
origin_allow = ["http://localhost:5173", "http://127.0.0.1:*"]
subprotocol = "acp.jsonrpc.v1"
compression = false

[project_roots]
roots = ["/abs/path/to/projectA", "/abs/path/to/projectB"]

[permissions]
allow_globs = ["${PR}/docs/**"]
deny_globs  = ["${PR}/.env", "${PR}/.ssh/**"]

[terminal]
enabled = true
summary_kib = 16

[auth_webview_proxy]
allow_hosts = ["auth.example.com", "console.example.com"]
idle_timeout_sec = 900
max_sessions = 4

[agents.claude]
command = "/usr/local/bin/claude-code-acp"

[agents.gemini]
command = "/usr/local/bin/gemini"
args    = ["--experimental-acp"]
#+end_src

** WebUI (client storage keys)
- ~rat.bridges[]: { name, url, brid? }~
- ~rat.alwaysAllowToolActions:boolean~ (per device, default false)
- ~rat.lastBridgeId:string~, ~rat.lastSessionByBridge: { BRID → SessionId }~

* Test Plan (Normative)
** Admission
- Correct Origin → 101; incorrect → 403/1008 per bridge.
** Initialize (per-BRID)
- FS caps advertised; version compat; negative tests.
** Sessions (per-BRID)
- ~session/new~, ~session/prompt~ streaming, ~session/cancel~ → ~cancelled~.
** Editor (per-BRID)
- Read/write; approval; OOB rejection; truncation signal; binary rejection.
** Terminal (per-BRID)
- Approval, streamed output, exit code; PR enforcement.
** MCP (optional)
- Routed tool call; permission enforced; error provenance.
** AWP
- Start with allow-listed domain → success; non-allow-listed → blocked page.
- Token/code capture server-side; no exposure to CT-WEB JS; flow timeout cleanup.
- Pop-up window within the same ~authSessionId~.
- After ~complete~, ~initialize/session/new~ succeeds without further auth.
** Multi-Bridge
- Two bridges connected; independent reconnect; simultaneous prompts; isolation of PRs and settings; independent metrics labeled by BRID.
** Security
- CSWSH blocked; symlink escape blocked; cookie-only WS auth forbidden; logs redacted.

* Requirements Traceability Matrix (RTM) (Normative)
| Req ID         | Target        | Verification                                                     | Status |
|----------------+---------------+------------------------------------------------------------------+--------|
| RAT-LWS-REQ-001| BRIDGE        | Origin allow-list enforced                                       | Must   |
| RAT-LWS-REQ-002| BRIDGE        | Subprotocol echoed                                               | Must   |
| RAT-LWS-REQ-003| BRIDGE        | PMCE disabled by default                                         | Should |
| RAT-LWS-REQ-004| WEB/BRIDGE    | No cookie-only WS auth                                           | Must   |
| RAT-LWS-REQ-005| BRIDGE        | Close codes                                                      | Should |
| RAT-LWS-REQ-010| All           | Subprotocol “acp.jsonrpc.v1”                                     | Must   |
| RAT-LWS-REQ-011| All           | JSON-RPC transparency                                            | Must   |
| RAT-LWS-REQ-020| WEB           | ~initialize~ advertises FS caps                                  | Must   |
| RAT-LWS-REQ-021| AGENT         | ~loadSession~ advertised when supported                          | Should |
| RAT-LWS-REQ-022| WEB           | Authenticate if required                                         | Must   |
| RAT-LWS-REQ-030| WEB/AGENT     | ~session/new~ & ~session/load~ semantics                         | Must   |
| RAT-LWS-REQ-031| AGENT         | ~session/update~ streaming                                       | Must   |
| RAT-LWS-REQ-032| AGENT/WEB     | cancel → ~stopReason:"cancelled"~ + pending perms cancelled      | Must   |
| RAT-LWS-REQ-040| BRIDGE        | ~fs/read_text_file~                                              | Must   |
| RAT-LWS-REQ-041| BRIDGE        | ~fs/write_text_file~ approval-gated                              | Must   |
| RAT-LWS-REQ-042| WEB           | Editor UX open/diff/write                                        | Must   |
| RAT-LWS-REQ-044| BRIDGE        | PR sandbox                                                       | Must   |
| RAT-LWS-REQ-060| BRIDGE        | Terminal capability                                              | May    |
| RAT-LWS-REQ-062| BRIDGE        | Terminal execute approval-gated                                  | Must   |
| RAT-LWS-REQ-063| BRIDGE        | PTY streams via notifications; exit code returned                | Must   |
| RAT-LWS-REQ-070| BRIDGE        | MCP proxy (if enabled)                                           | May    |
| RAT-LWS-REQ-071| BRIDGE        | MCP tool calls use same permissions                              | Must   |
| RAT-LWS-REQ-072| BRIDGE        | MCP proxy translates to ACP content blocks;errors have provenance| Must   |
| RAT-LWS-REQ-090| WEB           | ~request_permission~ round-trip                                  | Must   |
| RAT-LWS-REQ-091| WEB           | Four option kinds                                                | Must   |
| RAT-LWS-REQ-092| BRIDGE        | Policy registry default deny                                     | Must   |
| RAT-LWS-REQ-093| WEB           | Global always-allow per device                                   | May    |
| RAT-LWS-REQ-094| BRIDGE        | Audit trail (redacted)                                           | Should |
| RAT-LWS-REQ-100| WEB           | Mobile responsiveness ≤414px                                     | Must   |
| RAT-LWS-REQ-101| WEB           | Streaming virtualization                                         | Should |
| RAT-LWS-REQ-102| WEB           | Mobile editor behaviors                                          | Must   |
| RAT-LWS-REQ-103| WEB           | Per-BRID reconnect/backoff, drafts preserved                     | Should |
| RAT-LWS-REQ-110| BRIDGE        | Large file truncation signal                                     | Should |
| RAT-LWS-REQ-111| BRIDGE        | Binary/encoding handling                                         | Must   |
| RAT-LWS-REQ-112| WEB/BRIDGE    | Unsaved buffer policy                                            | Should |
| RAT-LWS-REQ-113| WEB           | Diff apply requires approval                                     | Must   |
| RAT-LWS-REQ-120| BRIDGE        | Terminal summary caps labeled                                    | Should |
| RAT-LWS-REQ-121| BRIDGE        | PR-scoped exec & env sanitation                                  | Must   |
| RAT-LWS-REQ-130| BRIDGE        | Metrics emitted (incl. AWP, BRID labels)                         | Should |
| RAT-LWS-REQ-131| WEB           | ACP log view per BRID                                            | Should |
| RAT-LWS-REQ-132| All           | Structured errors with ~data.details~                            | Should |
| RAT-LWS-REQ-140| WEB           | Chat streaming per BRID                                          | Must   |
| RAT-LWS-REQ-141| WEB           | Plan rendering                                                   | Must   |
| RAT-LWS-REQ-142| WEB           | Tool call rendering                                              | Must   |
| RAT-LWS-REQ-143| WEB           | Editor via ACP FS                                                | Must   |
| RAT-LWS-REQ-144| WEB           | Permission dialog UX                                             | Must   |
| RAT-LWS-REQ-145| WEB           | Bridge switcher UI                                               | Must   |
| RAT-LWS-REQ-146| WEB           | Auth WebView UX                                                  | Must   |
| RAT-LWS-REQ-200| BRIDGE        | AWP provided                                                     | Must   |
| RAT-LWS-REQ-201| BRIDGE        | AWP egress originates from bridge host                           | Must   |
| RAT-LWS-REQ-202| BRIDGE        | AWP target allow-list                                            | Must   |
| RAT-LWS-REQ-203| BRIDGE        | AWP CSP + anti-redirect + referer controls                       | Must   |
| RAT-LWS-REQ-204| BRIDGE        | AWP ephemeral ~authSessionId~ & isolated cookies                 | Must   |
| RAT-LWS-REQ-205| BRIDGE        | Server-side token/callback capture; no JS exposure               | Must   |
| RAT-LWS-REQ-206| BRIDGE        | No TLS MITM; validate provider TLS                               | Must   |
| RAT-LWS-REQ-207| WEB/BRIDGE    | Multi-window handling within same auth session                   | Should |
| RAT-LWS-REQ-208| WEB/BRIDGE    | AWP entrypoint API                                               | Must   |
| RAT-LWS-REQ-209| BRIDGE        | AWP status events                                                | Should |
| RAT-LWS-REQ-210| BRIDGE        | Device binding token attachment (if safe)                        | Should |
| RAT-LWS-REQ-211| BRIDGE        | Privacy redaction in logs                                        | Must   |
| RAT-LWS-REQ-300| BRIDGE        | ~bridgeId~ exposed                                               | Must   |
| RAT-LWS-REQ-301| WEB           | Multiple concurrent WS connections (multi-bridge)                | Must   |
| RAT-LWS-REQ-302| WEB           | Session addressing ~(BRID, SessionId)~                           | Must   |
| RAT-LWS-REQ-303| WEB/BRIDGE    | Isolation of PRs/policies/terminals per BRID                     | Must   |
| RAT-LWS-REQ-304| WEB           | Independent reconnect per BRID                                   | Should |
| RAT-LWS-REQ-305| WEB           | Explicit opt-in discovery/attach                                 | Should |

* Versioning and Change Management (Normative)
Semantic versioning; wire-compatible additions bump MINOR; breaking wire changes bump MAJOR. New AWP/multi-bridge fields are additive in ~initialize._meta~ and control APIs.

* Internationalization & Accessibility (Informative)
All UI text UTF-8 and externalized; screen-reader and keyboard accessible dialogs; high-contrast themes; AWP sheets include accessible titles, provider domain annunciation, and focus management.

* References (Informative)
ACP (overview/initialization/prompt turn/file system), JSON-RPC 2.0, RFC 6455/7692/9110, SolidJS docs.

* Change Log (Informative)
- 1.2.0: Added multi-bridge architecture and requirements; defined Auth WebView Proxy (AWP) with same-host/IP egress guarantee; expanded security model, UI mandates, metrics, protocol details, and RTM. Clarified addressing ~(BRID, SessionId)~ and added per-BRID isolation rules.
- 1.1.0: Mobile-first UX rules, permission state machine, PR sandboxing, MCP hooks, terminal/editor details, RTM.
- 1.0.0: Baseline WS + ACP control plane, FS editor, permissions.
#+END_SRC
