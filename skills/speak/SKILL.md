---
name: speak
description: Play spoken audio on the host machine. Use whenever the user would benefit from hearing something aloud.
---

# Voice Alerts

You can speak text aloud on the user's machine to proactively alert them.

## When to use

When the user asks you to speak, read aloud, remind them by voice, or anytime voice output is more natural than text.

## How to use

**Detect your environment first**, then use the right method:

```bash
if [ -d /workspace/ipc ]; then
  # NanoClaw container — write IPC file
  cat > /workspace/ipc/commands/speak-$(date +%s).json << 'EOF'
{"type": "speak", "text": "该开会了"}
EOF
else
  # Claude Code or local — call directly
  agent-speak "该开会了"
fi
```

## Design principle

**Keep it simple.** Don't specify engine or voice — the user has configured their default. Just send the text. Language is auto-detected.

Only specify `voice` if the user explicitly asks:

```bash
agent-speak -v Samantha "Hello"
```

Or in IPC:

```json
{"type": "speak", "text": "Hello", "voice": "Samantha"}
```

## Combining with messages

For important alerts, speak AND send a text message so the user has a written record.
