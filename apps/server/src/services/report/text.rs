//! The words and number formats of a report, in English or Spanish.
//!
//! Every string is one arm of a match on both languages, so a phrase missing
//! from either does not compile.

use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

impl Lang {
    /// The account's language, else the first the browser accepts that a
    /// report speaks, else English: the dashboard's own order.
    pub fn pick(account: Option<&str>, accept_language: Option<&str>) -> Lang {
        let of = |tag: &str| match tag.trim().get(..2).map(str::to_ascii_lowercase) {
            Some(p) if p == "es" => Some(Lang::Es),
            Some(p) if p == "en" => Some(Lang::En),
            _ => None,
        };
        account
            .and_then(of)
            .or_else(|| {
                accept_language?
                    .split(',')
                    .find_map(|part| of(part.split(';').next().unwrap_or("")))
            })
            .unwrap_or(Lang::En)
    }

    fn separators(self) -> (char, char) {
        match self {
            Lang::En => (',', '.'),
            Lang::Es => ('.', ','),
        }
    }

    /// `1,234.56` or `1.234,56`. Spanish groups only from five digits, as
    /// the dashboard's `Intl` does.
    pub fn number(self, value: f64, decimals: usize) -> String {
        let (group, point) = self.separators();
        let fixed = format!("{:.*}", decimals, value.abs());
        let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
        let mut grouped = String::new();
        let groups = self == Lang::En || int.len() > 4;
        for (i, digit) in int.chars().enumerate() {
            if groups && i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push(group);
            }
            grouped.push(digit);
        }
        let negative = value < 0.0 && fixed.chars().any(|c| c.is_ascii_digit() && c != '0');
        let mut out = String::new();
        if negative {
            out.push('−');
        }
        out.push_str(&grouped);
        if !frac.is_empty() {
            out.push(point);
            out.push_str(frac);
        }
        out
    }

    pub fn integer(self, value: i64) -> String {
        self.number(value as f64, 0)
    }

    /// An amount with its currency: two decimals, four for a non-zero amount
    /// that two would show as nothing.
    pub fn money(self, currency: &str, value: f64) -> String {
        let decimals = if value != 0.0 && value.abs() < 0.005 {
            4
        } else {
            2
        };
        self.money_with(currency, value, decimals)
    }

    pub fn money_with(self, currency: &str, value: f64, decimals: usize) -> String {
        let number = self.number(value, decimals);
        let symbol = symbol(currency);
        match self {
            Lang::Es => format!("{number} {symbol}"),
            Lang::En => match number.strip_prefix('−') {
                Some(rest) => format!("−{symbol}{rest}"),
                None => format!("{symbol}{number}"),
            },
        }
    }

    /// A fraction as a percentage: `0.123` is `12.3%` or `12,3 %`.
    pub fn percent(self, fraction: f64, decimals: usize) -> String {
        let number = self.number(fraction * 100.0, decimals);
        match self {
            Lang::En => format!("{number}%"),
            Lang::Es => format!("{number} %"),
        }
    }

    /// A change against a previous value, signed: `+12.3%`.
    pub fn change(self, fraction: f64) -> String {
        let sign = if fraction > 0.0005 { "+" } else { "" };
        format!("{sign}{}", self.percent(fraction, 1))
    }

    /// `7 Oct 2026`, `7 oct 2026`.
    pub fn date(self, date: NaiveDate) -> String {
        format!("{} {}", self.day_month(date), date.year())
    }

    /// `7 Oct`, `7 oct`.
    pub fn day_month(self, date: NaiveDate) -> String {
        let month = self.month(date.month0() as usize);
        format!("{} {month}", date.day())
    }

    /// `Oct 2026`, `oct 2026`.
    pub fn month_year(self, date: NaiveDate) -> String {
        format!("{} {}", self.month(date.month0() as usize), date.year())
    }

    fn month(self, index: usize) -> &'static str {
        const EN: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        const ES: [&str; 12] = [
            "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
        ];
        match self {
            Lang::En => EN[index],
            Lang::Es => ES[index],
        }
    }

    /// `7 Sep – 6 Oct 2026 · 30 days`.
    pub fn period(self, from: NaiveDate, to: NaiveDate) -> String {
        let days = (to - from).num_days() + 1;
        let start = if from.year() == to.year() {
            self.day_month(from)
        } else {
            self.date(from)
        };
        let unit = match (self, days) {
            (Lang::En, 1) => "day",
            (Lang::En, _) => "days",
            (Lang::Es, 1) => "día",
            (Lang::Es, _) => "días",
        };
        format!("{start} – {} · {days} {unit}", self.date(to))
    }
}

