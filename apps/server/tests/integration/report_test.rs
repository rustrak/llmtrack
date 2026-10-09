use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};
use serde_json::json;

use crate::common::test_app;
use crate::integration::keys_test::create_team;
use crate::integration::usage_test::{key, spend};

fn workbook(bytes: Vec<u8>) -> Xlsx<Cursor<Vec<u8>>> {
    Xlsx::new(Cursor::new(bytes)).expect("a readable workbook")
}

/// The first number in the row whose first cell is `label`.
fn value_of(book: &mut Xlsx<Cursor<Vec<u8>>>, sheet: &str, label: &str) -> f64 {
    let range = book.worksheet_range(sheet).expect("the sheet");
    range
        .rows()
        .find(|row| matches!(row.first(), Some(Data::String(s)) if s == label))
        .and_then(|row| {
            row.iter().skip(1).find_map(|c| match c {
                Data::Float(f) => Some(*f),
                Data::Int(i) => Some(*i as f64),
                _ => None,
            })
        })
        .unwrap_or_else(|| panic!("no '{label}' row in {sheet}"))
}

#[actix_web::test]
async fn a_pdf_report_downloads_as_a_file() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let acme_key = key(&mut app, acme, "acme-backend").await;
    spend(&app, acme, acme_key, "gpt-4o", 3_000_000_000, 0, 200).await;
    spend(&app, acme, acme_key, "claude", 1_000_000_000, 2, 500).await;

    let (status, headers, body) = app
        .call_bytes(
            "/api/usage/export?format=pdf&client=Initech&reference=PO-7&markup=20",
            &[],
        )
        .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    assert_eq!(headers.get("content-type").unwrap(), "application/pdf");
    let disposition = headers
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(disposition.starts_with("attachment;"), "{disposition}");
    assert!(disposition.contains(".pdf"), "{disposition}");
    assert!(body.starts_with(b"%PDF-"));
    assert!(body.len() > 2_000);
}

#[actix_web::test]
async fn an_xlsx_report_speaks_the_readers_language_and_currency() {
    let mut app = test_app().await;
    app.admin().await;
    let (status, _) = app
        .call(
            "PATCH",
            "/auth/me",
            Some(json!({"language": "es", "currency": "EUR", "currency_rate": 0.5})),
        )
        .await;
    assert_eq!(status, 200);
    let acme = create_team(&mut app, "Acme", &[]).await;
    let acme_key = key(&mut app, acme, "acme-backend").await;
    spend(&app, acme, acme_key, "gpt-4o", 3_000_000_000, 0, 200).await;
    spend(&app, acme, acme_key, "claude", 1_000_000_000, 1, 200).await;

    let (status, headers, body) = app
        .call_bytes("/api/usage/export?format=xlsx&markup=10", &[])
        .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    let mut book = workbook(body);
    let names = book.sheet_names();
    assert_eq!(names[0], "Resumen", "{names:?}");
    assert!(names.contains(&"Modelos".to_string()), "{names:?}");
    assert!(names.contains(&"Detalle".to_string()), "{names:?}");
    // $4 at 0.5 EUR per dollar, plus 10 %.
    let total = value_of(&mut book, "Resumen", "Importe total");
    assert!((total - 2.2).abs() < 1e-9, "{total}");
    // The provider's cost stays out of a document meant for the client.
    let range = book.worksheet_range("Resumen").unwrap();
    assert!(!range
        .rows()
        .any(|r| matches!(&r[0], Data::String(s) if s == "Coste del proveedor")));
}

#[actix_web::test]
async fn the_provider_cost_and_margin_show_when_asked() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let acme_key = key(&mut app, acme, "k").await;
    spend(&app, acme, acme_key, "gpt-4o", 2_000_000_000, 0, 200).await;

    let (status, _, body) = app
        .call_bytes(
            "/api/usage/export?format=xlsx&markup=25&show_cost=true",
            &[("Accept-Language", "en-GB,en;q=0.9")],
        )
        .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let mut book = workbook(body);
    assert_eq!(book.sheet_names()[0], "Summary");
    assert_eq!(value_of(&mut book, "Summary", "Provider cost"), 2.0);
    assert_eq!(value_of(&mut book, "Summary", "Margin"), 0.5);
    assert_eq!(value_of(&mut book, "Summary", "Total amount"), 2.5);
}

#[actix_web::test]
async fn sections_left_out_are_not_in_the_workbook() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let acme_key = key(&mut app, acme, "k").await;
    spend(&app, acme, acme_key, "gpt-4o", 1_000_000_000, 0, 200).await;

    let (status, _, body) = app
        .call_bytes(
            "/api/usage/export?format=xlsx&sections=models,requests",
            &[],
        )
        .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let mut book = workbook(body);
    let names = book.sheet_names();
    assert!(names.contains(&"Models".to_string()), "{names:?}");
    assert!(names.contains(&"Requests".to_string()), "{names:?}");
    assert!(!names.contains(&"Teams".to_string()), "{names:?}");
    let requests = book.worksheet_range("Requests").unwrap();
    assert_eq!(requests.rows().count(), 2, "a header and the one request");
}

#[actix_web::test]
async fn bad_export_options_are_refused() {
    let mut app = test_app().await;
    app.admin().await;
    for uri in [
        "/api/usage/export?format=docx",
        "/api/usage/export?format=pdf&markup=-5",
        "/api/usage/export?format=pdf&markup=5000",
        "/api/usage/export?format=pdf&sections=nope",
        "/api/usage/export?format=pdf&from=2026-05-02&to=2026-05-01",
    ] {
        let (status, _, body) = app.call_bytes(uri, &[]).await;
        assert_eq!(status, 400, "{uri}: {}", String::from_utf8_lossy(&body));
    }
}

#[actix_web::test]
async fn a_member_cannot_export_a_team_they_are_not_in() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    app.logout_locally();
    app.login_as("member@example.com", "member").await;
    let (status, _, _) = app
        .call_bytes(&format!("/api/usage/export?format=pdf&team_id={acme}"), &[])
        .await;
    assert!(status == 403 || status == 404, "{status}");

    app.logout_locally();
    let (status, _, _) = app.call_bytes("/api/usage/export?format=pdf", &[]).await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn filtered_to_a_team_the_team_is_the_client() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let acme_key = key(&mut app, acme, "k").await;
    spend(&app, acme, acme_key, "gpt-4o", 1_000_000_000, 0, 200).await;

    let (status, _, body) = app
        .call_bytes(
            &format!("/api/usage/export?format=xlsx&team_id={acme}"),
            &[],
        )
        .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let mut book = workbook(body);
    let names = book.sheet_names();
    assert!(
        !names.contains(&"Teams".to_string()),
        "one team, no table of teams: {names:?}"
    );
    assert!(names.contains(&"Keys".to_string()), "{names:?}");
    let summary = book.worksheet_range("Summary").unwrap();
    let client = summary
        .rows()
        .find(|r| matches!(&r[0], Data::String(s) if s == "Client"))
        .map(|r| r[1].to_string());
    assert_eq!(client.as_deref(), Some("Acme"));
}
