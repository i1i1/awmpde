use super::{Error, FromField, FutureExt, LocalBoxFuture};

use uuid::Uuid;

impl FromField for Uuid {
    // Should never return error as anything fits
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_field(field: actix_multipart::Field) -> Self::Future {
        async move {
            let s = String::from_field(field).await?;
            Ok(Uuid::parse_str(&s)?)
        }
        .boxed_local()
    }
}