/// How a currency is written next to an amount.
pub fn symbol(currency: &str) -> &str {
    match currency {
        "USD" => "$",
        "EUR" => "€",
        other => other,
    }
}

macro_rules! phrases {
    ($($name:ident => $en:expr, $es:expr;)*) => {
        /// A phrase of the report.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum T { $($name),* }

        impl Lang {
            pub fn t(self, phrase: T) -> &'static str {
                match (self, phrase) {
                    $((Lang::En, T::$name) => $en, (Lang::Es, T::$name) => $es,)*
                }
            }
        }
    };
}

phrases! {
    Kind => "Usage and cost report", "Informe de uso y costes";
    NotInvoice => "Usage statement. This document is not an invoice.",
        "Informe de consumo. Este documento no es una factura.";
    Client => "Client", "Cliente";
    Reference => "Reference", "Referencia";
    Period => "Period", "Periodo";
    From => "From", "Desde";
    To => "To", "Hasta";
    Currency => "Currency", "Moneda";
    ExchangeRate => "Exchange rate", "Tipo de cambio";
    Markup => "Markup", "Recargo";
    Generated => "Generated", "Generado";
    PreparedBy => "Prepared by", "Elaborado por";
    Scope => "Scope", "Alcance";
    Everything => "All usage you can see", "Todo el uso que puedes ver";
    ReportId => "Report ID", "ID del informe";
    Fingerprint => "Data fingerprint (SHA-256)", "Huella de los datos (SHA-256)";
    PageOf => "Page {{page}} of {{pages}}", "Página {{page}} de {{pages}}";
    Empty => "No usage in this period.", "Sin uso en este periodo.";

    Team => "Team", "Equipo";
    Key => "API key", "Clave de API";
    KeyHint => "Key ends in", "Terminación";
    Person => "Person", "Persona";
    Model => "Model", "Modelo";
    Customer => "Customer", "Cliente final";
    Tag => "Tag", "Etiqueta";
    Date => "Date", "Fecha";
    Time => "Time (UTC)", "Hora (UTC)";
    Provider => "Provider", "Proveedor";
    Endpoint => "Endpoint", "Endpoint";
    Status => "Status", "Estado";
    RequestId => "Request ID", "ID de petición";
    Personal => "Personal", "Personal";
    DeletedKey => "Deleted key", "Clave eliminada";
    DeletedTeam => "Deleted team", "Equipo eliminado";
    Other => "Other", "Otros";
    Total => "Total", "Total";
    Subtotal => "Subtotal", "Subtotal";

    TotalAmount => "Total amount", "Importe total";
    ProviderCost => "Provider cost", "Coste del proveedor";
    Margin => "Margin", "Margen";
    Amount => "Amount", "Importe";
    Cost => "Cost", "Coste";
    Share => "Share", "Peso";
    RunningTotal => "Running total", "Acumulado";
    Requests => "Requests", "Peticiones";
    Failed => "Failed", "Fallidas";
    FailedRequests => "Failed requests", "Peticiones fallidas";
    Tokens => "Tokens", "Tokens";
    InputTokens => "Input tokens", "Tokens de entrada";
    OutputTokens => "Output tokens", "Tokens de salida";
    CacheReads => "Cache reads", "Lecturas de caché";
    CacheWrites => "Cache writes", "Escrituras de caché";
    Reasoning => "Reasoning", "Razonamiento";
    UncachedInput => "Input, uncached", "Entrada sin caché";
    PlainOutput => "Output, excluding reasoning", "Salida sin razonamiento";
    In => "In", "Entrada";
    Out => "Out", "Salida";
    Cached => "Cached", "Caché";
    AvgLatency => "Average latency", "Latencia media";
    LatencyMs => "Average latency (ms)", "Latencia media (ms)";
    Latency => "Latency", "Latencia";
    ErrorRate => "Error rate", "Tasa de error";
    Indicator => "Indicator", "Indicador";
    ThisPeriod => "This period", "Este periodo";
    PreviousPeriod => "Previous period", "Periodo anterior";
    Change => "Change", "Variación";
    VsPrevious => "vs previous period", "vs. periodo anterior";
    NoPrevious => "No previous usage", "Sin uso anterior";
    PerRequest => "per request", "por petición";

    Summary => "Summary", "Resumen";
    SpendPerDay => "Spend per day", "Gasto por día";
    SpendPerWeek => "Spend per week", "Gasto por semana";
    SpendPerMonth => "Spend per month", "Gasto por mes";
    Peak => "Peak", "Máximo";
    DailyAverage => "Daily average", "Media diaria";
    TokenMix => "Token mix", "Composición de tokens";
    TokenMixNote => "Every token the period used, by how it is billed.",
        "Todos los tokens del periodo, según cómo se facturan.";
    ByTeam => "By team", "Por equipo";
    ByKey => "By API key", "Por clave de API";
    ByPerson => "By person", "Por persona";
    ByModel => "By model", "Por modelo";
    ByCustomer => "By customer", "Por cliente final";
    ByTag => "By tag", "Por etiqueta";
    DayByDay => "Day by day", "Día a día";
    DayByDayNote => "Days with traffic. Days are UTC.", "Días con tráfico. Días en UTC.";
    Lines => "Detail lines", "Líneas de detalle";
    LinesNote => "One line per day, API key and model, with a running total.",
        "Una línea por día, clave de API y modelo, con el acumulado.";
    RequestLog => "Requests", "Peticiones";
    RatesApplied => "Rates", "Tarifas";
    RatesNote => "In force when the report was generated, per million tokens.",
        "Vigentes al generar el informe, por millón de tokens.";
    Methodology => "Methodology and notes", "Metodología y notas";
    Notes => "Notes", "Notas";
    MoneySequence => "From provider cost to amount", "Del coste del proveedor al importe";

    Contents => "Contents", "Índice";
    ExecutiveSummary => "Executive summary", "Resumen ejecutivo";
    SummaryIntro => "The period at a glance, against the one before.",
        "El periodo de un vistazo, frente al anterior.";
    Consumption => "Consumption over time", "Evolución del consumo";
    ConsumptionIntro => "How spend moved through the period, and what the tokens went on.",
        "Cómo evolucionó el gasto durante el periodo y en qué se emplearon los tokens.";
    Breakdown => "Spend breakdown", "Desglose del gasto";
    BreakdownIntro => "Who spent, with which keys and on which models.",
        "Quién gastó, con qué claves y en qué modelos.";
    Annex => "Appendix: rates and methodology", "Anexo: tarifas y metodología";
    AnnexIntro => "The prices behind the amounts, and how they were worked out.",
        "Los precios detrás de los importes y cómo se calcularon.";
    RequestsIntro => "Every request of the period, oldest first.",
        "Cada petición del periodo, de la más antigua a la más reciente.";
    KeyPoints => "Key points", "Puntos clave";
    Confidential => "Confidential", "Confidencial";
    PreparedFor => "Prepared for", "Preparado para";
    AiUsage => "AI usage", "Uso de IA";
    Page => "Page", "Página";
    SheetSummary => "Summary", "Resumen";
    SheetTeams => "Teams", "Equipos";
    SheetKeys => "Keys", "Claves";
    SheetPeople => "People", "Personas";
    SheetModels => "Models", "Modelos";
    SheetCustomers => "Customers", "Clientes finales";
    SheetTags => "Tags", "Etiquetas";
    SheetDaily => "Daily", "Diario";
    SheetLines => "Detail", "Detalle";
    SheetRequests => "Requests", "Peticiones";
    SheetRates => "Rates", "Tarifas";
    SheetAbout => "About", "Notas";

    SourceCatalog => "Price list", "Lista de precios";
    SourceCustom => "Own rates", "Tarifa propia";
    SourceFree => "Free", "Gratuito";
    SourceNone => "Not priced", "Sin tarifa";
    PriceSource => "Price source", "Origen";
    InputRate => "Input", "Entrada";
    OutputRate => "Output", "Salida";
    CacheReadRate => "Cache read", "Lectura caché";
    CacheWriteRate => "Cache write", "Escritura caché";
    ReasoningRate => "Reasoning", "Razonamiento";
    ColumnGuide => "Columns of the detail sheet, and their FinOps FOCUS names",
        "Columnas de la hoja de detalle y su nombre en FinOps FOCUS";
}

