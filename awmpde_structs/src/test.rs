use std::convert::TryInto;

use actix_multipart::Multipart;
use actix_utils::mpsc;
use actix_web::error::PayloadError;
use actix_web::http::header::{self, HeaderMap};
use actix_web::web::Bytes;
use futures::{Stream, StreamExt};

#[derive(Clone)]
pub struct MultipartBuilder(Vec<u8>);

impl MultipartBuilder {
    pub const BOUNDARY: &'static str = "--abbc761f78ff4d7cb7573b5a23f96ef0";
    pub const CONTENT_TYPE: &'static str =
        "multipart/mixed; boundary=\"abbc761f78ff4d7cb7573b5a23f96ef0\"";

    fn write<C: Iterator<Item = u8>>(mut self, cont: C) -> Self {
        self.0.extend(cont);
        self
    }

    fn add_boundary(self) -> Self {
        self.write(Self::BOUNDARY.bytes()).new_line()
    }

    fn new_line(self) -> Self {
        self.write(b"\r\n".iter().copied())
    }

    pub fn new() -> Self {
        Self(Default::default()).add_boundary()
    }

    pub fn add_field<'a, C: IntoIterator<Item = u8>>(
        mut self,
        name: &'static str,
        cont_type: Option<&'static str>,
        content: C,
    ) -> Self {
        self = self
            .write(format!("Content-Disposition: form-data; name=\"{}\"", name).bytes())
            .new_line();

        if let Some(tp) = cont_type {
            self = self
                .write(format!("Content-Type: {}", tp).bytes())
                .new_line();
        }

        self.new_line()
            .write(content.into_iter())
            .new_line()
            .add_boundary()
    }

    pub fn build_payload_bytes(self) -> Bytes {
        let mut bytes: Bytes = self.0.into();
        let bytes = bytes.split_to(bytes.len()); // strip crlf
        bytes
    }

    fn create_stream() -> (
        mpsc::Sender<Result<Bytes, PayloadError>>,
        impl Stream<Item = Result<Bytes, PayloadError>>,
    ) {
        let (tx, rx) = mpsc::channel();

        (tx, rx.map(|res| res.map_err(|_| panic!())))
    }

    pub fn build_payload(self) -> impl Stream<Item = Result<Bytes, PayloadError>> {
        let (sender, payload) = Self::create_stream();
        sender.send(Ok(self.build_payload_bytes())).unwrap();
        payload
    }

    pub fn build(self) -> Multipart {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, Self::CONTENT_TYPE.try_into().unwrap());

        Multipart::new(&headers, self.build_payload())
    }
}
