//! The report as a workbook to work with: a summary that adds up the detail
//! sheet with formulas, one sheet per breakdown, the detail lines flat
//! enough for a pivot table, and the notes that say what the numbers mean.
//!
//! Cells hold numbers and dates, never numbers written as text: a total
//! someone has to retype is not a deliverable. Every formula carries the
//! value it comes to, for readers that do not calculate.

use imprenta_core::color::Color;
use imprenta_core::units::Edges;
use imprenta_xlsx::ir::{Cell, Column, Freeze, Merge, Row, Sheet, Value, Workbook};
use imprenta_xlsx::style::{Across, Alignment, Border, Down, Font, Line, Points, Style};
use imprenta_xlsx::{serial, write};

use super::text::T;
use super::{avg_latency, change, Group, Report};
use crate::error::{AppError, AppResult};
use crate::services::usage::Totals;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
const INK: Color = rgb(15, 23, 42);
const MUTED: Color = rgb(100, 116, 139);
const ACCENT: Color = rgb(30, 64, 175);
const HEAD_FILL: Color = rgb(230, 236, 246);
const TOTAL_FILL: Color = rgb(244, 246, 250);

pub fn render(r: &Report) -> AppResult<Vec<u8>> {
    let s = Styles::new(r);
    let mut sheets = Vec::new();
    // The detail goes first in the arithmetic: the summary sums it.
    let detail = r.has("lines").then(|| lines(r, &s));
    sheets.push(summary(r, &s, detail.as_ref()));
    for (section, _, sheet, detail) in r.breakdowns() {
        let groups = r.groups(section);
        if !groups.is_empty() {
            sheets.push(breakdown(r, &s, r.t(sheet), &groups, section, detail));
        }
    }
    if r.has("daily") || r.has("trend") {
        sheets.push(daily(r, &s));
    }
    if let Some(detail) = detail {
        sheets.push(detail.sheet);
    }
    if r.has("requests") {
        sheets.push(requests(r, &s));
    }
    if r.has("rates") {
        sheets.push(rates(r, &s));
    }
    sheets.push(about(r, &s));
    write(&Workbook::new(sheets), &[]).map_err(|e| AppError::Internal(format!("xlsx report: {e}")))
}

/// The formats a report's cells use, built once.
struct Styles {
    title: Style,
    subtitle: Style,
    label: Style,
    head: Style,
    head_number: Style,
    text: Style,
    wrap: Style,
    money: Style,
    rate: Style,
    int: Style,
    decimal: Style,
    percent: Style,
    date: Style,
    datetime: Style,
    total_text: Style,
    total_money: Style,
    total_int: Style,
    total_decimal: Style,
    total_percent: Style,
}

impl Styles {
    fn new(r: &Report) -> Self {
        let money_format = |decimals: &str| match r.currency {
            "USD" => format!("\"$\"#,##0.{decimals}"),
            "EUR" => format!("#,##0.{decimals} \"€\""),
            other => format!("#,##0.{decimals} \"{other}\""),
        };
        let format = |f: &str| Style {
            format: Some(f.into()),
            ..Style::default()
        };
        let total = |base: &Style| Style {
            font: Font {
                bold: true,
                color: Some(INK),
                ..Font::default()
            },
            fill: Some(TOTAL_FILL),
            border: Edges {
                top: Some(Border {
                    style: Line::Thin,
                    color: Some(INK),
                }),
                ..Edges::default()
            },
            ..base.clone()
        };
        let head = Style {
            font: Font {
                bold: true,
                color: Some(INK),
                ..Font::default()
            },
            fill: Some(HEAD_FILL),
            border: Edges {
                bottom: Some(Border {
                    style: Line::Thin,
                    color: Some(ACCENT),
                }),
                ..Edges::default()
            },
            align: Alignment {
                vertical: Some(Down::Middle),
                wrap: true,
                ..Alignment::default()
            },
            ..Style::default()
        };
        // Money keeps up to four places: a request costs fractions of a cent.
        let money = format(&money_format("00##"));
        let int = format("#,##0");
        let decimal = format("#,##0");
        let percent = format("0.0%");
        Self {
            title: Style {
                font: Font {
                    bold: true,
                    size: Some(Points(16.0)),
                    color: Some(INK),
                    ..Font::default()
                },
                ..Style::default()
            },
            subtitle: Style {
                font: Font {
                    color: Some(MUTED),
                    ..Font::default()
                },
                ..Style::default()
            },
            label: Style {
                font: Font {
                    bold: true,
                    color: Some(MUTED),
                    ..Font::default()
                },
                ..Style::default()
            },
            head_number: Style {
                align: Alignment {
                    horizontal: Some(Across::Right),
                    ..head.align.clone()
                },
                ..head.clone()
            },
            head,
            text: Style::default(),
            wrap: Style {
                align: Alignment {
                    wrap: true,
                    vertical: Some(Down::Top),
                    ..Alignment::default()
                },
                ..Style::default()
            },
            rate: format(&money_format("0000")),
            date: format("yyyy-mm-dd"),
            datetime: format("yyyy-mm-dd hh:mm:ss"),
            total_text: total(&Style::default()),
            total_money: total(&money),
            total_int: total(&int),
            total_decimal: total(&decimal),
            total_percent: total(&percent),
            money,
            int,
            decimal,
            percent,
        }
    }
}