impl Lang {
    /// What the report says about how its numbers were made, one paragraph
    /// per item.
    #[allow(clippy::too_many_arguments)]
    pub fn methodology(
        self,
        period: &str,
        currency: &str,
        rate: f64,
        author: &str,
        markup: f64,
        show_cost: bool,
        fingerprint: &str,
        filtered_by_model: bool,
    ) -> Vec<String> {
        let mut out = Vec::new();
        let rate_text = self.number(rate, 4);
        match self {
            Lang::En => {
                out.push(format!(
                    "Covers {period}. Days run from 00:00 to 24:00 UTC. Every request the gateway \
                     answered under the scope above is included, failed ones too: a provider may \
                     bill for a request that failed."
                ));
                out.push(
                    "Each request is priced when it finishes, from the model's rates at that \
                     moment, in billionths of a dollar. Amounts are only rounded to be shown, so \
                     totals are the exact sum of their lines."
                        .into(),
                );
                if currency == "USD" {
                    out.push("Amounts are in US dollars, the currency providers bill in.".into());
                } else {
                    out.push(format!(
                        "Amounts are in {currency}, converted from US dollars at 1 USD = \
                         {rate_text} {currency}, the rate set in the profile of {author}."
                    ));
                }
                if markup > 0.0 {
                    out.push(format!(
                        "Amounts include a {} markup on the provider's cost{}.",
                        self.percent(markup / 100.0, 2),
                        if show_cost {
                            "; provider cost and margin are shown apart"
                        } else {
                            ""
                        }
                    ));
                }
                out.push(
                    "Input tokens include cache reads and cache writes, which bill at their own \
                     rates; output tokens include reasoning. A rate left blank bills at the input \
                     or output rate."
                        .into(),
                );
                if filtered_by_model {
                    out.push(
                        "Customer and tag breakdowns are kept without the model or person, so a \
                         filter on either leaves them empty."
                            .into(),
                    );
                }
                out.push(
                    "Amounts exclude taxes. This statement supports an invoice; it is not one."
                        .into(),
                );
                out.push(format!(
                    "Data fingerprint: SHA-256 of the detail lines, {fingerprint}. The same data \
                     always gives the same fingerprint."
                ));
            }
            Lang::Es => {
                out.push(format!(
                    "Cubre {period}. Los días van de 00:00 a 24:00 UTC. Incluye cada petición que \
                     la pasarela atendió dentro del alcance indicado, también las fallidas: un \
                     proveedor puede cobrar una petición que falló."
                ));
                out.push(
                    "Cada petición se valora al terminar, con las tarifas del modelo en ese \
                     momento, en milmillonésimas de dólar. Los importes solo se redondean para \
                     mostrarlos, así que cada total es la suma exacta de sus líneas."
                        .into(),
                );
                if currency == "USD" {
                    out.push(
                        "Importes en dólares estadounidenses, la moneda en que facturan los \
                         proveedores."
                            .into(),
                    );
                } else {
                    out.push(format!(
                        "Importes en {currency}, convertidos desde dólares a 1 USD = {rate_text} \
                         {currency}, el tipo configurado en el perfil de {author}."
                    ));
                }
                if markup > 0.0 {
                    out.push(format!(
                        "Los importes incluyen un recargo del {} sobre el coste del proveedor{}.",
                        self.percent(markup / 100.0, 2),
                        if show_cost {
                            "; el coste del proveedor y el margen se muestran aparte"
                        } else {
                            ""
                        }
                    ));
                }
                out.push(
                    "Los tokens de entrada incluyen las lecturas y escrituras de caché, que se \
                     cobran a su propia tarifa; los de salida incluyen el razonamiento. Una tarifa \
                     en blanco se cobra a la de entrada o salida."
                        .into(),
                );
                if filtered_by_model {
                    out.push(
                        "Los desgloses por cliente final y etiqueta se guardan sin modelo ni \
                         persona, así que un filtro por cualquiera de ellos los deja vacíos."
                            .into(),
                    );
                }
                out.push(
                    "Importes sin impuestos. Este informe sirve de soporte a una factura; no es \
                     una factura."
                        .into(),
                );
                out.push(format!(
                    "Huella de los datos: SHA-256 de las líneas de detalle, {fingerprint}. Los \
                     mismos datos dan siempre la misma huella."
                ));
            }
        }
        out
    }
}

