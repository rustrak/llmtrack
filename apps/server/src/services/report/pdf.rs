//! The report as a printed document: A4, a running header and footer, the
//! headline figures, charts drawn as vectors, then the breakdowns and the
//! lines behind them.
//!
//! A canvas paints in one colour, so a chart of two colours is a row of
//! canvases side by side: the previous period's bars in one, this period's
//! in the next.

use chrono::Datelike;
use imprenta_core::color::Color;
use imprenta_core::units::{Edges, Length, Pt};
use imprenta_pdf::build::{build, Assets};
use imprenta_pdf::ir::{
    self, Align, Anchor, Band, Border, BoxStyle, Canvas, ColumnSpec, Container, Document, Link,
    List, Marker, Node, Numbering, Op, Overflow, PageBreak, PageSetup, Run, Section, SectionPage,
    Spacer, Stroke, Table, Text, TextStyle, Weight,
};
use imprenta_pdf::render::Options;
use imprenta_pdf::shape::Face;

use super::text::T;
use super::{avg_latency, change, plain_output, uncached_input, Group, Report};
use crate::error::{AppError, AppResult};
use crate::services::usage::{Day, Totals};

const REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Geist-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Geist-SemiBold.ttf");

const PAGE_W: f32 = 595.2756;
const PAGE_H: f32 = 841.8898;
const SIDE: f32 = 40.0;
/// The width everything is laid out in.
const W: f32 = PAGE_W - 2.0 * SIDE;
/// Geist's line height, per point of type size.
const LEADING: f32 = 1.2;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
// The dashboard's tokens (`apps/dashboard/src/styles.css`), light theme:
// neutral greys with no tint, and the brand lime as the one accent.
const INK: Color = rgb(17, 17, 17);
const BODY: Color = rgb(55, 55, 55);
const MUTED: Color = rgb(93, 93, 93);
const FAINT: Color = rgb(163, 163, 163);
const RULE: Color = rgb(225, 225, 225);
const PANEL: Color = rgb(248, 248, 248);
const TRACK: Color = rgb(237, 237, 237);
/// `--primary`, light: bars, rules and marks.
const ACCENT: Color = rgb(113, 161, 0);
/// The previous period, and anything else set back: grey, as IBCS draws it.
const PREVIOUS: Color = rgb(212, 212, 212);
/// `--chart-1` to `--chart-5`, in their fixed order.
const SERIES: [Color; 5] = [
    rgb(41, 120, 214),
    rgb(235, 104, 52),
    rgb(27, 175, 122),
    rgb(237, 161, 1),
    rgb(232, 123, 164),
];

const MONO: &[u8] = include_bytes!("../../../assets/fonts/GeistMono-Regular.ttf");
const MONO_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/GeistMono-SemiBold.ttf");

/// Geist Mono, the dashboard's second family: wordmark, labels, identifiers.
fn mono(text: impl Into<String>) -> Run {
    Run::new(text).in_family("mono")
}

/// `llmtrack`, as the dashboard writes it: mono, "track" in lime.
fn wordmark(size: f32, base: Color, lime: Color) -> Node {
    styled(
        vec![
            mono("llm").bold().colored(base),
            mono("track").bold().colored(lime),
        ],
        size,
        base,
        Align::Start,
        0.0,
    )
}

pub fn render(r: &Report) -> AppResult<Vec<u8>> {
    let built = built(&document(r))?;
    for problem in built.diagnostics.iter().filter(|d| !is_clipped(d)) {
        log::warn!("pdf report {}: {problem}", r.id);
    }
    Ok(built.pdf.into_vec())
}

/// Long names are cut with an ellipsis on purpose; the engine notes each.
fn is_clipped(diagnostic: &str) -> bool {
    diagnostic.contains("[text-clipped]")
}

fn assets() -> Assets {
    Assets::new()
        .with_font(Face::REGULAR, REGULAR.to_vec())
        .with_font(Face::BOLD, SEMIBOLD.to_vec())
        .with_font(Face::family("mono"), MONO.to_vec())
        .with_font(Face::family("mono").bold(), MONO_SEMIBOLD.to_vec())
}

fn built(document: &Document) -> AppResult<imprenta_pdf::build::Built> {
    build(document, &assets(), Options::default())
        .map_err(|e| AppError::Internal(format!("pdf report: {e}")))
}

/// One document: the cover and the contents as unnumbered sections of their
/// own (no running header, wider margins), then the chapters, each on a new
/// page. The contents asks the engine where each chapter landed.
fn document(r: &Report) -> Document {
    let parts = parts(r);
    let entries: Vec<_> = parts.iter().flat_map(|p| p.toc.iter().cloned()).collect();
    let mut children = vec![front(cover(r)), front(vec![contents(r, &entries)])];
    for (i, part) in parts.into_iter().enumerate() {
        // The body opens a page after the sections on its own.
        if i > 0 {
            children.push(Node::PageBreak(PageBreak::default()));
        }
        children.extend(part.nodes);
    }
    Document {
        page: PageSetup {
            width: Pt(PAGE_W),
            height: Pt(PAGE_H),
            margin: Edges {
                top: Pt(28.0),
                right: Pt(SIDE),
                bottom: Pt(26.0),
                left: Pt(SIDE),
            },
        },
        header: Some(header(r)),
        footer: Some(footer(r)),
        accumulators: Vec::new(),
        children,
    }
}

/// Front matter: no bands, its own margins, not numbered or counted.
fn front(children: Vec<Node>) -> Node {
    Node::Section(Section {
        page: Some(SectionPage {
            margin: Some(Edges::symmetric(Pt(48.0), Pt(FRONT_SIDE))),
            ..SectionPage::default()
        }),
        header: Some(None),
        footer: Some(None),
        numbering: Numbering::None,
        children,
    })
}

/// Where a numbered heading is, for the contents and the PDF's outline.
fn anchor_id(number: &str) -> String {
    format!("sec-{number}")
}

fn anchor(number: &str, title: &str, level: u8) -> Node {
    Node::Anchor(Anchor {
        id: anchor_id(number),
        bookmark: Some(format!("{number}  {title}")),
        level,
    })
}

/// A run of the body that opens a page, and what the contents lists for it.
struct Part {
    toc: Vec<(u8, String, String)>,
    nodes: Vec<Node>,
}

/// The chapters, in order, numbered as they come.
fn parts(r: &Report) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut n = 0;
    let open = |n: &mut usize, title: T, intro: Option<&str>| {
        *n += 1;
        (
            (0u8, n.to_string(), r.t(title).to_string()),
            chapter(*n, r.t(title), intro),
        )
    };

    if r.has("summary") {
        let (entry, mut nodes) = open(&mut n, T::ExecutiveSummary, Some(r.t(T::SummaryIntro)));
        nodes.extend(summary(r));
        parts.push(Part {
            toc: vec![entry],
            nodes,
        });
    }

    if r.has("trend") || r.has("tokens") {
        let (entry, mut nodes) = open(&mut n, T::Consumption, Some(r.t(T::ConsumptionIntro)));
        let mut toc = vec![entry];
        let mut sub = 0;
        let next = |sub: &mut i32| {
            *sub += 1;
            format!("{n}.{sub}")
        };
        if r.has("trend") {
            let number = next(&mut sub);
            nodes.extend(trend(r, &number));
            toc.push((1, number, trend_title(r).to_string()));
        }
        if r.has("tokens") {
            let number = next(&mut sub);
            nodes.extend(token_mix(r, &number));
            toc.push((1, number, r.t(T::TokenMix).to_string()));
        }
        parts.push(Part { toc, nodes });
    }

    let breakdowns: Vec<_> = r
        .breakdowns()
        .into_iter()
        .map(|(section, title, _, detail)| (r.groups(section), title, detail))
        // A breakdown with nothing in it (no customers, no tags) is left out.
        .filter(|(groups, ..)| !groups.is_empty())
        .collect();
    if !breakdowns.is_empty() {
        let (entry, head) = open(&mut n, T::Breakdown, Some(r.t(T::BreakdownIntro)));
        // One page each: a team's table never shares a page with a model's.
        for (i, (groups, title, detail)) in breakdowns.iter().enumerate() {
            let number = format!("{n}.{}", i + 1);
            let mut toc = Vec::new();
            let mut nodes = Vec::new();
            if i == 0 {
                toc.push(entry.clone());
                nodes.extend(head.iter().cloned());
            }
            toc.push((1, number.clone(), r.t(*title).to_string()));
            nodes.extend(breakdown(r, &number, groups, *title, *detail));
            parts.push(Part { toc, nodes });
        }
    }

    for (section, title, intro, body) in [
        (
            "daily",
            T::DayByDay,
            T::DayByDayNote,
            daily as fn(&Report) -> Vec<Node>,
        ),
        ("lines", T::Lines, T::LinesNote, lines),
        ("requests", T::RequestLog, T::RequestsIntro, requests),
    ] {
        if r.has(section) {
            let (entry, mut nodes) = open(&mut n, title, Some(r.t(intro)));
            nodes.extend(body(r));
            parts.push(Part {
                toc: vec![entry],
                nodes,
            });
        }
    }

    let (entry, mut nodes) = open(&mut n, T::Annex, Some(r.t(T::AnnexIntro)));
    let mut toc = vec![entry];
    let mut sub = 0;
    if r.has("rates") {
        sub += 1;
        let number = format!("{n}.{sub}");
        nodes.extend(rates(r, &number));
        toc.push((1, number, r.t(T::RatesApplied).to_string()));
    }
    sub += 1;
    let number = format!("{n}.{sub}");
    nodes.extend(methodology(r, &number));
    toc.push((1, number, r.t(T::Methodology).to_string()));
    parts.push(Part { toc, nodes });
    parts
}

