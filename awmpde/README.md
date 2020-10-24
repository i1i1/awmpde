# awmpde

[![Docs](https://docs.rs/awmpde/badge.svg)](https://docs.rs/crate/awmpde/)
[![Crates.io](https://img.shields.io/crates/v/awmpde.svg)](https://crates.io/crates/awmpde)

A convenience library for working with multipart/form-data in [`actix-web`](https://docs.rs/actix-web) 3.x.

This library uses [`actix-multipart`](https://docs.rs/actix-multipart) internally, and is not a replacement
for `actix-multipart`.

## Usage

This crate supports `actix-web` of versions 3.x only.

```toml
awmpde = "0.1.1"
```

### Example

```rust
use actix_web::{web, App, post, Error, HttpResponse, HttpServer};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Description {
    genre: String,
    author: String,
    year: i64,
}

#[derive(FromActixMultipart)]
struct Book {
    file: awmpde::File<Vec<u8>>,
    #[serde_json]
    description: Description,
}

#[post("/put_book")]
async fn put_book(book: awmpde::Multipart<Book>) -> Result<HttpResponse, Error> {
    let Book {
        file: awmpde::File { name, inner, ..},
        description
    } = book.into_inner()?;
    std::fs::write(std::path::Path::from("books").join(name), &*inner)?;
    let body = format!("Wrote book with description {:?}", desription);
    Ok(HttpResponse::Ok().body(body))
}

#[actix_rt::main]
async fn main() -> Result<(), std::io::Error> {
    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .route(put_book)
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}
```

Current version: 0.1.1

License: MIT