impl Lang {
    /// "Total amount: €48.92, +18.2% on the previous period."
    pub fn point_total(self, amount: &str, change: Option<&str>, previous: &str) -> String {
        match (self, change) {
            (Lang::En, Some(c)) => {
                format!("The period comes to {amount}, {c} on the previous one ({previous}).")
            }
            (Lang::En, None) => {
                format!("The period comes to {amount}; there was no usage the period before.")
            }
            (Lang::Es, Some(c)) => {
                format!("El periodo suma {amount}, un {c} frente al anterior ({previous}).")
            }
            (Lang::Es, None) => {
                format!("El periodo suma {amount}; en el periodo anterior no hubo uso.")
            }
        }
    }

    pub fn point_leader(self, what: &str, name: &str, amount: &str, share: &str) -> String {
        match self {
            Lang::En => {
                format!("{name} is the {what} with the most spend: {amount}, {share} of the total.")
            }
            Lang::Es => format!("{name} es {what} con más gasto: {amount}, el {share} del total."),
        }
    }

    pub fn point_cache(self, share: &str, saved: Option<&str>) -> String {
        match (self, saved) {
            (Lang::En, Some(s)) => format!("Cache reads served {share} of the input, saving about {s} at today's rates."),
            (Lang::En, None) => format!("Cache reads served {share} of the input."),
            (Lang::Es, Some(s)) => format!("Las lecturas de caché cubrieron el {share} de la entrada y ahorraron unos {s} con las tarifas actuales."),
            (Lang::Es, None) => format!("Las lecturas de caché cubrieron el {share} de la entrada."),
        }
    }

