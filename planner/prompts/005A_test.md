Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 005A: Write tests for agent-to-bridge streaming notifications

Context (read-only inputs):
- Spec: planner/spec.md (do not edit)
- Human hints: planner/human.md (follow constraints)
- Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
- Agents can send session/update notifications through NotificationSender during prompt execution
- Notifications are relayed from agent through bridge to CT-WEB without modification
- Test the actual streaming mechanism that makes existing failing tests pass

Scope & files:
- Target area: tests/bridge_handshake.rs - enhance FakeStreamingAgentTransport to actually use NotificationSender
- You may create/modify only test files and light test scaffolding.
- Focus on making the existing failing tests pass by implementing agent-side streaming behavior in test doubles

What to deliver:
1. Modify FakeStreamingAgentTransport to actually call notification_sender.send_notification() with session/update messages
2. Tests should exercise the full flow: prompt request → agent sends notifications → bridge relays to CT-WEB
3. Keep changes minimal and focused on streaming mechanism

Notes & logging (append-only):
- Append observations, decisions, and evidence to planner/notes/005A_test.md using `§ TEST` blocks with timestamps.
- Include the commands you ran, failing output snippets, and links to any generated artifacts.
- Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:
```
cargo test
```

Constraints:
- Do not change application code.
- Only modify test agent implementations to use the NotificationSender
- Keep test names descriptive (<module>: <behavior>).

Exit condition:
- The 2 currently failing streaming tests should start passing because agent test doubles now simulate proper streaming behavior.