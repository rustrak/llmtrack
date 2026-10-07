//! What every paged list takes and returns, so each endpoint only says which
//! fields it sorts by and what its own filters mean.
//!
//! `?page=2&per_page=50&sort=-created_at&q=prod`, answered with
//! `{data, total, page, per_page}`. Sort fields are public names mapped to
//! SQL by the endpoint; anything not on its list is a 400, so a column name
//! never comes from the request.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, FieldErrorCode};

pub const DEFAULT_PER_PAGE: i64 = 50;
pub const MAX_PER_PAGE: i64 = 200;

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    /// A field name, `-` first for descending.
    pub sort: Option<String>,
    /// Free text, matched by the endpoint against the columns it chooses.
    pub q: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Paged<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

impl ListQuery {
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> i64 {
        self.per_page
            .unwrap_or(DEFAULT_PER_PAGE)
            .clamp(1, MAX_PER_PAGE)
    }

    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.per_page()
    }

    /// The `ORDER BY` body for `sort`, from `fields` (public name, SQL
    /// expression). Rows without a value go last either way, and `tiebreak`
    /// keeps pages stable when values repeat.
    pub fn order_by(
        &self,
        fields: &[(&str, &'static str)],
        default: &str,
        tiebreak: &'static str,
    ) -> AppResult<String> {
        let sort = self
            .sort
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(default);
        let (name, dir) = match sort.strip_prefix('-') {
            Some(name) => (name, "DESC"),
            None => (sort, "ASC"),
        };
        let Some((_, column)) = fields.iter().find(|(field, _)| *field == name) else {
            let allowed: Vec<_> = fields.iter().map(|(field, _)| *field).collect();
            return Err(AppError::Validation(format!(
                "cannot sort by '{name}'; use one of {}",
                allowed.join(", ")
            ))
            .with_field("sort", FieldErrorCode::Invalid));
        };
        Ok(format!(
            "({column} IS NULL), {column} {dir}, {tiebreak} {dir}"
        ))
    }

    /// `q` as a lowercase `LIKE` pattern, matched with `ESCAPE '\'` against
    /// `LOWER(column)`; `None` when there is nothing to search.
    pub fn search(&self) -> Option<String> {
        let q = self.q.as_deref()?.trim();
        (!q.is_empty()).then(|| format!("%{}%", escape_like(&q.to_lowercase())))
    }

    pub fn paged<T>(&self, data: Vec<T>, total: i64) -> Paged<T> {
        Paged {
            data,
            total,
            page: self.page(),
            per_page: self.per_page(),
        }
    }
}

/// `value` with `LIKE`'s wildcards taken literally, for `ESCAPE '\'`.
pub fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(page: Option<i64>, per_page: Option<i64>, sort: Option<&str>) -> ListQuery {
        ListQuery {
            page,
            per_page,
            sort: sort.map(str::to_string),
            q: None,
        }
    }

    #[test]
    fn page_and_size_have_defaults_and_bounds() {
        let q = query(None, None, None);
        assert_eq!((q.page(), q.per_page(), q.offset()), (1, 50, 0));
        let q = query(Some(0), Some(10_000), None);
        assert_eq!((q.page(), q.per_page()), (1, MAX_PER_PAGE));
        let q = query(Some(3), Some(20), None);
        assert_eq!(q.offset(), 40);
    }

    #[test]
    fn sort_maps_listed_fields_only() {
        let fields = [("name", "k.name"), ("spend", "k.spend_nanos")];
        let order = |s| query(None, None, s).order_by(&fields, "-spend", "k.id");
        assert_eq!(
            order(None).unwrap(),
            "(k.spend_nanos IS NULL), k.spend_nanos DESC, k.id DESC"
        );
        assert_eq!(
            order(Some("name")).unwrap(),
            "(k.name IS NULL), k.name ASC, k.id ASC"
        );
        assert!(order(Some("k.name; DROP TABLE x")).is_err());
    }

    #[test]
    fn search_is_lowercase_and_literal() {
        let q = |s: &str| ListQuery {
            q: Some(s.into()),
            ..Default::default()
        };
        assert_eq!(q("  ").search(), None);
        assert_eq!(q("Prod").search().unwrap(), "%prod%");
        assert_eq!(q("50%_a\\b").search().unwrap(), "%50\\%\\_a\\\\b%");
    }
}
