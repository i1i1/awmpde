#[macro_use]
extern crate derive_deref;

use actix_web::{dev::Payload, http::StatusCode, FromRequest, HttpRequest};
use displaydoc::Display;
use either::Either;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::StreamExt;
use image::io::Reader as ImgReader;
use image::{Bgr, Bgra, ImageFormat};
use mime::Mime;
use thiserror::Error;

use std::collections::HashMap;
use std::future::Future;
use std::io::Cursor;
use std::marker::PhantomData;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct File<T> {
    pub name: PathBuf,
    pub mime: Mime,
    pub inner: T,
}

impl<T> File<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }
}

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
    /// Failed to parse Utf8 string
    StringDecodeError(#[from] std::string::FromUtf8Error),
    /// Actix Error
    ActixWebError(#[from] actix_web::error::Error),
    /// Failed to find field {0:?} in request
    FieldError(&'static str),
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
        match self {
            Error::ActixWebError(e) => e.as_response_error().error_response(),
            _ => actix_web::error::ResponseError::error_response(self),
        }
    }
}

pub struct Multipart<T> {
    mp: actix_multipart::Multipart,
    marker: PhantomData<T>,
}

pub trait FromField: Sized {
    /// The associated error which can be returned.
    type Error: Into<actix_http::error::Error>;
    /// Future that resolves to a Self
    type Future: Future<Output = Result<Self, Self::Error>>;
    /// Mime that should be in field
    const MIME: Either<mime::Mime, mime::Name<'static>>;

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
                marker: Default::default(),
            })
        }
        .boxed_local()
    }
}

#[derive(Deref, DerefMut, Debug)]
pub struct ImageBuffer<P: image::Pixel, Cont>(image::ImageBuffer<P, Cont>);

#[derive(Deref, DerefMut)]
pub struct DynamicImage(pub image::DynamicImage);

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
        let vec = f.splitn(2, "=").collect::<Vec<_>>();
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

impl FromField for Vec<u8> {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    const MIME: Either<mime::Mime, mime::Name<'static>> =
        Either::Left(mime::APPLICATION_OCTET_STREAM);

    fn from_field(mut field: actix_multipart::Field) -> Self::Future {
        async move {
            let mut vec: Vec<u8> = Vec::new();
            while let Some(chunk) = field.next().await {
                vec.extend(chunk.iter().flat_map(|m| m.iter()))
            }
            Ok(vec)
        }
        .boxed_local()
    }
}

impl<T> FromField for File<T>
where
    T: FromField + 'static,
    T::Error: std::fmt::Debug,
{
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    const MIME: Either<mime::Mime, mime::Name<'static>> =
        Either::Right(mime::IMAGE);

    fn from_field(field: actix_multipart::Field) -> Self::Future {
        let mime = field.content_type().clone();
        let mut disp = get_content_disposition(&field);
        let name = disp
            .remove("filename")
            .ok_or(Error::NoFilenameError)
            .map(|s| PathBuf::from(s.to_string())); //
        let inner = T::from_field(field);

        async move {
            let name = name?;
            let inner: T = inner.await.unwrap();

            Ok(Self { name, mime, inner })
        }
        .boxed_local()
    }
}

#[derive(Deref, DerefMut)]
pub struct Json<T>(pub T);

impl<T: serde::de::DeserializeOwned + 'static> FromField for Json<T> {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    const MIME: Either<mime::Mime, mime::Name<'static>> =
        Either::Left(mime::APPLICATION_JSON);

    fn from_field(field: actix_multipart::Field) -> Self::Future {
        let vec = Vec::<u8>::from_field(field);
        async move {
            let vec = vec.await.unwrap();
            let json: T = serde_json::from_reader(&*vec)?;
            Ok(Self(json))
        }
        .boxed_local()
    }
}

impl FromField for String {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    const MIME: Either<mime::Mime, mime::Name<'static>> =
        Either::Right(mime::STAR);

    fn from_field(field: actix_multipart::Field) -> Self::Future {
        async move {
            let vec = Vec::<u8>::from_field(field).await.unwrap();
            Ok(String::from_utf8(vec)?)
        }
        .boxed_local()
    }
}

impl FromField for DynamicImage {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    const MIME: Either<mime::Mime, mime::Name<'static>> =
        Either::Right(mime::IMAGE);

