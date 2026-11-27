# Environment Variables

## Authoring toolchain

| Variable | Purpose |
|---|---|
| `AIX_CONTENT_ROOT` | Canonical content directory override; defaults to the working directory's `skills/` or `content/` |

## Desktop app

The desktop app reads provider credentials from its XDG config file
(`~/.config/aix/config.json`, mode 0600 on unix). Environment variables serve as fallback:

| Variable | Purpose |
|---|---|
| `ANTHROPIC_API_KEY` | Anthropic provider credential fallback |
| `OPENAI_API_KEY` | OpenAI provider credential fallback |
| `OPENROUTER_API_KEY` | OpenRouter provider credential fallback |
| `QWEN_API_KEY` | Qwen provider credential fallback |

Never commit populated `.env` files or API keys.