fn trend_title(r: &Report) -> &'static str {
    r.t(match buckets(&r.usage.daily).0 {
        Bucket::Day => T::SpendPerDay,
        Bucket::Week => T::SpendPerWeek,
        Bucket::Month => T::SpendPerMonth,
    })
}

// ── building blocks ─────────────────────────────────────────────────────

fn styled(runs: Vec<Run>, size: f32, color: Color, align: Align, space_after: f32) -> Node {
    Node::Text(Text {
        runs,
        style: TextStyle {
            size: Pt(size),
            color,
            align,
            space_after: Pt(space_after),
            ..TextStyle::default()
        },
    })
}

fn plain(text: impl Into<String>, size: f32, color: Color) -> Node {
    styled(vec![Run::new(text)], size, color, Align::Start, 0.0)
}

fn aligned(text: impl Into<String>, size: f32, color: Color, align: Align) -> Node {
    styled(vec![Run::new(text)], size, color, align, 0.0)
}

fn spacer(height: f32) -> Node {
    Node::Spacer(Spacer {
        height: Pt(height),
        grow: false,
    })
}

/// A box of a fixed width, which is what places things side by side.
fn column(width: f32, children: Vec<Node>) -> Node {
    Node::Box(Container {
        style: BoxStyle {
            width: Some(Pt(width)),
            ..BoxStyle::default()
        },
        children,
    })
}

fn panel(width: f32, padding: f32, children: Vec<Node>) -> Node {
    Node::Box(Container {
        style: BoxStyle {
            width: Some(Pt(width)),
            background: Some(PANEL),
            radius: Pt(5.0),
            padding: Edges::all(Pt(padding)),
            ..BoxStyle::default()
        },
        children,
    })
}

fn row(children: Vec<Node>) -> Node {
    Node::Row(Container {
        style: BoxStyle::default(),
        children,
    })
}

