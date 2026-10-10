---
"@llmtrack/server": patch
---

Keys can now keep request and response bodies, off by default. Turn it on per key and set how many days to keep them (empty keeps them until deleted); streamed replies are folded into one, inline images and files are stored as their size only, and an hourly task purges expired bodies and those of deleted keys. Bodies live apart from usage and billing. In the dashboard, Logs shows each request's content and exports the filtered requests as JSON Lines, either every field with both bodies or as an OpenAI chat dataset for fine-tuning and evals. The bundled price list is refreshed.
