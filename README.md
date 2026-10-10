<div align="center">
  <img alt="gray-whatsapp" src="assets/icon.svg" width="120" height="120" />
  <h1>gray-whatsapp</h1>
  <p><strong>WhatsApp Business Cloud API — send texts, plus a local webhook receiver for inbound.</strong></p>
  <p>
    <a href="https://gray.alignment.id">Website</a> ·
    <a href="https://github.com/vstaln/gray-whatsapp">Source</a> ·
    <a href="https://github.com/vstaln/gray">gray</a>
  </p>
  <p>
    <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-1c1c20?style=flat-square&labelColor=0a0a0b" /></a>
    <a href="https://www.rust-lang.org"><img alt="Built with Rust" src="https://img.shields.io/badge/built%20with-rust-1c1c20?style=flat-square&labelColor=0a0a0b&logo=rust&logoColor=d4a373" /></a>
    <img alt="gray plugin" src="https://img.shields.io/badge/gray-plugin-1c1c20?style=flat-square&labelColor=0a0a0b&color=7aa2f7" />
  </p>
</div>

<br/>
```bash
gray plugin install whatsapp
```

`gray-whatsapp` talks to the WhatsApp Business **Cloud API**: outbound
texts via `graph.facebook.com`, inbound via a self-hosted webhook receiver
that writes a local ledger.

## Setup

```sh
whatsapp setup --phone-id <phone_number_id> --token <permanent-token> [--verify <verify-token>]
whatsapp status
```

Config: `~/.gray/whatsapp/config.json` (`{"phone_number_id","token","verify"}`,
written 0600) or env `WA_PHONE_ID` / `WA_TOKEN` / `WA_VERIFY`. Credentials
come from the Meta app dashboard (WhatsApp → API Setup).

## Tools

| Tool | What it does |
|---|---|
| `wa_send` | Send a text — `to` (E.164 digits, no `+`), `text`. |
| `wa_inbox` | Read inbound messages the receiver captured — `limit`. |

## Inbound flow

```sh
whatsapp serve --port 9090
```

runs a small webhook server:

- `GET` answers Meta's verification handshake (`hub.verify_token` must
  match `verify`; default `gray-wa`).
- `POST` appends each inbound text to `~/.gray/whatsapp/inbox.jsonl`,
  which `wa_inbox` reads.

Expose it to Meta with any tunnel (`cloudflared`, `ngrok`, a VPS forward)
and set the tunnel URL + verify token as the app's callback.

## Build from source

```sh
cargo build --release
gray plugin check ./target/release/whatsapp
```

## License

MIT
