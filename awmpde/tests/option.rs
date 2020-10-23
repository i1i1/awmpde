use actix_web::post;
use awmpde::FromActixMultipart;

#[derive(FromActixMultipart)]
struct Help {
    _img: awmpde::File<Vec<u8>>,
    _animal: Option<String>,
}

#[post("/test")]
async fn test(help: awmpde::Multipart<Help>) -> actix_web::web::Json<()> {
    let _h: Help = help.into_inner().await.unwrap();
    actix_web::web::Json(())
}