    pub fn point_failures(self, count: &str, share: &str) -> String {
        match self {
            Lang::En => format!("{count} requests failed, {share} of the total."),
            Lang::Es => format!("Fallaron {count} peticiones, el {share} del total."),
        }
    }

    pub fn point_peak(self, day: &str, amount: &str) -> String {
        match self {
            Lang::En => format!("The busiest day was {day}, at {amount}."),
            Lang::Es => format!("El día de más gasto fue el {day}, con {amount}."),
        }
    }

    /// "the team", "el equipo": how a leader is introduced.
    pub fn the(self, what: T) -> String {
        let word = self.t(what).to_lowercase();
        match (self, what) {
            (Lang::En, _) => format!("the {word}"),
            (Lang::Es, T::Person) => format!("la {word}"),
            (Lang::Es, T::Key) => format!("la {word}"),
            (Lang::Es, _) => format!("el {word}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_follow_each_languages_separators() {
        assert_eq!(Lang::En.number(1234567.891, 2), "1,234,567.89");
        assert_eq!(Lang::Es.number(1234567.891, 2), "1.234.567,89");
        assert_eq!(
            Lang::Es.number(1234.0, 0),
            "1234",
            "Spanish groups from five digits"
        );
        assert_eq!(Lang::Es.number(12345.0, 0), "12.345");
        assert_eq!(Lang::En.number(-0.5, 2), "−0.50");
        assert_eq!(Lang::En.number(-0.001, 2), "0.00", "no minus on a zero");
    }

    #[test]
    fn money_goes_where_each_language_puts_it() {
        assert_eq!(Lang::En.money("USD", 1234.5), "$1,234.50");
        assert_eq!(Lang::Es.money("EUR", 1234.5), "1234,50 €");
        assert_eq!(Lang::En.money("EUR", -3.0), "−€3.00");
        assert_eq!(
            Lang::Es.money("USD", 0.0012),
            "0,0012 $",
            "tiny amounts keep four places"
        );
        assert_eq!(Lang::En.percent(0.1234, 1), "12.3%");
        assert_eq!(Lang::Es.percent(0.1234, 1), "12,3 %");
        assert_eq!(Lang::Es.change(0.5), "+50,0 %");
    }

    #[test]
    fn the_account_language_wins_then_the_browser_then_english() {
        assert_eq!(Lang::pick(Some("es-ES"), Some("en")), Lang::Es);
        assert_eq!(Lang::pick(None, Some("fr-FR,es;q=0.8,en;q=0.5")), Lang::Es);
        assert_eq!(Lang::pick(Some("fr"), None), Lang::En);
        assert_eq!(Lang::pick(None, None), Lang::En);
    }

    #[test]
    fn a_period_reads_naturally() {
        let d = |m, d| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
        assert_eq!(
            Lang::Es.period(d(9, 7), d(10, 6)),
            "7 sept – 6 oct 2026 · 30 días"
        );
        assert_eq!(
            Lang::En.period(d(10, 6), d(10, 6)),
            "6 Oct – 6 Oct 2026 · 1 day"
        );
    }
}
