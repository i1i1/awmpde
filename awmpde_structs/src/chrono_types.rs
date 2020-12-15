use super::*;

use chrono::offset::{FixedOffset, Local, Utc};
use chrono::{DateTime, NaiveDate, NaiveDateTime};

macro_rules! from_field(
    { $ty:ty } => {
        impl FromField for $ty {
            type Error = serde_json::error::Error;
            type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

            fn from_field(field: actix_multipart::Field) -> Self::Future {
                let vec = Vec::<u8>::from_field(field);
                async move {
                    let vec = vec.await.unwrap();
                    Ok(serde_json::from_reader(&*vec)?)
                }
                .boxed_local()
            }
        }
    }
);

from_field!(NaiveDate);
from_field!(NaiveDateTime);
from_field!(DateTime<FixedOffset>);
from_field!(DateTime<Local>);
from_field!(DateTime<Utc>);
