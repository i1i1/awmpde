use super::*;
use std::convert::Infallible;

// Returns raw bytes of multipart payload
impl FromField for Vec<u8> {
    // Should never return error as anything fits
    type Error = Infallible;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

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

/// Type for wrapping any other field in order to get its name and mime.
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

impl<T> FromField for File<T>
where
    T: FromField + 'static,
    T::Error: std::fmt::Debug,
{
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

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

/// Type for wrapping json decoding of multipart field
#[derive(Deref, DerefMut)]
pub struct Json<T>(pub T);

impl<T: serde::de::DeserializeOwned + 'static> FromField for Json<T> {
    type Error = serde_json::error::Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

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

impl<T: FromField + 'static> FromField for Box<T> {
    type Error = T::Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_field(field: actix_multipart::Field) -> Self::Future {
        async move { T::from_field(field).await.map(Box::new) }.boxed_local()
    }
}

impl FromField for String {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_field(field: actix_multipart::Field) -> Self::Future {
        async move {
            let vec = Vec::<u8>::from_field(field).await.unwrap();
            Ok(String::from_utf8(vec)?)
        }
        .boxed_local()
    }
}
