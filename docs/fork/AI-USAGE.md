# Uso de IA — Windows preview

Open **Uso de IA** in Whispera Settings, or from the tray menu. This view shows
account quotas for OpenAI/Codex and Claude, independently of dictation and Groq.
It preserves the reading shortcut and playback controls from readaloud.3.

## OpenAI / Codex

Install the official Windows Codex app and sign in with ChatGPT. Whispera resolves
its executable from the verified Store package and starts a short-lived
`codex app-server --listen stdio://` process in its own empty data directory.
It sends only `initialize`, `initialized`, and `account/rateLimits/read`, then
terminates that helper. It does not restart the desktop app, create a conversation,
make an inference request, inject UI, or copy tokens/authentication files.

The panel displays all reported quota buckets and primary/secondary windows.
Window lengths are taken from the response; they are not assumed to be fixed.
Remaining quota is `max(0, min(100, 100 - usedPercent))`. Credits, when reported,
are shown separately in the provider's units, not assumed to be US dollars.
This is not a complete meter of every ChatGPT feature or API billing account.

While the view is visible, Codex is refreshed about every five minutes. Manual
refresh is throttled to at most once per 30 seconds. Errors preserve the previous
successful observation and mark it as previous data; unavailable values stay empty.

## Claude Code

Choose **Conectar Claude Code** once. This explicitly configures the user
`statusLine` in the Windows `~/.claude/settings.json` (or `CLAUDE_CONFIG_DIR` inherited
by Whispera). A copy of the previous settings is saved beside that file. The prior
`statusLine` is recorded so **Desconectar Claude Code** can restore it while keeping
unrelated later settings edits. If another application changes that field, Whispera
refuses to overwrite it during disconnection.

The connector installs a small .NET Framework helper in Whispera's user data
directory and uses an encoded PowerShell launcher to handle Windows paths safely.
Claude passes its documented status-line JSON on stdin. The helper keeps only the
recognized quota windows, percentages, reset times, optional gateway spend amounts,
and local reception time. It drops session IDs, transcript paths, model information,
directories and all other fields. It makes no network requests. Nothing is collected
until the user connects it and Claude actually runs that status line.

Start a Claude Code CLI session **on Windows**, with the desired Claude account,
and send a message. Subscription quota fields are normally present for Pro/Max
after a response. CLI, VS Code and the desktop app share the same account quota
when signed in to the same account, but VS Code/app interfaces may not execute a
terminal status line. They therefore do not guarantee live dashboard updates.

**Actualizar cuotas** rereads the last Claude snapshot; it does not force Claude's
servers to return new data. The timestamp is reception from Claude Code, not proof
of a fresh server lookup. It can reflect cached quota data from that session.
WSL, remote environments, project overrides, custom config directories not inherited
by Whispera, organization hook restrictions and different accounts require their
own connector configuration; the automatic connector targets the Windows user.
The official usage link is available when no compatible data source is running.

The optional gateway spend window is labeled separately. Subscription percentages
and monetary balances are never added together. No general Claude API-key billing
or extra-credit balance is inferred from subscription percentages.

## Freshness, privacy and validation

The interface marks observations older than five minutes as previous data. Once a
window's reset passes, its remaining percentage becomes unknown until another
observation arrives. No reset automatically implies 100% remaining.

OpenAI observations remain in memory; Claude's sanitized latest snapshot remains
on this computer until disconnection. No quota history, prompts or conversations
are sent to a Whispera server. The existing transcription/clipboard behavior remains
governed by its own explicit settings. Disconnect Claude before uninstalling if you
want to remove its status-line connection; closing Whispera alone leaves the local
collector available for future sessions.

Focused validation includes native parsing/missing-data tests, reversible settings
tests in temporary directories, collector privacy fixtures, and the dashboard's
loading/error/expired states, actions and narrow layout with mocked IPC. A live
provider query and an actual Claude CLI delivery are distinct from fixture tests.

Official integration references:
- https://learn.chatgpt.com/docs/app-server
- https://code.claude.com/docs/en/statusline
