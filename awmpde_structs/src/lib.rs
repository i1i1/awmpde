#[macro_use]
extern crate derive_deref;

#[cfg(feature = "chrono")]
pub mod chrono_types;
pub mod images;
#[cfg(feature = "uuid")]
pub mod uuid_field;

mod basic;
pub use basic::*;

use actix_web::{dev::Payload, http::StatusCode, FromRequest, HttpRequest};
use displaydoc::Display;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::StreamExt;
use mime::Mime;
use serde::de::DeserializeOwned;
use thiserror::Error;

use std::collections::HashMap;
use std::future::Future;
use std::io::Cursor;
use std::marker::PhantomData;
use std::path::PathBuf;

#[derive(Debug, Error, Display)]
pub enum Error {
    /// Failed to deserialize
    SerializationError(#[from] serde_json::error::Error),
    /// Failed to decode image
    ImageDecodeError(#[from] image::error::ImageError),
    /// Failed to decode jpeg image
    MozjpgDecodeError,
    /// No such field in request `{0}'
    NoFieldError(String),
    /// No such filename for file
    NoFilenameError,
    /// Filename must be valid UTF8
    FilenameUTF8Error,
    /// Failed to parse UTF8 string
    StringDecodeError(#[from] std::string::FromUtf8Error),
    /// {0}
    ActixWebError(#[from] actix_web::error::Error),
    /// Failed to find field {0:?} in request
    FieldError(&'static str),
    /// Unknown Error. Usually for empty error type
    UnknownError,

    #[cfg(feature = "uuid")]
    /// Failed to parse UUID
    UUIDParseError(#[from] uuid::Error),
}

impl actix_web::error::ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::ActixWebError(e) => e.as_response_error().status_code(),
            _ => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(
        &self,
    ) -> actix_web::web::HttpResponse<actix_web::dev::Body> {
        actix_web::dev::HttpResponseBuilder::new(self.status_code())
            .set_header(
                actix_web::http::header::CONTENT_TYPE,
                "text/html; charset=utf-8",
            )
            .body(self.to_string())
    }
}

impl std::convert::From<()> for Error {
    fn from(_: ()) -> Self {
        Self::UnknownError
    }
}

pub struct Multipart<T> {
    /// Actual multipart
    pub mp: actix_multipart::Multipart,
    /// Marker of phantomdata
    pub _marker: PhantomData<T>,
}

pub trait FromField: Sized {
    /// The associated error which can be returned.
    type Error: Into<actix_http::error::Error>;
    /// Future that resolves to a Self
    type Future: Future<Output = Result<Self, Self::Error>> + 'static;

    fn from_field(field: actix_multipart::Field) -> Self::Future;
}

/// Trait which implements macro for your structures
///
/// FromRequest won't do because of static construction of future (static lifetimes).
///
/// We need to have lifetime because we depend upon multipart and branch into
/// parsing different fields.
pub trait FromMultipart<'a>: Sized {
    /// The associated error which can be returned.
    type Error: Into<actix_web::error::Error>;
    /// Future that resolves to a Self
    type Future: Future<Output = Result<Self, Self::Error>> + 'a;

    fn from_multipart(mp: actix_multipart::Multipart) -> Self::Future;
}

impl<'a, T: FromMultipart<'a>> Multipart<T> {
    #[inline]
    pub async fn into_inner(self) -> Result<T, Error> {
        Ok(T::from_multipart(self.mp).await.map_err(|e| e.into())?)
    }
}

impl<'a, T: Sized> FromRequest for Multipart<T> {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;
    type Config = ();

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let mp = actix_multipart::Multipart::from_request(req, payload);
        async {
            Ok(Self {
                mp: mp.await?,
                _marker: Default::default(),
            })
        }
        .boxed_local()
    }
}

/// Type for annotating unwrapping of FormOrMultipartFuture by
/// `form_or_multipart_unwrap`.
#[derive(Deref, DerefMut, Debug, Clone, Copy, Display)]
pub struct FormOrMultipart<T>(pub T);

/// Type for accepting request both with types urlencoded and multipart.
pub enum FormOrMultipartFuture<T> {
    /// url encoded form
    Form(actix_web::web::Form<T>),
    /// multipart request
    Multipart(Multipart<T>),
}

impl<'a, T: FromMultipart<'a>> FormOrMultipartFuture<T> {
    /// If type match returns inner type
    pub async fn into_inner(self) -> Result<T, Error> {
        Ok(match self {
            Self::Form(f) => f.into_inner(),
            Self::Multipart(m) => m.into_inner().await?,
        })
    }
}

impl<T: DeserializeOwned + 'static> FromRequest for FormOrMultipartFuture<T> {
    type Config = ();
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        use actix_web::HttpMessage;

        let cont_type: &str = &req.content_type().to_lowercase();
        if cont_type == "application/x-www-form-urlencoded" {
            actix_web::web::Form::from_request(req, payload)
                .map(move |res| Ok(Self::Form(res?)))
                .boxed_local()
        } else {
            Multipart::from_request(req, payload)
                .map(move |res| Ok(Self::Multipart(res?)))
                .boxed_local()
        }
    }
}

// TODO: doesn't assume UTF8
pub fn get_content_disposition(
    field: &actix_multipart::Field,
) -> HashMap<Box<str>, Box<str>> {
    let mut out = HashMap::new();
    let disp = field
        .headers()
        .get("content-disposition")
        .expect("Multipart always should have content-disposition")
        .to_str()
        .expect("TODO: for now assume `content-disposition' is UTF8");

    let mut splt = disp.split(';').map(|f| f.trim());
    assert_eq!(splt.next().unwrap(), "form-data");

    for f in splt {
        let vec = f.splitn(2, '=').collect::<Vec<_>>();
        let k = vec[0];
        let v = vec[1]
            .strip_prefix("\"")
            .unwrap()
            .strip_suffix("\"")
            .unwrap();

        out.insert(
            k.to_string().into_boxed_str(),
            v.to_string().into_boxed_str(),
        );
    }

    out
}
