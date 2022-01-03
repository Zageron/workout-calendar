mod youtube;

#[macro_use]
extern crate maplit;
extern crate dotenv;
extern crate env_logger;
extern crate google_youtube3 as youtube3;

use actix_files::Files;
use actix_http::body::{BoxBody, EitherBody};
use actix_web::{
    dev::ServiceResponse,
    guard,
    http::{header, StatusCode},
    middleware::{ErrorHandlerResponse, ErrorHandlers, Logger, NormalizePath, TrailingSlash},
    web, App, HttpRequest, HttpResponse, HttpServer, Result,
};
use dotenv::dotenv;
use handlebars::Handlebars;
use mongodb::bson::doc;
use youtube3::YouTube;
use yup_oauth2::{self, InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use serde::Deserialize;
use std::{collections::BTreeMap, io};

#[derive(Clone, Debug, Deserialize)]
struct RootData {
    data: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
struct LearningData {
    data: BTreeMap<String, String>,
}

// Macro documentation can be found in the actix_web_codegen crate
async fn index(
    hb: web::Data<Handlebars<'_>>,
    root_template_data: web::Data<RootData>,
    _req: HttpRequest,
) -> HttpResponse {
    let body = hb
        .render("pages/calendar", &root_template_data.data)
        .unwrap();
    HttpResponse::Ok().body(body)
}

#[derive(Deserialize, Debug)]
struct Info {
    entry_id: u32,
}

async fn study(_hb: web::Data<Handlebars<'_>>) -> HttpResponse {
    HttpResponse::Ok().body("You need to pick an entry to study.")
}

async fn study_entry(_hb: web::Data<Handlebars<'_>>, info: web::Path<Info>) -> HttpResponse {
    println!("Entry {:?}", info.entry_id);
    HttpResponse::Ok().body(format!("GET: {:?}", info))
}

async fn study_submit(_hb: web::Data<Handlebars<'_>>, info: web::Path<Info>) -> HttpResponse {
    HttpResponse::Ok().body(format!("POST: {:?}", info))
}

async fn learning_entry(
    hb: web::Data<Handlebars<'_>>,
    root_template_data: web::Data<RootData>,
    learning_data: web::Data<LearningData>,
) -> HttpResponse {
    let mut data: BTreeMap<&String, &String> = BTreeMap::new();
    data.extend(root_template_data.data.iter());
    data.extend(learning_data.data.iter());

    let body = hb.render("pages/learn", &data).unwrap();
    println!("{}", body);
    HttpResponse::Ok().body(body)
}

async fn youtube() -> HttpResponse {
    let secret = yup_oauth2::read_application_secret(".secrets/client_secret.json")
            .await
            .expect(".secrets/client_secret.json");

    // Create an authenticator that uses an InstalledFlow to authenticate. The
    // authentication tokens are persisted to a file named tokencache.json. The
    // authenticator takes care of caching tokens to disk and refreshing tokens once
    // they've expired.
    let auth = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
        .persist_tokens_to_disk(".secrets/tokencache.json")
        .build()
        .await
        .unwrap();

    let connector = hyper_rustls::HttpsConnector::with_native_roots();
    let client = YouTube::new(hyper::Client::builder().build(connector), auth);
    let playlist_items = youtube::request_playlists(&client).await;
    HttpResponse::Ok().body(format!("{:?}", playlist_items))
}

async fn copyright(hb: web::Data<Handlebars<'_>>) -> HttpResponse {
    let data = btreemap! {
        "author".to_string() => "Zageron".to_string(),
        "year".to_string() => "2021".to_string()
    };

    let body: String = hb.render("copyright", &data).unwrap();
    HttpResponse::Ok().body(body)
}

async fn robots(hb: web::Data<Handlebars<'_>>) -> HttpResponse {
    let data = btreemap! {
        "url".to_string() => "https://www.zageron.com".to_string(),
    };

    let body: String = hb.render("robots", &data).unwrap();

    let mut builder = HttpResponse::Ok();
    builder.insert_header(header::ContentType(mime::TEXT_PLAIN_UTF_8));
    builder.body(body)
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenv().ok();

    #[cfg(debug_assertions)]
    std::env::set_var("RUST_LOG", "actix_web=info");
    #[cfg(debug_assertions)]
    env_logger::init();

    let mut handlebars = Handlebars::new();

    handlebars.set_dev_mode(cfg!(debug_assertions));

    handlebars
        .register_templates_directory(".hbs", "templates/")
        .unwrap();
    let handlebars_ref = web::Data::new(handlebars);

    let _route =
        std::env::var("ROUTE").expect("Route is not set and is inferred to be unnecessary.");
    let _base_url = std::env::var("BASE_URL");
    assert!(_base_url.is_ok());

    let root_template_data = RootData {
        data: btreemap! {
            "title".to_string() => "Learn - Splatoon Callouts".to_string(),
            "author".to_string() => "Zageron".to_string(),
            "url".to_string() => _base_url.unwrap(),
            "description".to_string() => "A Spaced Repetition site for memorizing Splatoon 2 callouts.".to_string(),
            "route".to_string() => _route,
            "parent".to_string() => "root".to_string()
        },
    };

    let learning_data = LearningData {
        data: btreemap! {
            "item_header".to_string() => "Test Item".to_string(),
            "item_title".to_string() => "Sploosh".to_string(),
            "item_description".to_string() => "Splooshes be splooshing.".to_string(),
            "item_footer".to_string() => "You've learned this already.".to_string(),
        },
    };

    HttpServer::new(move || {
        App::new()
            .app_data(handlebars_ref.clone())
            .app_data(web::Data::new(root_template_data.clone()))
            .app_data(web::Data::new(learning_data.clone()))
            .wrap(NormalizePath::new(TrailingSlash::Trim))
            .wrap(error_handlers())
            .wrap(Logger::default())
            .service(
                web::scope("/learn").service(
                    web::resource(["/", ""])
                        .guard(guard::Get())
                        .to(learning_entry),
                ),
            )
            .service(
                web::scope("/study")
                    .service(web::resource(["/", ""]).guard(guard::Get()).to(study))
                    .service(
                        web::resource("{entry_id}")
                            .guard(guard::Get())
                            .to(study_entry),
                    )
                    .service(
                        web::resource("{entry_id}")
                            .guard(guard::Post())
                            .to(study_submit),
                    ),
            )
            .service(web::resource("/").guard(guard::Get()).to(index))
            .service(web::resource("youtube").guard(guard::Get()).to(youtube))
            .service(web::resource("copyright").guard(guard::Get()).to(copyright))
            .service(web::resource("robots.txt").guard(guard::Get()).to(robots))
            .service(Files::new("/", "./data"))
    })
    .bind("0.0.0.0:8081")?
    .run()
    .await
}

// Custom error handlers, to return HTML responses when an error occurs.
fn error_handlers() -> ErrorHandlers<BoxBody> {
    ErrorHandlers::new().handler(StatusCode::NOT_FOUND, not_found)
}

// Error handler for a 404 Page not found error.
fn not_found<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<BoxBody>> {
    let response = get_error_response(&res, "Page not found");
    Ok(ErrorHandlerResponse::Response(res.into_response(response)))
}

// Generic error handler.
fn get_error_response<B>(
    res: &ServiceResponse<B>,
    error: &str,
) -> HttpResponse<EitherBody<BoxBody>> {
    let request = res.request();
    let data = request.app_data::<web::Data<RootData>>().unwrap();

    // Provide a fallback to a simple plain text response in case an error occurs during the
    // rendering of the error page.
    let fallback = |e: &str| {
        HttpResponse::build(res.status())
            .content_type("text/plain")
            .body(e.to_string())
            .map_into_left_body()
    };

    let hb = request
        .app_data::<web::Data<Handlebars>>()
        .map(|t| t.get_ref());
    match hb {
        Some(hb) => {
            let mut merged_data: BTreeMap<&String, &String> = BTreeMap::new();
            let error_data = btreemap! {
                "error".to_string() => error.to_string(),
                "status_code".to_string() =>  res.status().to_string(),
                "page".to_string() =>  request.uri().to_string()
            };
            merged_data.extend(data.data.iter());
            merged_data.extend(error_data.iter());

            let body = hb.render("partials/404", &merged_data);

            match body {
                Ok(body) => HttpResponse::build(res.status())
                    .content_type("text/html")
                    .body(body)
                    .map_into_left_body(),
                Err(_) => fallback(error),
            }
        }
        None => fallback(error),
    }
}