fn canvas(width: f32, height: f32, ops: Vec<Op>, fill: Option<Color>) -> Node {
    Node::Canvas(Canvas {
        width: Pt(width),
        height: Pt(height),
        ops,
        fill,
        stroke: None,
        space_after: Pt(0.0),
    })
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Op {
    Op::Rect {
        x: Pt(x),
        y: Pt(y),
        w: Pt(w),
        h: Pt(h),
    }
}

/// A square of `color`, centred on a line of `size`-point text.
fn swatch(color: Color, size: f32) -> Node {
    let line = size * LEADING;
    let side = size * 0.9;
    canvas(
        side + 5.0,
        line,
        vec![rect(0.0, (line - side) / 2.0, side, side)],
        Some(color),
    )
}

fn rule(color: Color, width: f32) -> Option<Border> {
    Some(Border {
        width: Pt(width),
        color,
    })
}

/// A numbered section title and its note, kept with what follows them, and
/// an anchor the contents and the outline point at.
fn heading(number: &str, title: &str, note: Option<&str>) -> Vec<Node> {
    let line = |runs: Vec<Run>, size: f32, color: Color, after: f32| {
        Node::Text(Text {
            runs,
            style: TextStyle {
                size: Pt(size),
                color,
                space_after: Pt(after),
                keep_with_next: true,
                ..TextStyle::default()
            },
        })
    };
    let mut out = vec![
        spacer(22.0),
        anchor(number, title, 2),
        line(
            vec![
                mono(format!("{number}  ")).bold().colored(ACCENT),
                Run::new(title).bold(),
            ],
            13.0,
            INK,
            if note.is_some() { 3.0 } else { 8.0 },
        ),
    ];
    if let Some(note) = note {
        out.push(line(vec![Run::new(note)], 7.5, MUTED, 9.0));
    }
    out
}

fn empty(r: &Report) -> Node {
    plain(r.t(T::Empty), 8.0, MUTED)
}

/// Long names cut to fit a label column.
fn clip(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let cut: String = name.chars().take(max - 1).collect();
        format!("{cut}…")
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Body,
    Subtotal,
    Total,
}

/// `None` is a column that takes what is left.
struct Col(Option<f32>, Align);

fn table(cols: &[Col], header: Vec<String>, rows: Vec<(Vec<String>, Kind)>) -> Node {
    const SIZE: f32 = 7.0;
    let cell = |text: String, size: f32, color: Color, weight: Weight| ir::Cell {
        text,
        col_span: None,
        size: Some(Pt(size)),
        color: Some(color),
        weight,
        italic: false,
        family: None,
    };
    let columns = cols
        .iter()
        .map(|Col(width, align)| ColumnSpec {
            width: width.map_or(Length::Auto, |w| Length::Pt(Pt(w))),
            align: *align,
            overflow: if *align == Align::Start {
                Overflow::Ellipsis
            } else {
                Overflow::Wrap
            },
        })
        .collect();
    let header = ir::Row {
        cells: header
            .into_iter()
            .map(|h| cell(h, 6.4, MUTED, Weight::Bold))
            .collect(),
        style: BoxStyle {
            border: Edges {
                bottom: rule(INK, 0.75),
                ..Edges::default()
            },
            ..BoxStyle::default()
        },
        totals: Vec::new(),
    };
    let rows = rows
        .into_iter()
        .map(|(cells, kind)| {
            let (color, weight) = match kind {
                Kind::Body => (BODY, Weight::Regular),
                Kind::Subtotal | Kind::Total => (INK, Weight::Bold),
            };
            let style = match kind {
                Kind::Body => BoxStyle {
                    border: Edges {
                        bottom: rule(RULE, 0.4),
                        ..Edges::default()
                    },
                    ..BoxStyle::default()
                },
                Kind::Subtotal => BoxStyle {
                    background: Some(PANEL),
                    border: Edges {
                        bottom: rule(RULE, 0.4),
                        ..Edges::default()
                    },
                    ..BoxStyle::default()
                },
                Kind::Total => BoxStyle {
                    background: Some(PANEL),
                    border: Edges {
                        top: rule(INK, 0.75),
                        ..Edges::default()
                    },
                    ..BoxStyle::default()
                },
            };
            ir::Row {
                cells: cells
                    .into_iter()
                    .map(|c| cell(c, SIZE, color, weight))
                    .collect(),
                style,
                totals: Vec::new(),
            }
        })
        .collect();
    Node::Table(Table {
        columns,
        header: vec![header],
        rows,
        repeat_header: true,
        padding: Edges::symmetric(Pt(3.2), Pt(4.0)),
        space_after: Pt(4.0),
    })
}

// ── page furniture ──────────────────────────────────────────────────────

fn header(r: &Report) -> Band {
    let right = match &r.client {
        Some(client) => format!("{client} · {}", r.period()),
        None => r.period(),
    };
    Band {
        height: None,
        children: vec![
            Node::Box(Container {
                style: BoxStyle {
                    border: Edges {
                        bottom: rule(RULE, 0.6),
                        ..Edges::default()
                    },
                    padding: Edges {
                        bottom: Pt(6.0),
                        ..Edges::default()
                    },
                    ..BoxStyle::default()
                },
                children: vec![row(vec![
                    column(
                        W * 0.45,
                        vec![styled(
                            vec![
                                mono("llm").bold().colored(INK),
                                mono("track").bold().colored(ACCENT),
                                Run::new(format!("   {}", r.t(T::Kind))).colored(MUTED),
                            ],
                            7.5,
                            MUTED,
                            Align::Start,
                            0.0,
                        )],
                    ),
                    column(W * 0.55, vec![aligned(right, 7.0, MUTED, Align::End)]),
                ])],
            }),
            spacer(14.0),
        ],
    }
}

fn footer(r: &Report) -> Band {
    Band {
        height: None,
        children: vec![
            spacer(10.0),
            Node::Box(Container {
                style: BoxStyle {
                    border: Edges {
                        top: rule(RULE, 0.6),
                        ..Edges::default()
                    },
                    padding: Edges {
                        top: Pt(5.0),
                        ..Edges::default()
                    },
                    ..BoxStyle::default()
                },
                children: vec![row(vec![
                    column(
                        W * 0.75,
                        vec![plain(
                            format!(
                                "{} · {} · {}",
                                r.t(T::Confidential),
                                r.id,
                                r.t(T::NotInvoice)
                            ),
                            6.5,
                            FAINT,
                        )],
                    ),
                    column(
                        W * 0.25,
                        vec![styled(
                            vec![Run::new(r.t(T::PageOf))],
                            6.5,
                            MUTED,
                            Align::End,
                            0.0,
                        )],
                    ),
                ])],
            }),
        ],
    }
}

/// The cover and contents are set inside these margins.
const FRONT_SIDE: f32 = 56.0;
const FRONT_W: f32 = PAGE_W - 2.0 * FRONT_SIDE;

/// The facts every reader of the report wants first, label and value.
fn facts_of(r: &Report) -> Vec<(T, String)> {
    let lang = r.lang;
    let currency = if r.currency == "USD" {
        "USD".to_string()
    } else {
        format!(
            "{} · 1 USD = {} {}",
            r.currency,
            lang.number(r.rate, 4),
            r.currency
        )
    };
    let dash = || "—".to_string();
    vec![
        (T::Client, r.client.clone().unwrap_or_else(dash)),
        (T::Reference, r.reference.clone().unwrap_or_else(dash)),
        (
            T::Period,
            format!("{} – {}", lang.date(r.usage.from), lang.date(r.usage.to)),
        ),
        (T::Currency, currency),
        (T::Scope, scope_line(r)),
        (T::PreparedBy, r.author.clone()),
        (
            T::Generated,
            format!(
                "{} · {}",
                lang.date(r.generated.date_naive()),
                r.generated.format("%H:%M UTC")
            ),
        ),
        (T::ReportId, r.id.clone()),
    ]
}

/// The filters as one line: `Team: Acme · Model: gpt-4o`.
fn scope_line(r: &Report) -> String {
    r.scope
        .iter()
        .map(|(label, value)| {
            if *label == T::Scope {
                value.clone()
            } else {
                format!("{}: {value}", r.t(*label))
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A small label, upper case.
fn label(text: &str, size: f32, color: Color) -> Node {
    styled(
        vec![Run::new(text.to_uppercase())],
        size,
        color,
        Align::Start,
        3.0,
    )
}

/// The cover, Swiss style: white, one large headline, the lime as the only
/// colour, and the facts small and on a four-column grid at the foot.
fn cover(r: &Report) -> Vec<Node> {
    let title = r.client.clone().unwrap_or_else(|| r.t(T::AiUsage).into());
    let col = (FRONT_W - 3.0 * 16.0) / 4.0;
    let grid_row = |cells: Vec<(T, String)>| {
        let mut out = Vec::new();
        for (i, (name, value)) in cells.into_iter().enumerate() {
            if i > 0 {
                out.push(column(16.0, Vec::new()));
            }
            out.push(column(
                col,
                vec![label(r.t(name), 6.5, MUTED), plain(value, 8.5, INK)],
            ));
        }
        Node::Box(Container {
            style: BoxStyle {
                border: Edges {
                    top: rule(RULE, 0.6),
                    ..Edges::default()
                },
                padding: Edges::symmetric(Pt(10.0), Pt(0.0)),
                ..BoxStyle::default()
            },
            children: vec![row(out)],
        })
    };
    let facts = facts_of(r);
    let pick = |names: [T; 4]| {
        names
            .iter()
            .map(|n| {
                facts
                    .iter()
                    .find(|(f, _)| f == n)
                    .cloned()
                    .unwrap_or((*n, String::new()))
            })
            .collect::<Vec<_>>()
    };
    let totals = &r.usage.totals;
    let figure = |name: T, value: String| {
        column(
            col,
            vec![
                label(r.t(name), 6.5, MUTED),
                styled(vec![Run::new(value).bold()], 15.0, INK, Align::Start, 0.0),
            ],
        )
    };

    vec![
        row(vec![
            column(FRONT_W / 2.0, vec![wordmark(12.0, INK, ACCENT)]),
            column(
                FRONT_W / 2.0,
                vec![styled(
                    vec![Run::new(r.t(T::Confidential).to_uppercase())],
                    6.8,
                    MUTED,
                    Align::End,
                    0.0,
                )],
            ),
        ]),
        // Everything after it sits at the foot of the page.
        Node::Spacer(Spacer {
            height: Pt(40.0),
            grow: true,
        }),
        canvas(28.0, 5.0, vec![rect(0.0, 0.0, 28.0, 5.0)], Some(ACCENT)),
        spacer(16.0),
        label(r.t(T::Kind), 8.0, MUTED),
        spacer(6.0),
        styled(
            {
                // The brand's full stop, in lime, as the sign-in page sets it.
                let mut runs = vec![Run::new(title.clone()).bold()];
                if !title.ends_with('.') {
                    runs.push(Run::new(".").bold().colored(ACCENT));
                }
                runs
            },
            40.0,
            INK,
            Align::Start,
            8.0,
        ),
        styled(vec![Run::new(r.period())], 15.0, MUTED, Align::Start, 40.0),
        Node::Box(Container {
            style: BoxStyle {
                border: Edges {
                    top: rule(INK, 1.0),
                    ..Edges::default()
                },
                padding: Edges::symmetric(Pt(12.0), Pt(0.0)),
                ..BoxStyle::default()
            },
            children: vec![row(vec![
                figure(T::TotalAmount, r.money(r.amount(totals.cost_nanos))),
                column(16.0, Vec::new()),
                figure(T::Requests, r.lang.integer(totals.requests)),
                column(16.0, Vec::new()),
                figure(
                    T::Tokens,
                    r.lang
                        .integer(totals.prompt_tokens + totals.completion_tokens),
                ),
                column(16.0, Vec::new()),
                figure(
                    T::AvgLatency,
                    super::avg_latency(totals)
                        .map(|ms| format!("{} ms", r.lang.number(ms, 0)))
                        .unwrap_or_else(|| "—".into()),
                ),
            ])],
        }),
        spacer(30.0),
        grid_row(pick([T::Client, T::Reference, T::Period, T::Currency])),
        grid_row(pick([T::Scope, T::PreparedBy, T::Generated, T::ReportId])),
        Node::Box(Container {
            style: BoxStyle {
                border: Edges {
                    top: rule(RULE, 0.6),
                    ..Edges::default()
                },
                padding: Edges {
                    top: Pt(10.0),
                    ..Edges::default()
                },
                ..BoxStyle::default()
            },
            children: vec![row(vec![
                column(FRONT_W * 0.6, vec![plain(r.t(T::NotInvoice), 6.8, MUTED)]),
                column(
                    FRONT_W * 0.4,
                    vec![aligned(
                        format!("SHA-256 {}…", &r.fingerprint[..16]),
                        6.5,
                        FAINT,
                        Align::End,
                    )],
                ),
            ])],
        }),
    ]
}

/// The contents: chapters and their sections, with the page each opens on.
/// The contents: every chapter and section, with the page the engine put it
/// on, each line a link to it.
fn contents(r: &Report, entries: &[(u8, String, String)]) -> Node {
    let inner = FRONT_W;
    let (number_w, page_w) = (40.0, 50.0);
    let mut children = vec![
        wordmark(10.0, INK, ACCENT),
        spacer(56.0),
        styled(
            vec![Run::new(r.t(T::Contents)).bold()],
            28.0,
            INK,
            Align::Start,
            10.0,
        ),
        canvas(44.0, 3.0, vec![rect(0.0, 0.0, 44.0, 3.0)], Some(ACCENT)),
        spacer(10.0),
        styled(
            vec![Run::new(format!("{} · {}", r.period(), scope_line(r)))],
            9.0,
            MUTED,
            Align::Start,
            26.0,
        ),
    ];
    for (level, number, title) in entries {
        let chapter = *level == 0;
        let (size, color) = if chapter { (11.5, INK) } else { (9.5, BODY) };
        let text = |t: String, align: Align, color: Color| {
            styled(
                vec![if chapter {
                    Run::new(t).bold()
                } else {
                    Run::new(t)
                }],
                size,
                color,
                align,
                0.0,
            )
        };
        let number_cell = if chapter {
            styled(
                vec![mono(format!("{:02}", number.parse::<usize>().unwrap_or(0))).bold()],
                size,
                ACCENT,
                Align::Start,
                0.0,
            )
        } else {
            spacer(0.0)
        };
        let label = if chapter {
            title.clone()
        } else {
            format!("{number}   {title}")
        };
        let line = Node::Box(Container {
            style: BoxStyle {
                border: Edges {
                    bottom: rule(if chapter { RULE } else { PANEL }, 0.6),
                    ..Edges::default()
                },
                padding: if chapter {
                    Edges {
                        top: Pt(14.0),
                        right: Pt(0.0),
                        bottom: Pt(7.0),
                        left: Pt(0.0),
                    }
                } else {
                    Edges::symmetric(Pt(6.0), Pt(0.0))
                },
                ..BoxStyle::default()
            },
            children: vec![row(vec![
                column(number_w, vec![number_cell]),
                column(
                    inner - number_w - page_w,
                    vec![text(label, Align::Start, color)],
                ),
                column(
                    page_w,
                    vec![text(
                        format!("{{{{pageof:{}}}}}", anchor_id(number)),
                        Align::End,
                        if chapter { INK } else { MUTED },
                    )],
                ),
            ])],
        });
        children.push(Node::Link(Link {
            href: format!("#{}", anchor_id(number)),
            child: Box::new(line),
        }));
    }
    Node::Box(Container {
        style: BoxStyle {
            padding: Edges {
                top: Pt(0.0),
                right: Pt(0.0),
                bottom: Pt(0.0),
                left: Pt(0.0),
            },
            ..BoxStyle::default()
        },
        children,
    })
}

/// A chapter opening: its anchor, its number, its title, what it is for.
fn chapter(number: usize, title: &str, intro: Option<&str>) -> Vec<Node> {
    let mut text = vec![styled(
        vec![Run::new(title).bold()],
        20.0,
        INK,
        Align::Start,
        4.0,
    )];
    if let Some(intro) = intro {
        text.push(plain(intro, 9.0, MUTED));
    }
    let head = Node::Box(Container {
        style: BoxStyle {
            keep_with_next: true,
            border: Edges {
                bottom: rule(INK, 1.0),
                ..Edges::default()
            },
            padding: Edges {
                bottom: Pt(12.0),
                ..Edges::default()
            },
            space_after: Pt(6.0),
            ..BoxStyle::default()
        },
        children: vec![row(vec![
            column(
                46.0,
                vec![styled(
                    vec![mono(format!("{number:02}")).bold()],
                    20.0,
                    ACCENT,
                    Align::Start,
                    0.0,
                )],
            ),
            column(W - 46.0, text),
        ])],
    });
    vec![anchor(&number.to_string(), title, 1), head]
}

/// Several nodes as one block: a page break never falls inside it.
fn keep(children: Vec<Node>) -> Node {
    Node::Box(Container {
        style: BoxStyle::default(),
        children,
    })
}

/// A tinted panel with a coloured rule down its left edge and a label.
fn callout(label: &str, edge: Color, children: Vec<Node>) -> Node {
    let mut inner = vec![styled(
        vec![Run::new(label.to_uppercase())],
        6.8,
        MUTED,
        Align::Start,
        7.0,
    )];
    inner.extend(children);
    Node::Box(Container {
        style: BoxStyle {
            width: Some(Pt(W)),
            background: Some(PANEL),
            border: Edges {
                left: rule(edge, 2.5),
                ..Edges::default()
            },
            padding: Edges::symmetric(Pt(14.0), Pt(16.0)),
            ..BoxStyle::default()
        },
        children: inner,
    })
}

fn bullets(items: Vec<String>, size: f32) -> Node {
    Node::List(List {
        marker: Marker::Bullet {
            glyph: "•".into()
        },
        items,
        style: TextStyle {
            size: Pt(size),
            color: BODY,
            space_after: Pt(4.0),
            ..TextStyle::default()
        },
        gutter: Some(Pt(11.0)),
    })
}

/// The author's notes, as a panel.
fn notes(r: &Report) -> Vec<Node> {
    let Some(notes) = &r.notes else {
        return Vec::new();
    };
    let paragraphs = notes
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|p| styled(vec![Run::new(p.trim())], 8.5, BODY, Align::Start, 4.0))
        .collect();
    vec![spacer(14.0), callout(r.t(T::Notes), MUTED, paragraphs)]
}

/// What a reader should take away, worked out from the figures.
fn key_points(r: &Report) -> Vec<String> {
    let lang = r.lang;
    let (now, before) = (&r.usage.totals, &r.usage.previous);
    let mut points = Vec::new();
    let amount = r.amount(now.cost_nanos);
    if now.requests == 0 {
        return points;
    }
    let previous = r.amount(before.cost_nanos);
    points.push(lang.point_total(
        &r.money(amount),
        change(amount, previous).map(|c| lang.change(c)).as_deref(),
        &r.money(previous),
    ));
    for (section, what) in [("teams", T::Team), ("models", T::Model)] {
        if r.narrowed.contains(&section) {
            continue;
        }
        let groups = r.groups(section);
        let total: i64 = groups.iter().map(|g| g.totals.cost_nanos).sum();
        if let (Some(top), true) = (groups.first(), groups.len() > 1 && total > 0) {
            points.push(lang.point_leader(
                &lang.the(what),
                &top.name,
                &r.money(r.amount(top.totals.cost_nanos)),
                &lang.percent(top.totals.cost_nanos as f64 / total as f64, 1),
            ));
        }
    }
    if now.prompt_tokens > 0 && now.cached_tokens > 0 {
        // What the cached tokens would have cost at the input rate.
        let saved: i64 = r
            .usage
            .by_model
            .iter()
            .filter_map(|m| {
                let rates = &r
                    .rates
                    .iter()
                    .find(|x| x.model == m.model_name)?
                    .pricing
                    .as_ref()?
                    .tokens;
                let read = rates.cache_read?;
                Some(
                    m.totals
                        .cached_tokens
                        .saturating_mul((rates.input - read).max(0))
                        / 1000,
                )
            })
            .sum();
        points.push(lang.point_cache(
            &lang.percent(now.cached_tokens as f64 / now.prompt_tokens as f64, 1),
            (saved > 0).then(|| r.money(r.amount(saved))).as_deref(),
        ));
    }
    if now.failed_requests > 0 {
        points.push(lang.point_failures(
            &lang.integer(now.failed_requests),
            &lang.percent(now.failed_requests as f64 / now.requests as f64, 1),
        ));
    }
    if let Some(peak) = r
        .usage
        .daily
        .iter()
        .filter(|d| d.totals.cost_nanos > 0)
        .max_by_key(|d| d.totals.cost_nanos)
    {
        points.push(lang.point_peak(
            &day_label(r, &peak.day),
            &r.money(r.amount(peak.totals.cost_nanos)),
        ));
    }
    points
}

// ── summary ─────────────────────────────────────────────────────────────

/// A polyline of `values` across a `w` × `h` box, highest at the top.
fn sparkline(values: &[f64], w: f32, h: f32) -> Node {
    let max = values.iter().cloned().fold(0.0, f64::max);
    let y = |v: f64| {
        if max > 0.0 {
            h - 1.0 - (v / max) as f32 * (h - 2.0)
        } else {
            h - 1.0
        }
    };
    let mut ops = Vec::new();
    match values {
        [] => {}
        [only] => {
            ops.push(Op::MoveTo {
                x: Pt(0.0),
                y: Pt(y(*only)),
            });
            ops.push(Op::LineTo {
                x: Pt(w),
                y: Pt(y(*only)),
            });
        }
        _ => {
            let step = w / (values.len() - 1) as f32;
            for (i, v) in values.iter().enumerate() {
                let (x, y) = (Pt(i as f32 * step), Pt(y(*v)));
                ops.push(if i == 0 {
                    Op::MoveTo { x, y }
                } else {
                    Op::LineTo { x, y }
                });
            }
        }
    }
    Node::Canvas(Canvas {
        width: Pt(w),
        height: Pt(h),
        ops,
        fill: None,
        stroke: Some(Stroke {
            color: ACCENT,
            width: Pt(1.0),
        }),
        space_after: Pt(0.0),
    })
}

fn summary(r: &Report) -> Vec<Node> {
    let lang = r.lang;
    let (now, before) = (&r.usage.totals, &r.usage.previous);
    let daily = &r.usage.daily;
    let delta = |current: f64, previous: f64| match change(current, previous) {
        Some(c) => format!("{} {}", lang.change(c), r.t(T::VsPrevious)),
        None => r.t(T::NoPrevious).to_string(),
    };
    let tokens = |t: &Totals| (t.prompt_tokens + t.completion_tokens) as f64;
    let failed = if now.requests > 0 {
        format!(
            "{} {} · {}",
            lang.integer(now.failed_requests),
            r.t(T::Failed).to_lowercase(),
            lang.percent(now.failed_requests as f64 / now.requests as f64, 1)
        )
    } else {
        String::new()
    };
    let tiles = [
        (
            T::TotalAmount,
            r.money(r.amount(now.cost_nanos)),
            delta(r.amount(now.cost_nanos), r.amount(before.cost_nanos)),
            daily
                .iter()
                .map(|d| r.amount(d.totals.cost_nanos))
                .collect::<Vec<_>>(),
        ),
        (
            T::Requests,
            lang.integer(now.requests),
            if failed.is_empty() {
                delta(now.requests as f64, before.requests as f64)
            } else {
                failed
            },
            daily.iter().map(|d| d.totals.requests as f64).collect(),
        ),
        (
            T::Tokens,
            lang.integer(now.prompt_tokens + now.completion_tokens),
            delta(tokens(now), tokens(before)),
            daily.iter().map(|d| tokens(&d.totals)).collect(),
        ),
        (
            T::AvgLatency,
            avg_latency(now)
                .map(|ms| format!("{} ms", lang.number(ms, 0)))
                .unwrap_or_else(|| "—".into()),
            delta(
                avg_latency(now).unwrap_or(0.0),
                avg_latency(before).unwrap_or(0.0),
            ),
            daily
                .iter()
                .map(|d| avg_latency(&d.totals).unwrap_or(0.0))
                .collect(),
        ),
    ];
    let gap = 10.0;
    let tile_w = (W - 3.0 * gap) / 4.0;
    let inner = tile_w - 20.0;
    let mut cells = Vec::new();
    for (i, (label, value, sub, series)) in tiles.into_iter().enumerate() {
        if i > 0 {
            cells.push(column(gap, Vec::new()));
        }
        cells.push(panel(
            tile_w,
            10.0,
            vec![
                styled(
                    vec![Run::new(r.t(label).to_uppercase())],
                    6.2,
                    MUTED,
                    Align::Start,
                    4.0,
                ),
                styled(vec![Run::new(value).bold()], 14.5, INK, Align::Start, 2.0),
                styled(vec![Run::new(sub)], 6.6, MUTED, Align::Start, 6.0),
                sparkline(&series, inner, 18.0),
            ],
        ));
    }
    let mut out = vec![row(cells)];

    if r.splits_cost() {
        let cost = r.cost(now.cost_nanos);
        let billed = r.amount(now.cost_nanos);
        let step = |label: String, value: f64, emphasis: bool| {
            column(
                (W - 24.0) / 3.0,
                vec![
                    styled(
                        vec![Run::new(label.to_uppercase())],
                        6.2,
                        MUTED,
                        Align::Start,
                        2.0,
                    ),
                    styled(
                        vec![if emphasis {
                            Run::new(r.money(value)).bold()
                        } else {
                            Run::new(r.money(value))
                        }],
                        11.0,
                        if emphasis { ACCENT } else { INK },
                        Align::Start,
                        0.0,
                    ),
                ],
            )
        };
        out.push(spacer(10.0));
        out.push(plain(r.t(T::MoneySequence), 7.5, MUTED));
        out.push(spacer(4.0));
        out.push(panel(
            W,
            12.0,
            vec![row(vec![
                step(r.t(T::ProviderCost).to_string(), cost, false),
                step(
                    format!(
                        "{} (+{})",
                        r.t(T::Margin),
                        lang.percent(r.markup_percent() / 100.0, 2)
                    ),
                    billed - cost,
                    false,
                ),
                step(r.t(T::TotalAmount).to_string(), billed, true),
            ])],
        ));
    }
    let points = key_points(r);
    if !points.is_empty() {
        out.push(spacer(18.0));
        out.push(callout(
            r.t(T::KeyPoints),
            ACCENT,
            vec![bullets(points, 8.5)],
        ));
    }
    out.extend(notes(r));
    out
}

// ── the trend chart ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Bucket {
    Day,
    Week,
    Month,
}

/// Days per bar: one up to two months, a week up to six, a month beyond.
/// Each bucket is a range of indexes into the days, so the previous period
/// is cut the same way.
fn buckets(days: &[Day]) -> (Bucket, Vec<std::ops::Range<usize>>) {
    let n = days.len();
    if n <= 62 {
        return (Bucket::Day, (0..n).map(|i| i..i + 1).collect());
    }
    if n <= 182 {
        return (
            Bucket::Week,
            (0..n).step_by(7).map(|i| i..(i + 7).min(n)).collect(),
        );
    }
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    let month = |i: usize| {
        chrono::NaiveDate::parse_from_str(&days[i].day, "%Y-%m-%d")
            .map(|d| (d.year(), d.month()))
            .ok()
    };
    for i in 0..n {
        match out.last_mut() {
            Some(last) if month(last.start) == month(i) => last.end = i + 1,
            _ => out.push(i..i + 1),
        }
    }
    (Bucket::Month, out)
}

/// A round step and how many of them reach `max`: 1, 2, 2.5 or 5 times a
/// power of ten, four or five steps (Heckbert's nice numbers).
fn nice_ticks(max: f64) -> (f64, usize) {
    if max <= 0.0 || !max.is_finite() {
        return (1.0, 1);
    }
    let raw = max / 4.0;
    let magnitude = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * magnitude);
    (step, (max / step).ceil().max(1.0) as usize)
}

fn trend(r: &Report, number: &str) -> Vec<Node> {
    let lang = r.lang;
    let days = &r.usage.daily;
    let (bucket, ranges) = buckets(days);
    let title = r.t(match bucket {
        Bucket::Day => T::SpendPerDay,
        Bucket::Week => T::SpendPerWeek,
        Bucket::Month => T::SpendPerMonth,
    });
    let sum = |list: &[Day], range: &std::ops::Range<usize>| {
        list.get(range.clone())
            .map(|ds| r.amount(ds.iter().map(|d| d.totals.cost_nanos).sum()))
            .unwrap_or(0.0)
    };
    let current: Vec<f64> = ranges.iter().map(|range| sum(days, range)).collect();
    let previous: Vec<f64> = ranges
        .iter()
        .map(|range| sum(&r.previous_daily, range))
        .collect();
    let has_previous = previous.iter().any(|v| *v > 0.0);

    let mut out = heading(number, title, None);
    let mut block = Vec::new();
    if current.iter().all(|v| *v == 0.0) {
        out.push(empty(r));
        return out;
    }

    let label_size = 6.5;
    let line = label_size * LEADING;
    let margin = line / 2.0;
    let plot_h = 120.0;
    let height = plot_h + 2.0 * margin;
    let axis_w = 48.0;
    let tick_w = 4.0;
    let plot_w = W - axis_w - tick_w;

    let max = current
        .iter()
        .chain(previous.iter())
        .cloned()
        .fold(0.0, f64::max);
    let (step, steps) = nice_ticks(max);
    let top = step * steps as f64;
    let decimals = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        4
    };
    let y_of = |v: f64| margin + plot_h - (v / top) as f32 * plot_h;

    // Legend.
    let mut legend = vec![
        swatch(ACCENT, 7.0),
        column(130.0, vec![plain(r.t(T::ThisPeriod), 7.0, BODY)]),
    ];
    if has_previous {
        legend.push(swatch(PREVIOUS, 7.0));
        legend.push(column(
            130.0,
            vec![plain(r.t(T::PreviousPeriod), 7.0, BODY)],
        ));
    }
    block.push(row(legend));
    block.push(spacer(8.0));

    // The axis: labels, then ticks against the plot's edge.
    let mut labels = Vec::new();
    let between = plot_h / steps as f32 - line;
    for i in (0..=steps).rev() {
        labels.push(aligned(
            lang.money_with(r.currency, step * i as f64, decimals),
            label_size,
            MUTED,
            Align::End,
        ));
        if i > 0 {
            labels.push(spacer(between));
        }
    }
    // One canvas: the grid, then each period's bars, each painted in its
    // own colour as it is closed off.
    let plot = |x: f32| tick_w + x;
    let mut ops = Vec::new();
    for i in 0..=steps {
        let y = Pt(y_of(step * i as f64));
        ops.push(Op::MoveTo { x: Pt(0.0), y });
        ops.push(Op::LineTo {
            x: Pt(tick_w + plot_w),
            y,
        });
    }
    ops.push(Op::Stroke {
        color: RULE,
        width: Pt(0.5),
    });
    let slot = plot_w / ranges.len() as f32;
    let bars = |ops: &mut Vec<Op>, values: &[f64], at: &dyn Fn(usize) -> (f32, f32)| {
        for (i, value) in values.iter().enumerate() {
            if *value > 0.0 {
                let (x, w) = at(i);
                let y = y_of(*value);
                ops.push(rect(plot(x), y, w, margin + plot_h - y));
            }
        }
    };
    if has_previous {
        let half = slot / 2.0;
        bars(&mut ops, &previous, &|i| {
            (i as f32 * slot + half * 0.3, half * 0.7)
        });
        ops.push(Op::Fill { color: PREVIOUS });
        bars(&mut ops, &current, &|i| {
            (i as f32 * slot + half, half * 0.7)
        });
    } else {
        bars(&mut ops, &current, &|i| {
            (i as f32 * slot + slot * 0.15, slot * 0.7)
        });
    }
    ops.push(Op::Fill { color: ACCENT });
    // The baseline over the bars' feet.
    ops.push(Op::MoveTo {
        x: Pt(0.0),
        y: Pt(margin + plot_h),
    });
    ops.push(Op::LineTo {
        x: Pt(tick_w + plot_w),
        y: Pt(margin + plot_h),
    });
    ops.push(Op::Stroke {
        color: FAINT,
        width: Pt(0.75),
    });
    block.push(row(vec![
        Node::Box(Container {
            style: BoxStyle {
                width: Some(Pt(axis_w)),
                padding: Edges {
                    right: Pt(4.0),
                    ..Edges::default()
                },
                ..BoxStyle::default()
            },
            children: labels,
        }),
        canvas(tick_w + plot_w, height, ops, None),
    ]));

    // Dates under the first, middle and last bars.
    let date = |range: &std::ops::Range<usize>| {
        let day = chrono::NaiveDate::parse_from_str(&days[range.start].day, "%Y-%m-%d").ok();
        day.map(|d| match bucket {
            Bucket::Month => lang.month_year(d),
            _ => lang.day_month(d),
        })
        .unwrap_or_default()
    };
    let third = plot_w / 3.0;
    let mut dates = vec![column(axis_w + tick_w, Vec::new())];
    dates.push(column(
        third,
        vec![aligned(date(&ranges[0]), label_size, MUTED, Align::Start)],
    ));
    dates.push(column(
        third,
        vec![if ranges.len() > 2 {
            aligned(
                date(&ranges[ranges.len() / 2]),
                label_size,
                MUTED,
                Align::Center,
            )
        } else {
            spacer(0.0)
        }],
    ));
    dates.push(column(
        third,
        vec![if ranges.len() > 1 {
            aligned(
                date(&ranges[ranges.len() - 1]),
                label_size,
                MUTED,
                Align::End,
            )
        } else {
            spacer(0.0)
        }],
    ));
    block.push(spacer(3.0));
    block.push(row(dates));

    // The figures the eye looks for.
    let (peak_at, peak) =
        current.iter().enumerate().fold(
            (0, 0.0),
            |(bi, bv), (i, v)| if *v > bv { (i, *v) } else { (bi, bv) },
        );
    let total = r.amount(r.usage.totals.cost_nanos);
    block.push(spacer(6.0));
    block.push(plain(
        format!(
            "{}: {} ({}) · {}: {}",
            r.t(T::Peak),
            r.money(peak),
            date(&ranges[peak_at]),
            r.t(T::DailyAverage),
            r.money(total / days.len().max(1) as f64)
        ),
        7.0,
        MUTED,
    ));
    out.push(keep(block));
    out
}

// ── token mix ───────────────────────────────────────────────────────────

fn token_mix(r: &Report, number: &str) -> Vec<Node> {
    let lang = r.lang;
    let t = &r.usage.totals;
    let parts = [
        (T::UncachedInput, uncached_input(t)),
        (T::CacheReads, t.cached_tokens),
        (T::CacheWrites, t.cache_write_tokens),
        (T::PlainOutput, plain_output(t)),
        (T::Reasoning, t.reasoning_tokens),
    ];
    let total: i64 = parts.iter().map(|(_, v)| v).sum();
    let mut out = heading(number, r.t(T::TokenMix), Some(r.t(T::TokenMixNote)));
    let mut block = Vec::new();
    if total == 0 {
        out.push(empty(r));
        return out;
    }
    let mut bar = Vec::new();
    let mut used = 0.0;
    let last = parts.iter().rposition(|(_, v)| *v > 0).unwrap_or(0);
    for (i, (_, value)) in parts.iter().enumerate() {
        if *value == 0 {
            continue;
        }
        let w = if i == last {
            W - used
        } else {
            W * *value as f32 / total as f32
        };
        used += w;
        bar.push(canvas(
            w,
            14.0,
            vec![rect(0.0, 0.0, w, 14.0)],
            Some(SERIES[i]),
        ));
    }
    block.push(row(bar));
    block.push(spacer(8.0));
    for (i, (label, value)) in parts.iter().enumerate() {
        block.push(row(vec![
            swatch(SERIES[i], 7.5),
            column(220.0, vec![plain(r.t(*label), 7.5, BODY)]),
            column(
                110.0,
                vec![aligned(lang.integer(*value), 7.5, INK, Align::End)],
            ),
            column(
                70.0,
                vec![aligned(
                    lang.percent(*value as f64 / total as f64, 1),
                    7.5,
                    MUTED,
                    Align::End,
                )],
            ),
        ]));
        block.push(spacer(2.5));
    }
    out.push(keep(block));
    out
}

// ── breakdowns ──────────────────────────────────────────────────────────

/// The amount columns a table carries: the amount, or cost and amount.
fn amount_heads(r: &Report) -> Vec<String> {
    if r.splits_cost() {
        vec![r.t(T::Cost).into(), r.t(T::Amount).into()]
    } else {
        vec![r.t(T::Amount).into()]
    }
}

fn amount_cells(r: &Report, nanos: i64) -> Vec<String> {
    if r.splits_cost() {
        vec![r.money(r.cost(nanos)), r.money(r.amount(nanos))]
    } else {
        vec![r.money(r.amount(nanos))]
    }
}

fn amount_cols(r: &Report, width: f32) -> Vec<Col> {
    let n = if r.splits_cost() { 2 } else { 1 };
    (0..n).map(|_| Col(Some(width), Align::End)).collect()
}

/// Horizontal bars for the largest groups, then the whole table.
fn breakdown(r: &Report, number: &str, groups: &[Group], title: T, detail: Option<T>) -> Vec<Node> {
    let lang = r.lang;
    let mut out = heading(number, r.t(title), None);
    if groups.is_empty() {
        out.push(empty(r));
        return out;
    }
    let total: i64 = groups.iter().map(|g| g.totals.cost_nanos).sum();
    let share = |nanos: i64| {
        if total > 0 {
            lang.percent(nanos as f64 / total as f64, 1)
        } else {
            "—".into()
        }
    };

    let mut chart = Vec::new();
    // The top of the list as bars, the rest as one.
    const TOP: usize = 8;
    let mut bars: Vec<(String, i64)> = groups
        .iter()
        .take(TOP)
        .map(|g| (g.name.clone(), g.totals.cost_nanos))
        .collect();
    if groups.len() > TOP {
        bars.push((
            format!("{} ({})", r.t(T::Other), groups.len() - TOP),
            groups[TOP..].iter().map(|g| g.totals.cost_nanos).sum(),
        ));
    }
    let max = bars.iter().map(|(_, v)| *v).max().unwrap_or(0);
    let (name_w, value_w, share_w, gap) = (150.0, 78.0, 44.0, 8.0);
    let bar_w = W - name_w - value_w - share_w - gap;
    let size = 7.5;
    let line = size * LEADING;
    for (i, (name, nanos)) in bars.iter().enumerate() {
        let other = i == TOP;
        let filled = if max > 0 {
            (bar_w * *nanos as f32 / max as f32).max(0.5)
        } else {
            0.5
        };
        let mut cells = vec![column(name_w, vec![plain(clip(name, 34), size, BODY)])];
        // The track, then the bar over it, in one canvas.
        cells.push(canvas(
            bar_w,
            line,
            vec![
                rect(0.0, 1.5, bar_w, line - 3.0),
                Op::Fill { color: TRACK },
                rect(0.0, 1.5, filled, line - 3.0),
                Op::Fill {
                    color: if other { PREVIOUS } else { ACCENT },
                },
            ],
            None,
        ));
        cells.push(column(gap, Vec::new()));
        cells.push(column(
            value_w,
            vec![aligned(r.money(r.amount(*nanos)), size, INK, Align::End)],
        ));
        cells.push(column(
            share_w,
            vec![aligned(share(*nanos), size, MUTED, Align::End)],
        ));
        chart.push(row(cells));
        chart.push(spacer(3.0));
    }
    chart.push(spacer(10.0));

    out.push(keep(chart));

    // Every group, with its numbers.
    let split = r.splits_cost();
    let mut cols = vec![Col(None, Align::Start)];
    let mut head = vec![out_name(r, title)];
    if let Some(detail) = detail {
        cols.push(Col(Some(80.0), Align::Start));
        head.push(r.t(detail).into());
    }
    cols.push(Col(Some(42.0), Align::End));
    head.push(r.t(T::Requests).into());
    cols.push(Col(Some(54.0), Align::End));
    head.push(r.t(T::In).into());
    cols.push(Col(Some(54.0), Align::End));
    head.push(r.t(T::Out).into());
    if !split {
        cols.push(Col(Some(50.0), Align::End));
        head.push(r.t(T::Cached).into());
    }
    cols.extend(amount_cols(r, 60.0));
    head.extend(amount_heads(r));
    cols.push(Col(Some(38.0), Align::End));
    head.push(r.t(T::Share).into());

    let cells = |name: String, detail_value: Option<String>, t: &Totals| {
        let mut c = vec![name];
        if detail.is_some() {
            c.push(detail_value.unwrap_or_default());
        }
        c.push(lang.integer(t.requests));
        c.push(lang.integer(t.prompt_tokens));
        c.push(lang.integer(t.completion_tokens));
        if !split {
            c.push(lang.integer(t.cached_tokens));
        }
        c.extend(amount_cells(r, t.cost_nanos));
        c.push(share(t.cost_nanos));
        c
    };
    let mut rows: Vec<(Vec<String>, Kind)> = groups
        .iter()
        .map(|g| {
            (
                cells(g.name.clone(), g.detail.clone(), &g.totals),
                Kind::Body,
            )
        })
        .collect();
    let mut sum = Totals::default();
    for g in groups {
        add(&mut sum, &g.totals);
    }
    rows.push((cells(r.t(T::Total).into(), None, &sum), Kind::Total));
    out.push(table(&cols, head, rows));
    out
}

/// What a breakdown's first column is called.
fn out_name(r: &Report, title: T) -> String {
    r.t(match title {
        T::ByTeam => T::Team,
        T::ByKey => T::Key,
        T::ByPerson => T::Person,
        T::ByModel => T::Model,
        T::ByCustomer => T::Customer,
        _ => T::Tag,
    })
    .into()
}

fn add(sum: &mut Totals, t: &Totals) {
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

fn day_label(r: &Report, day: &str) -> String {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| r.lang.date(d))
        .unwrap_or_else(|_| day.to_string())
}

fn daily(r: &Report) -> Vec<Node> {
    let lang = r.lang;
    let mut out = Vec::new();
    let days: Vec<&Day> = r
        .usage
        .daily
        .iter()
        .filter(|d| d.totals.requests > 0 || d.totals.cost_nanos > 0)
        .collect();
    if days.is_empty() {
        out.push(empty(r));
        return out;
    }
    let mut cols = vec![
        Col(Some(76.0), Align::Start),
        Col(Some(56.0), Align::End),
        Col(Some(46.0), Align::End),
        Col(Some(66.0), Align::End),
        Col(Some(66.0), Align::End),
    ];
    cols.extend(amount_cols(r, 66.0));
    cols.push(Col(None, Align::End));
    let mut head: Vec<String> = [T::Date, T::Requests, T::Failed, T::In, T::Out]
        .iter()
        .map(|t| r.t(*t).to_string())
        .collect();
    head.extend(amount_heads(r));
    head.push(r.t(T::RunningTotal).into());
    let mut running = 0;
    let mut sum = Totals::default();
    let mut rows = Vec::new();
    for d in days {
        running += d.totals.cost_nanos;
        add(&mut sum, &d.totals);
        let mut c = vec![
            day_label(r, &d.day),
            lang.integer(d.totals.requests),
            lang.integer(d.totals.failed_requests),
            lang.integer(d.totals.prompt_tokens),
            lang.integer(d.totals.completion_tokens),
        ];
        c.extend(amount_cells(r, d.totals.cost_nanos));
        c.push(r.money(r.amount(running)));
        rows.push((c, Kind::Body));
    }
    let mut c = vec![
        r.t(T::Total).into(),
        lang.integer(sum.requests),
        lang.integer(sum.failed_requests),
        lang.integer(sum.prompt_tokens),
        lang.integer(sum.completion_tokens),
    ];
    c.extend(amount_cells(r, sum.cost_nanos));
    c.push(String::new());
    rows.push((c, Kind::Total));
    out.push(table(&cols, head, rows));
    out
}

fn lines(r: &Report) -> Vec<Node> {
    let lang = r.lang;
    let mut out = Vec::new();
    if r.lines.is_empty() {
        out.push(empty(r));
        return out;
    }
    let split = r.splits_cost();
    let mut cols = vec![
        Col(Some(52.0), Align::Start),
        Col(None, Align::Start),
        Col(None, Align::Start),
        Col(None, Align::Start),
        Col(Some(36.0), Align::End),
        Col(Some(52.0), Align::End),
        Col(Some(52.0), Align::End),
    ];
    cols.extend(amount_cols(r, if split { 52.0 } else { 58.0 }));
    cols.push(Col(Some(60.0), Align::End));
    let mut head: Vec<String> = [
        T::Date,
        T::Team,
        T::Key,
        T::Model,
        T::Requests,
        T::In,
        T::Out,
    ]
    .iter()
    .map(|t| r.t(*t).to_string())
    .collect();
    head.extend(amount_heads(r));
    head.push(r.t(T::RunningTotal).into());

    let mut rows = Vec::new();
    let mut running = 0i64;
    let mut index = 0;
    while index < r.lines.len() {
        let day = &r.lines[index].day;
        let end = r.lines[index..]
            .iter()
            .position(|l| &l.day != day)
            .map_or(r.lines.len(), |p| index + p);
        let mut day_sum = Totals::default();
        for l in &r.lines[index..end] {
            running += l.cost_nanos;
            day_sum.cost_nanos += l.cost_nanos;
            day_sum.requests += l.requests;
            day_sum.prompt_tokens += l.prompt_tokens;
            day_sum.completion_tokens += l.completion_tokens;
            let mut c = vec![
                day_label(r, &l.day),
                r.team_of(l.team_name.as_deref(), l.owner.as_deref()),
                r.key_of(l.key_name.as_deref(), l.key_id),
                l.model_name.clone(),
                lang.integer(l.requests),
                lang.integer(l.prompt_tokens),
                lang.integer(l.completion_tokens),
            ];
            c.extend(amount_cells(r, l.cost_nanos));
            c.push(r.money(r.amount(running)));
            rows.push((c, Kind::Body));
        }
        if end - index > 1 {
            let mut c = vec![
                String::new(),
                format!("{} {}", r.t(T::Subtotal), day_label(r, day)),
                String::new(),
                String::new(),
                lang.integer(day_sum.requests),
                lang.integer(day_sum.prompt_tokens),
                lang.integer(day_sum.completion_tokens),
            ];
            c.extend(amount_cells(r, day_sum.cost_nanos));
            c.push(String::new());
            rows.push((c, Kind::Subtotal));
        }
        index = end;
    }
    let t = &r.usage.totals;
    let mut c = vec![
        r.t(T::Total).into(),
        String::new(),
        String::new(),
        String::new(),
        lang.integer(t.requests),
        lang.integer(t.prompt_tokens),
        lang.integer(t.completion_tokens),
    ];
    c.extend(amount_cells(r, t.cost_nanos));
    c.push(r.money(r.amount(running)));
    rows.push((c, Kind::Total));
    out.push(table(&cols, head, rows));
    out
}

fn requests(r: &Report) -> Vec<Node> {
    let lang = r.lang;
    let note = r.requests_capped.map(|cap| match lang {
        super::Lang::En => format!(
            "The first {} requests of the period; the rest are in the dashboard's logs.",
            lang.integer(cap as i64)
        ),
        super::Lang::Es => format!(
            "Las primeras {} peticiones del periodo; el resto está en los registros del panel.",
            lang.integer(cap as i64)
        ),
    });
    let mut out = Vec::new();
    if let Some(note) = note {
        out.push(styled(vec![Run::new(note)], 7.5, MUTED, Align::Start, 8.0));
    }
    if r.requests.is_empty() {
        out.push(empty(r));
        return out;
    }
    let mut cols = vec![
        Col(Some(62.0), Align::Start),
        Col(Some(96.0), Align::Start),
        Col(None, Align::Start),
        Col(None, Align::Start),
        Col(Some(30.0), Align::Center),
        Col(Some(44.0), Align::End),
        Col(Some(44.0), Align::End),
        Col(Some(40.0), Align::End),
    ];
    cols.extend(amount_cols(r, 52.0));
    let mut head: Vec<String> = [
        T::Time,
        T::RequestId,
        T::Key,
        T::Model,
        T::Status,
        T::In,
        T::Out,
        T::Latency,
    ]
    .iter()
    .map(|t| r.t(*t).to_string())
    .collect();
    head.extend(amount_heads(r));
    let rows = r
        .requests
        .iter()
        .map(|q| {
            let mut c = vec![
                q.created_at.format("%Y-%m-%d %H:%M").to_string(),
                q.request_id.clone(),
                r.key_of(q.key_name.as_deref(), q.key_id),
                q.model_name.clone(),
                q.status_code.to_string(),
                lang.integer(q.prompt_tokens),
                lang.integer(q.completion_tokens),
                format!("{} ms", lang.integer(q.latency_ms)),
            ];
            c.extend(amount_cells(r, q.cost_nanos));
            (c, Kind::Body)
        })
        .collect();
    out.push(table(&cols, head, rows));
    out
}

fn rates(r: &Report, number: &str) -> Vec<Node> {
    let mut out = heading(number, r.t(T::RatesApplied), Some(r.t(T::RatesNote)));
    if r.rates.is_empty() {
        out.push(empty(r));
        return out;
    }
    let cols = [
        Col(None, Align::Start),
        Col(Some(58.0), Align::Start),
        Col(Some(62.0), Align::Start),
        Col(Some(50.0), Align::End),
        Col(Some(50.0), Align::End),
        Col(Some(54.0), Align::End),
        Col(Some(56.0), Align::End),
        Col(Some(56.0), Align::End),
    ];
    let head = [
        T::Model,
        T::Provider,
        T::PriceSource,
        T::InputRate,
        T::OutputRate,
        T::CacheReadRate,
        T::CacheWriteRate,
        T::ReasoningRate,
    ]
    .iter()
    .map(|t| r.t(*t).to_string())
    .collect();
    let price = |micros: Option<i64>| match micros {
        Some(m) => {
            let v = r.rate_value(m);
            r.lang
                .money_with(r.currency, v, if v != 0.0 && v < 1.0 { 4 } else { 2 })
        }
        None => "—".into(),
    };
    let rows = r
        .rates
        .iter()
        .map(|rate| {
            let tokens = rate.pricing.as_ref().map(|p| &p.tokens);
            (
                vec![
                    rate.model.clone(),
                    rate.provider.clone(),
                    r.source_name(rate.source).into(),
                    price(tokens.map(|t| t.input)),
                    price(tokens.map(|t| t.output)),
                    price(tokens.and_then(|t| t.cache_read)),
                    price(tokens.and_then(|t| t.cache_write)),
                    price(tokens.and_then(|t| t.reasoning)),
                ],
                Kind::Body,
            )
        })
        .collect();
    out.push(table(&cols, head, rows));
    out
}

fn methodology(r: &Report, number: &str) -> Vec<Node> {
    let mut out = heading(number, r.t(T::Methodology), None);
    out.push(Node::List(List {
        marker: Marker::Bullet {
            glyph: "•".into()
        },
        items: r.methodology(),
        style: TextStyle {
            size: Pt(7.5),
            color: BODY,
            space_after: Pt(3.0),
            ..TextStyle::default()
        },
        gutter: Some(Pt(10.0)),
    }));
    if !r.has("summary") {
        out.extend(notes(r));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ticks_land_on_round_numbers() {
        assert_eq!(nice_ticks(9.3), (2.5, 4));
        assert_eq!(nice_ticks(100.0), (25.0, 4));
        assert_eq!(nice_ticks(0.042), (0.02, 3));
        assert_eq!(nice_ticks(0.0), (1.0, 1));
    }

    #[test]
    fn long_ranges_are_drawn_by_week_then_by_month() {
        let days = |n: i64| {
            let start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
            (0..n)
                .map(|i| Day {
                    day: (start + chrono::Duration::days(i)).to_string(),
                    totals: Totals::default(),
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(buckets(&days(30)).1.len(), 30);
        let (bucket, weeks) = buckets(&days(90));
        assert!(bucket == Bucket::Week && weeks.len() == 13);
        let (bucket, months) = buckets(&days(365));
        assert!(bucket == Bucket::Month && months.len() == 12);
        assert_eq!(months[1], 31..59, "February");
    }

    #[test]
    fn a_full_report_renders_without_a_missing_glyph() {
        for lang in [super::super::Lang::En, super::super::Lang::Es] {
            let report = super::super::tests::sample(lang, 2_000, true);
            let built = built(&document(&report)).unwrap();
            let problems: Vec<_> = built
                .diagnostics
                .iter()
                .filter(|d| !is_clipped(d))
                .collect();
            assert!(problems.is_empty(), "{problems:?}");
            assert!(built.pages > 8, "{} pages", built.pages);
            let pdf = built.pdf.into_vec();
            if let Ok(dir) = std::env::var("REPORT_PREVIEW_DIR") {
                std::fs::write(format!("{dir}/report-{lang:?}.pdf"), pdf).unwrap();
            }
        }
    }
}
