---
"@llmtrack/server": minor
"@llmtrack/dashboard": minor
---

Usage reports to bill clients by, as PDF or Excel, from the usage page (`GET /api/usage/export`). The PDF has a cover, linked contents, an executive summary with key points, spend over time, the token mix, a page per breakdown, the day-by-day and detail lines with running totals, and the rates and methodology; the workbook has a summary that adds up the detail sheet with formulas, a sheet per breakdown and the detail flat for pivot tables. Both follow the reader's language and currency, can add a markup over provider cost (shown apart only when asked), and adapt to the page's filters: filtered to a team, the team is the client.