fn with(value: Value, style: &Style) -> Cell {
    Cell {
        value,
        style: Some(Box::new(style.clone())),
    }
}

fn text(value: impl Into<String>, style: &Style) -> Cell {
    with(Value::Text(value.into()), style)
}

fn number(value: f64, style: &Style) -> Cell {
    with(Value::Number(value), style)
}

fn int(value: i64, s: &Styles) -> Cell {
    number(value as f64, &s.int)
}

fn day(value: &str, s: &Styles) -> Cell {
    match chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .and_then(|d| {
            use chrono::Datelike;
            serial::from_ymd(d.year(), d.month(), d.day())
        }) {
        Some(serial) => with(Value::Date(serial), &s.date),
        None => text(value, &s.text),
    }
}

/// `A`, `B`, … `Z`, `AA`: the letter of a zero-based column.
fn letter(index: usize) -> String {
    let mut n = index + 1;
    let mut out = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.insert(0, (b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    out
}

/// A header row the autofilter hangs from.
fn head_row(heads: &[(String, bool)], s: &Styles) -> Row {
    Row {
        cells: heads
            .iter()
            .map(|(h, numeric)| text(h, if *numeric { &s.head_number } else { &s.head }))
            .collect(),
        height: Some(30.0),
        filter: true,
        style: None,
    }
}

fn widths(widths: &[f64]) -> Vec<Column> {
    widths
        .iter()
        .map(|w| Column {
            width: Some(*w),
            style: None,
        })
        .collect()
}

/// The sheet the summary sums, and where its amount column is.
struct Detail {
    sheet: Sheet,
    amount_column: usize,
    cost_column: Option<usize>,
    rows: usize,
}

// ── summary ─────────────────────────────────────────────────────────────

fn summary(r: &Report, s: &Styles, detail: Option<&Detail>) -> Sheet {
    let lang = r.lang;
    let mut rows = vec![
        Row::new(vec![text(r.t(T::Kind), &s.title)]).styled(Style::default()),
        Row::new(vec![text(r.period(), &s.subtitle)]),
        Row::new(vec![text(r.t(T::NotInvoice), &s.subtitle)]),
        Row::default(),
    ];
    rows[0].height = Some(24.0);
    let meta = |label: T, value: Cell| Row::new(vec![text(r.t(label), &s.label), value]);
    let dash = || "—".to_string();
    rows.push(meta(
        T::Client,
        text(r.client.clone().unwrap_or_else(dash), &s.text),
    ));
    rows.push(meta(
        T::Reference,
        text(r.reference.clone().unwrap_or_else(dash), &s.text),
    ));
    rows.push(meta(T::From, day(&r.usage.from.to_string(), s)));
    rows.push(meta(T::To, day(&r.usage.to.to_string(), s)));
    rows.push(meta(T::Currency, text(r.currency, &s.text)));
    if r.currency != "USD" {
        rows.push(meta(
            T::ExchangeRate,
            number(
                r.rate,
                &Style {
                    format: Some("0.0000".into()),
                    ..Style::default()
                },
            ),
        ));
    }
    if r.splits_cost() {
        rows.push(meta(
            T::Markup,
            number(
                r.markup_percent() / 100.0,
                &Style {
                    format: Some("0.00%".into()),
                    ..Style::default()
                },
            ),
        ));
    }
    for (label, value) in &r.scope {
        rows.push(meta(
            if *label == T::Scope { T::Scope } else { *label },
            text(value, &s.text),
        ));
    }
    rows.push(meta(T::PreparedBy, text(&r.author, &s.text)));
    rows.push(meta(
        T::Generated,
        with(
            Value::Date(serial::from_unix_ms(r.generated.timestamp_millis() as f64)),
            &s.datetime,
        ),
    ));
    rows.push(meta(T::ReportId, text(&r.id, &s.text)));
    rows.push(meta(T::Fingerprint, text(&r.fingerprint, &s.text)));
    rows.push(Row::default());

    rows.push(head_row(
        &[
            (r.t(T::Indicator).into(), false),
            (r.t(T::ThisPeriod).into(), true),
            (r.t(T::PreviousPeriod).into(), true),
            (r.t(T::Change).into(), true),
        ],
        s,
    ));
    rows.last_mut().unwrap().filter = false;
    let (now, before) = (&r.usage.totals, &r.usage.previous);
    let kpi = |label: T, current: Cell, previous: f64, previous_style: &Style, c: f64| {
        Row::new(vec![
            text(r.t(label), &s.text),
            current,
            number(previous, previous_style),
            match change(c, previous) {
                Some(delta) => number(delta, &s.percent),
                None => Cell::blank(),
            },
        ])
    };
    // A total that is a formula over the detail sheet, with its value.
    let summed = |column: Option<usize>, value: f64| match (detail, column) {
        (Some(d), Some(col)) if d.rows > 0 => with(
            Value::formula_worth(
                format!(
                    "SUM('{}'!{c}2:{c}{})",
                    r.t(T::SheetLines),
                    d.rows + 1,
                    c = letter(col)
                ),
                value,
            ),
            &s.total_money,
        ),
        _ => number(value, &s.total_money),
    };
    if r.splits_cost() {
        let cost = r.cost(now.cost_nanos);
        rows.push(kpi(
            T::ProviderCost,
            summed(detail.and_then(|d| d.cost_column), cost),
            r.cost(before.cost_nanos),
            &s.money,
            cost,
        ));
        let margin = r.amount(now.cost_nanos) - cost;
        rows.push(kpi(
            T::Margin,
            number(margin, &s.money),
            r.amount(before.cost_nanos) - r.cost(before.cost_nanos),
            &s.money,
            margin,
        ));
    }
    let amount = r.amount(now.cost_nanos);
    rows.push(kpi(
        T::TotalAmount,
        summed(detail.map(|d| d.amount_column), amount),
        r.amount(before.cost_nanos),
        &s.money,
        amount,
    ));
    for (label, current, previous) in [
        (T::Requests, now.requests, before.requests),
        (
            T::FailedRequests,
            now.failed_requests,
            before.failed_requests,
        ),
        (T::InputTokens, now.prompt_tokens, before.prompt_tokens),
        (T::CacheReads, now.cached_tokens, before.cached_tokens),
        (
            T::CacheWrites,
            now.cache_write_tokens,
            before.cache_write_tokens,
        ),
        (
            T::OutputTokens,
            now.completion_tokens,
            before.completion_tokens,
        ),
        (T::Reasoning, now.reasoning_tokens, before.reasoning_tokens),
    ] {
        rows.push(kpi(
            label,
            int(current, s),
            previous as f64,
            &s.int,
            current as f64,
        ));
    }
    let latency = avg_latency(now).unwrap_or(0.0);
    rows.push(kpi(
        T::LatencyMs,
        number(latency, &s.decimal),
        avg_latency(before).unwrap_or(0.0),
        &s.decimal,
        latency,
    ));
    let _ = lang;

    Sheet {
        name: r.t(T::SheetSummary).into(),
        columns: widths(&[30.0, 26.0, 20.0, 14.0]),
        merges: (0..3)
            .map(|row| Merge {
                from_row: row,
                from_column: 0,
                to_row: row,
                to_column: 3,
            })
            .collect(),
        rows,
        ..Sheet::default()
    }
}

// ── breakdowns ──────────────────────────────────────────────────────────

/// The numeric columns every breakdown and the daily sheet share, after
/// their name columns.
fn metric_heads(r: &Report) -> Vec<(String, bool)> {
    let mut heads: Vec<(String, bool)> = [
        T::Requests,
        T::FailedRequests,
        T::InputTokens,
        T::CacheReads,
        T::CacheWrites,
        T::OutputTokens,
        T::Reasoning,
        T::LatencyMs,
    ]
    .iter()
    .map(|t| (r.t(*t).to_string(), true))
    .collect();
    if r.splits_cost() {
        heads.push((r.t(T::ProviderCost).into(), true));
        heads.push((r.t(T::Margin).into(), true));
    }
    heads.push((r.t(T::Amount).into(), true));
    heads
}

fn metric_cells(r: &Report, s: &Styles, t: &Totals) -> Vec<Cell> {
    let mut cells = vec![
        int(t.requests, s),
        int(t.failed_requests, s),
        int(t.prompt_tokens, s),
        int(t.cached_tokens, s),
        int(t.cache_write_tokens, s),
        int(t.completion_tokens, s),
        int(t.reasoning_tokens, s),
        number(avg_latency(t).unwrap_or(0.0), &s.decimal),
    ];
    if r.splits_cost() {
        let cost = r.cost(t.cost_nanos);
        cells.push(number(cost, &s.money));
        cells.push(number(r.amount(t.cost_nanos) - cost, &s.money));
    }
    cells.push(number(r.amount(t.cost_nanos), &s.money));
    cells
}

/// A total row: `SUM` over each summable column, with the sum.
fn total_row(
    r: &Report,
    s: &Styles,
    leading: usize,
    first: usize,
    last: usize,
    t: &Totals,
) -> Vec<Cell> {
    let sum = |col: usize, value: f64, style: &Style| {
        with(
            Value::formula_worth(format!("SUM({c}{first}:{c}{last})", c = letter(col)), value),
            style,
        )
    };
    let mut cells = vec![text(r.t(T::Total), &s.total_text)];
    for _ in 1..leading {
        cells.push(text("", &s.total_text));
    }
    let mut col = leading;
    for value in [
        t.requests,
        t.failed_requests,
        t.prompt_tokens,
        t.cached_tokens,
        t.cache_write_tokens,
        t.completion_tokens,
        t.reasoning_tokens,
    ] {
        cells.push(sum(col, value as f64, &s.total_int));
        col += 1;
    }
    // An average does not add up: it is worked out from the totals.
    cells.push(number(avg_latency(t).unwrap_or(0.0), &s.total_decimal));
    col += 1;
    if r.splits_cost() {
        let cost = r.cost(t.cost_nanos);
        cells.push(sum(col, cost, &s.total_money));
        cells.push(sum(col + 1, r.amount(t.cost_nanos) - cost, &s.total_money));
        col += 2;
    }
    cells.push(sum(col, r.amount(t.cost_nanos), &s.total_money));
    cells
}

fn accumulate(sum: &mut Totals, t: &Totals) {
    sum.cost_nanos += t.cost_nanos;
    sum.requests += t.requests;
    sum.failed_requests += t.failed_requests;
    sum.prompt_tokens += t.prompt_tokens;
    sum.completion_tokens += t.completion_tokens;
    sum.cached_tokens += t.cached_tokens;
    sum.cache_write_tokens += t.cache_write_tokens;
    sum.reasoning_tokens += t.reasoning_tokens;
    sum.latency_ms += t.latency_ms;
}

fn breakdown(
    r: &Report,
    s: &Styles,
    name: &str,
    groups: &[Group],
    section: &str,
    detail: Option<T>,
) -> Sheet {
    let first = r.t(match section {
        "teams" => T::Team,
        "keys" => T::Key,
        "people" => T::Person,
        "models" => T::Model,
        "customers" => T::Customer,
        _ => T::Tag,
    });
    let mut heads = vec![(first.to_string(), false)];
    if let Some(detail) = detail {
        heads.push((r.t(detail).into(), false));
    }
    let leading = heads.len();
    heads.extend(metric_heads(r));
    heads.push((r.t(T::Share).into(), true));

    let total_nanos: i64 = groups.iter().map(|g| g.totals.cost_nanos).sum();
    let mut rows = vec![head_row(&heads, s)];
    let mut sum = Totals::default();
    for g in groups {
        accumulate(&mut sum, &g.totals);
        let mut cells = vec![text(&g.name, &s.text)];
        if detail.is_some() {
            cells.push(text(g.detail.clone().unwrap_or_default(), &s.text));
        }
        cells.extend(metric_cells(r, s, &g.totals));
        cells.push(if total_nanos > 0 {
            number(g.totals.cost_nanos as f64 / total_nanos as f64, &s.percent)
        } else {
            Cell::blank()
        });
        rows.push(Row::new(cells));
    }
    if !groups.is_empty() {
        let mut cells = total_row(r, s, leading, 2, groups.len() + 1, &sum);
        cells.push(number(1.0, &s.total_percent));
        rows.push(Row::new(cells));
    }
    let mut cols = vec![36.0];
    if detail.is_some() {
        cols.push(24.0);
    }
    cols.extend(std::iter::repeat_n(14.0, heads.len() - cols.len()));
    Sheet {
        name: name.into(),
        columns: widths(&cols),
        rows,
        freeze: Some(Freeze {
            rows: 1,
            columns: 1,
        }),
        ..Sheet::default()
    }
}

fn daily(r: &Report, s: &Styles) -> Sheet {
    let mut heads = vec![(r.t(T::Date).to_string(), false)];
    heads.extend(metric_heads(r));
    heads.push((r.t(T::RunningTotal).into(), true));
    let mut rows = vec![head_row(&heads, s)];
    let mut sum = Totals::default();
    for d in &r.usage.daily {
        accumulate(&mut sum, &d.totals);
        let mut cells = vec![day(&d.day, s)];
        cells.extend(metric_cells(r, s, &d.totals));
        cells.push(number(r.amount(sum.cost_nanos), &s.money));
        rows.push(Row::new(cells));
    }
    let n = r.usage.daily.len();
    if n > 0 {
        let mut cells = total_row(r, s, 1, 2, n + 1, &sum);
        cells.push(text("", &s.total_text));
        rows.push(Row::new(cells));
    }
    let mut cols = vec![13.0];
    cols.extend(std::iter::repeat_n(14.0, heads.len() - 1));
    Sheet {
        name: r.t(T::SheetDaily).into(),
        columns: widths(&cols),
        rows,
        freeze: Some(Freeze {
            rows: 1,
            columns: 1,
        }),
        ..Sheet::default()
    }
}

/// One row per day, key and model, and no total row: a total inside the
/// data is counted twice by every pivot table built on it.
fn lines(r: &Report, s: &Styles) -> Detail {
    let mut heads: Vec<(String, bool)> = [
        (T::Date, false),
        (T::Team, false),
        (T::Key, false),
        (T::KeyHint, false),
        (T::Person, false),
        (T::Model, false),
    ]
    .iter()
    .map(|(t, n)| (r.t(*t).to_string(), *n))
    .collect();
    let leading = heads.len();
    heads.extend(metric_heads(r));
    let amount_column = heads.len() - 1;
    let cost_column = r.splits_cost().then(|| amount_column - 2);
    let mut rows = vec![head_row(&heads, s)];
    for l in &r.lines {
        let t = Totals {
            cost_usd: 0.0,
            cost_nanos: l.cost_nanos,
            requests: l.requests,
            failed_requests: l.failed_requests,
            prompt_tokens: l.prompt_tokens,
            completion_tokens: l.completion_tokens,
            cached_tokens: l.cached_tokens,
            cache_write_tokens: l.cache_write_tokens,
            reasoning_tokens: l.reasoning_tokens,
            latency_ms: l.latency_ms,
        };
        let mut cells = vec![
            day(&l.day, s),
            text(
                r.team_of(l.team_name.as_deref(), l.owner.as_deref()),
                &s.text,
            ),
            text(r.key_of(l.key_name.as_deref(), l.key_id), &s.text),
            text(
                l.last4
                    .as_deref()
                    .map(|l| format!("sk-...{l}"))
                    .unwrap_or_default(),
                &s.text,
            ),
            text(l.person_name.clone().unwrap_or_default(), &s.text),
            text(&l.model_name, &s.text),
        ];
        cells.extend(metric_cells(r, s, &t));
        rows.push(Row::new(cells));
    }
    let mut cols = vec![12.0, 26.0, 24.0, 13.0, 18.0, 22.0];
    cols.extend(std::iter::repeat_n(14.0, heads.len() - leading));
    Detail {
        rows: r.lines.len(),
        amount_column,
        cost_column,
        sheet: Sheet {
            name: r.t(T::SheetLines).into(),
            columns: widths(&cols),
            rows,
            freeze: Some(Freeze {
                rows: 1,
                columns: 1,
            }),
            ..Sheet::default()
        },
    }
}

fn requests(r: &Report, s: &Styles) -> Sheet {
    let mut heads: Vec<(String, bool)> = [
        (T::Time, false),
        (T::RequestId, false),
        (T::Team, false),
        (T::Key, false),
        (T::Model, false),
        (T::Provider, false),
        (T::Endpoint, false),
        (T::Status, true),
        (T::Customer, false),
        (T::Tag, false),
        (T::InputTokens, true),
        (T::CacheReads, true),
        (T::CacheWrites, true),
        (T::OutputTokens, true),
        (T::Reasoning, true),
        (T::Latency, true),
    ]
    .iter()
    .map(|(t, n)| (r.t(*t).to_string(), *n))
    .collect();
    if r.splits_cost() {
        heads.push((r.t(T::ProviderCost).into(), true));
    }
    heads.push((r.t(T::Amount).into(), true));
    let mut rows = vec![head_row(&heads, s)];
    for q in &r.requests {
        let tags: Vec<String> = q
            .tags_json
            .as_deref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default();
        let mut cells = vec![
            with(
                Value::Date(serial::from_unix_ms(q.created_at.timestamp_millis() as f64)),
                &s.datetime,
            ),
            text(&q.request_id, &s.text),
            text(r.team_of(q.team_name.as_deref(), None), &s.text),
            text(r.key_of(q.key_name.as_deref(), q.key_id), &s.text),
            text(&q.model_name, &s.text),
            text(&q.provider, &s.text),
            text(&q.endpoint, &s.text),
            int(i64::from(q.status_code), s),
            text(q.end_user.clone().unwrap_or_default(), &s.text),
            text(tags.join(", "), &s.text),
            int(q.prompt_tokens, s),
            int(q.cached_tokens, s),
            int(q.cache_write_tokens, s),
            int(q.completion_tokens, s),
            int(q.reasoning_tokens, s),
            int(q.latency_ms, s),
        ];
        if r.splits_cost() {
            cells.push(number(r.cost(q.cost_nanos), &s.money));
        }
        cells.push(number(r.amount(q.cost_nanos), &s.money));
        rows.push(Row::new(cells));
    }
    let mut cols = vec![19.0, 30.0, 22.0, 22.0, 22.0, 12.0, 18.0, 8.0, 18.0, 16.0];
    cols.extend(std::iter::repeat_n(13.0, heads.len() - cols.len()));
    Sheet {
        name: r.t(T::SheetRequests).into(),
        columns: widths(&cols),
        rows,
        freeze: Some(Freeze {
            rows: 1,
            columns: 0,
        }),
        ..Sheet::default()
    }
}

fn rates(r: &Report, s: &Styles) -> Sheet {
    let heads: Vec<(String, bool)> = [
        (T::Model, false),
        (T::Provider, false),
        (T::PriceSource, false),
        (T::InputRate, true),
        (T::OutputRate, true),
        (T::CacheReadRate, true),
        (T::CacheWriteRate, true),
        (T::ReasoningRate, true),
    ]
    .iter()
    .map(|(t, n)| (r.t(*t).to_string(), *n))
    .collect();
    let mut rows = vec![head_row(&heads, s)];
    let price = |micros: Option<i64>| match micros {
        Some(m) => number(r.rate_value(m), &s.rate),
        None => Cell::blank(),
    };
    for rate in &r.rates {
        let tokens = rate.pricing.as_ref().map(|p| &p.tokens);
        rows.push(Row::new(vec![
            text(&rate.model, &s.text),
            text(&rate.provider, &s.text),
            text(r.source_name(rate.source), &s.text),
            price(tokens.map(|t| t.input)),
            price(tokens.map(|t| t.output)),
            price(tokens.and_then(|t| t.cache_read)),
            price(tokens.and_then(|t| t.cache_write)),
            price(tokens.and_then(|t| t.reasoning)),
        ]));
    }
    rows.push(Row::default());
    rows.push(Row::new(vec![text(r.t(T::RatesNote), &s.subtitle)]));
    Sheet {
        name: r.t(T::SheetRates).into(),
        columns: widths(&[28.0, 14.0, 18.0, 14.0, 14.0, 14.0, 14.0, 14.0]),
        rows,
        freeze: Some(Freeze {
            rows: 1,
            columns: 1,
        }),
        ..Sheet::default()
    }
}

fn about(r: &Report, s: &Styles) -> Sheet {
    let mut rows = vec![
        Row::new(vec![text(r.t(T::Methodology), &s.title)]),
        Row::default(),
    ];
    for paragraph in r.methodology() {
        rows.push(Row::new(vec![text(paragraph, &s.wrap)]));
    }
    if let Some(cap) = r.requests_capped {
        rows.push(Row::new(vec![text(
            match r.lang {
                super::Lang::En => {
                    format!("The requests sheet lists the first {cap} requests of the period.")
                }
                super::Lang::Es => format!(
                    "La hoja de peticiones recoge las primeras {cap} peticiones del periodo."
                ),
            },
            &s.wrap,
        )]));
    }
    if let Some(notes) = &r.notes {
        rows.push(Row::default());
        rows.push(Row::new(vec![text(r.t(T::Notes), &s.label)]));
        rows.push(Row::new(vec![text(notes, &s.wrap)]));
    }
    rows.push(Row::default());
    rows.push(Row::new(vec![text(r.t(T::ColumnGuide), &s.label)]));
    for (column, focus) in [
        (T::Date, "ChargePeriodStart"),
        (T::Currency, "BillingCurrency"),
        (T::Model, "SkuId"),
        (T::Provider, "ServiceName"),
        (T::Requests, "ConsumedQuantity (x_Requests)"),
        (T::InputTokens, "ConsumedQuantity (x_InputTokens)"),
        (T::OutputTokens, "ConsumedQuantity (x_OutputTokens)"),
        (T::ProviderCost, "EffectiveCost"),
        (T::Amount, "BilledCost"),
        (T::Team, "x_Team"),
        (T::Key, "x_ApiKey"),
    ] {
        rows.push(Row::new(vec![text(
            format!("{} → {focus}", r.t(column)),
            &s.text,
        )]));
    }
    Sheet {
        name: r.t(T::SheetAbout).into(),
        columns: widths(&[110.0]),
        rows,
        ..Sheet::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_lettered_like_excel() {
        assert_eq!(letter(0), "A");
        assert_eq!(letter(25), "Z");
        assert_eq!(letter(26), "AA");
        assert_eq!(letter(27), "AB");
    }

    #[test]
    fn a_full_workbook_writes() {
        for lang in [super::super::Lang::En, super::super::Lang::Es] {
            let report = super::super::tests::sample(lang, 500, true);
            let bytes = render(&report).unwrap();
            assert!(bytes.starts_with(b"PK"));
            if let Ok(dir) = std::env::var("REPORT_PREVIEW_DIR") {
                std::fs::write(format!("{dir}/report-{lang:?}.xlsx"), bytes).unwrap();
            }
        }
    }
}
