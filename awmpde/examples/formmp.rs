use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use awmpde::{form_or_multipart_unwrap, FromActixMultipart};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AnimalDesc {
    name: String,
    kind: String,
}

#[derive(Debug, Deserialize, FromActixMultipart)]
pub struct IsAnimalRequest {
    #[serde_json]
    animal_desc: AnimalDesc,
}

#[form_or_multipart_unwrap]
async fn is_animal(
    awmpde::FormOrMultipart(req): awmpde::FormOrMultipart<IsAnimalRequest>,
) -> HttpResponse {
    let IsAnimalRequest {
        animal_desc: AnimalDesc { kind, .. },
    } = req;
    let kind: &str = &kind;

    let out = match kind {
        "dog" => true,
        "cat" => true,
        _ => false,
    };

    actix_web::HttpResponse::Ok().body(format!("out is {}", out))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::DefaultHeaders::new().header("X-Version", "0.2"))
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .route("/is_animal", web::post().to(is_animal))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