    fn from_field(field: actix_multipart::Field) -> Self::Future {
        async move {
            let ct = field.content_type().clone();
            let vec = Vec::<u8>::from_field(field).await.unwrap();
            let tp = match ct.subtype() {
                mime::JPEG => Some(ImageFormat::Jpeg),
                mime::PNG => Some(ImageFormat::Png),
                mime::GIF => Some(ImageFormat::Gif),
                mime::BMP => Some(ImageFormat::Bmp),
                _ => None,
            };
            let cur = Cursor::new(&*vec);
            let rdr = match tp {
                Some(fmt) => ImgReader::with_format(cur, fmt),
                None => ImgReader::new(cur)
                    .with_guessed_format()
                    .expect("Cursor io never fails"),
            };

            Ok(Self(rdr.decode()?))
        }
        .boxed_local()
    }
}

macro_rules! ff_img(
	{ $ty:ident, $img:ty, $into:ident } => {
		#[derive(Deref, DerefMut, Debug)]
		pub struct $ty(pub $img);

		impl FromField for $ty {
			type Error = Error;
			type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

			const MIME: Either<mime::Mime, mime::Name<'static>> =
				DynamicImage::MIME;

			fn from_field(field: actix_multipart::Field) -> Self::Future {
				async move {
					let img = DynamicImage::from_field(field).await?;
					let img = img.0.$into();
					Ok(Self(img))
				}
				.boxed_local()
			}
		}
	};
);

macro_rules! ff_img_mozjpeg(
	{ $ty:ident, $img:ty, $subp:ty, $into:ident } => {
		#[derive(Deref, DerefMut, Debug)]
		pub struct $ty(pub $img);

		impl FromField for $ty {
			type Error = Error;
			type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

			const MIME: Either<mime::Mime, mime::Name<'static>> =
				Either::Left(mime::IMAGE_JPEG);

			fn from_field(field: actix_multipart::Field) -> Self::Future {
				use mozjpeg::{decompress::DctMethod, Decompress, ALL_MARKERS};

				async move {
					let buf = Vec::<u8>::from_field(field).await.unwrap();
					let mut decomp = Decompress::with_markers(ALL_MARKERS)
						.from_mem(&buf[..])
						.map_err(|_| Error::MozjpgDecodeError)?;
					decomp.dct_method(DctMethod::IntegerFast);

					let (w, h) = decomp.size();
					let mut decomp = decomp.$into()
						.map_err(|_| Error::MozjpgDecodeError)?;
					let out = decomp
						.read_scanlines::<$subp>()
						.ok_or(Error::MozjpgDecodeError)?;

					let out: &[$subp] = &*out;
					let sz = out.len() * std::mem::size_of::<$subp>();
					let out: &[u8] = unsafe {
						let out: *const u8 = std::mem::transmute(out.as_ptr());
						std::slice::from_raw_parts(out, sz)
					};

					Ok(Self(<$img>::from_raw(w as u32, h as u32, out.to_vec())
							.ok_or(Error::MozjpgDecodeError)?))
				}
				.boxed_local()
			}
		}
	};
);

//#[cfg(feature = "mozjpeg")]
ff_img_mozjpeg!(RgbImage, image::RgbImage, [u8; 3], rgb);
//#[cfg(not(feature = "mozjpeg"))]
//ff_img!(RgbImage, image::RgbImage, into_rgb);

//#[cfg(feature = "mozjpeg")]
ff_img_mozjpeg!(RgbaImage, image::RgbaImage, [u8; 4], rgba);
//#[cfg(not(feature = "mozjpeg"))]
//ff_img!(RgbaImage, image::RgbaImage, convert);

//#[cfg(feature = "mozjpeg")]
ff_img_mozjpeg!(GrayImage, image::GrayImage, [u8; 1], grayscale);
//#[cfg(not(feature = "mozjpeg"))]
//ff_img!(GrayImage, image::GrayImage, into_luma);

ff_img!(GrayAlphaImage, image::GrayAlphaImage, into_luma_alpha);
ff_img!(BgrImage, image::ImageBuffer<Bgr<u8>, Vec<u8>>, into_bgr);
ff_img!(BgraImage, image::ImageBuffer<Bgra<u8>, Vec<u8>>, into_bgra);
