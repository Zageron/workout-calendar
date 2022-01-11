mod youtube;

#[macro_use]
extern crate maplit;
extern crate dotenv;
extern crate env_logger;
extern crate google_youtube3 as youtube3;
extern crate json_value_merge;

use json_value_merge::Merge;

use actix_files::Files;
use actix_http::body::{BoxBody, EitherBody};
use actix_web::{
    dev::ServiceResponse,
    guard,
    http::{self, header, StatusCode},
    middleware::{ErrorHandlerResponse, ErrorHandlers, Logger, NormalizePath, TrailingSlash},
    web, App, HttpRequest, HttpResponse, HttpServer, Result,
};
use dotenv::dotenv;
use handlebars::Handlebars;
use mongodb::bson::doc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io,
    ops::{Deref, DerefMut},
    sync::RwLock,
};
use youtube::PlaylistWrapper;
use youtube3::{
    hyper, hyper_rustls,
    oauth2::{self, InstalledFlowAuthenticator, InstalledFlowReturnMethod},
    YouTube,
};

#[derive(Clone)]
struct UserState {
    playlist: Option<PlaylistWrapper>,
}

impl UserState {
    fn set_playlist(&mut self, playlist: PlaylistWrapper) {
        self.playlist = Some(playlist);
    }
}

impl Deref for UserState {
    type Target = Option<PlaylistWrapper>;

    fn deref(&self) -> &Self::Target {
        &self.playlist
    }
}

impl DerefMut for UserState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.playlist
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RootData {
    data: BTreeMap<String, String>,
}

// Macro documentation can be found in the actix_web_codegen crate
async fn index(
    hb: web::Data<Handlebars<'_>>,
    root_template_data: web::Data<Value>,
    state: web::Data<RwLock<UserState>>,
    _req: HttpRequest,
) -> HttpResponse {
    let mut object: Value = Value::default();
    let what = &*root_template_data.into_inner();
    if let Some(obj) = what.as_object() {
        object.merge_in("/", Value::Object(obj.clone())).unwrap();
    }

    if let Some(ref unwrapped_data) = state.read().unwrap().playlist {
        if let Ok(new_value) = serde_json::to_value(unwrapped_data) {
            object.merge_in("/", new_value).unwrap();
        }
    }

    let body = hb.render("pages/calendar", &object).unwrap();
    HttpResponse::Ok().body(body)
}

#[derive(Deserialize)]
struct YouTubeUrlForm {
    url: String,
}

fn redirect_to(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header((http::header::LOCATION, location))
        .finish()
}

async fn youtube_add_playlist(
    yt_client: web::Data<YouTube>,
    form: web::Form<YouTubeUrlForm>,
    state: web::Data<RwLock<UserState>>,
) -> HttpResponse {
    if let Ok(url) = url::Url::parse(&form.url) {
        if let Some((_query, arg)) = url.query_pairs().next() {
            if let Some(playlist) = youtube::request_playlist(&yt_client, &arg).await {
                let mut mut_state = state.write().unwrap();
                mut_state.set_playlist(playlist);
            }
        }
    }

    redirect_to("/")
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

async fn initialize_youtube() -> YouTube {
    let secret = oauth2::read_application_secret(".secrets/client_secret.json")
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

    YouTube::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
        auth,
    )
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenv().ok();

    #[cfg(debug_assertions)]
    std::env::set_var("RUST_LOG", "actix_web=info");
    #[cfg(debug_assertions)]
    env_logger::init();

    let state = UserState { playlist: None };
    let youtube_client = initialize_youtube().await;

    let mut handlebars = Handlebars::new();
    handlebars.set_dev_mode(cfg!(debug_assertions));
    handlebars
        .register_templates_directory(".hbs", "templates/")
        .unwrap();

    let handlebars_ref = web::Data::new(handlebars);

    let route =
        std::env::var("ROUTE").expect("Route is not set and is inferred to be unnecessary.");
    let base_url = std::env::var("BASE_URL");
    assert!(base_url.is_ok());

    let root_template_data = json!({
        "title": "Learn - Splatoon Callouts",
        "author": "Zageron",
        "url": base_url.unwrap(),
        "description": "A Spaced Repetition site for memorizing Splatoon 2 callouts.",
        "route": route,
        "parent": "root",
    });

    HttpServer::new(move || {
        App::new()
            .app_data(handlebars_ref.clone())
            .app_data(web::Data::new(root_template_data.clone()))
            .app_data(web::Data::new(youtube_client.clone()))
            .app_data(web::Data::new(RwLock::new(state.clone())))
            .wrap(NormalizePath::new(TrailingSlash::Trim))
            .wrap(error_handlers())
            .wrap(Logger::default())
            .service(web::resource("/").guard(guard::Get()).to(index))
            .service(
                web::resource("youtube_add_playlist")
                    .guard(guard::Post())
                    .to(youtube_add_playlist),
            )
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
