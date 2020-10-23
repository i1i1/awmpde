use actix_web::{middleware, web, App, Error, HttpResponse, HttpServer};
use awmpde::FromActixMultipart;
use serde::Deserialize;

use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct AnimalDesc {
    name: String,
    kind: String,
}

#[derive(Debug, FromActixMultipart)]
pub struct IsAnimalRequest {
    img: awmpde::File<awmpde::RgbImage>,
    #[serde_json]
    animal_desc: AnimalDesc,
}

async fn is_animal(
    req: awmpde::Multipart<IsAnimalRequest>,
) -> Result<HttpResponse, Error> {
    let IsAnimalRequest {
        img,
        animal_desc: AnimalDesc { kind, .. },
    } = req.into_inner().await?;
    let kind: &str = &kind;

    let out = match kind {
        "dog" => true,
        "cat" => true,
        _ => false,
    };

    if out {
        let awmpde::File { name, inner, .. } = img;
        inner.save(Path::new("animals/").join(name)).unwrap();
    }

    Ok(actix_web::HttpResponse::Ok().body(""))
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
