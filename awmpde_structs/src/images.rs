use super::*;

use image::io::Reader as ImgReader;
use image::{Bgr, Bgra, ImageFormat};

#[derive(Deref, DerefMut, Debug)]
pub struct ImageBuffer<P: image::Pixel, Cont>(
    pub Box<image::ImageBuffer<P, Cont>>,
);

#[derive(Deref, DerefMut)]
pub struct DynamicImage(pub Box<image::DynamicImage>);

impl FromField for DynamicImage {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

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

            Ok(Self(Box::new(rdr.decode()?)))
        }
        .boxed_local()
    }
}

macro_rules! ff_img(
	{ $ty:ident, $img:ty, $into:ident } => {
		#[derive(Deref, DerefMut, Debug)]
		pub struct $ty(pub Box<$img>);

		impl FromField for $ty {
			type Error = Error;
			type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

			fn from_field(field: actix_multipart::Field) -> Self::Future {
				async move {
					let img = DynamicImage::from_field(field).await?;
					let img = img.0.$into();
					Ok(Self(Box::new(img)))
				}
				.boxed_local()
			}
		}
	};
);

#[cfg(feature = "mozjpeg")]
macro_rules! ff_img_mozjpeg(
	{ $ty:ident, $img:ty, $subp:ty, $into:ident, $into_img:ident } => {
		#[derive(Deref, DerefMut, Debug, Clone)]
		pub struct $ty(pub Box<$img>);

		impl FromField for $ty {
			type Error = Error;
			type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

			fn from_field(field: actix_multipart::Field) -> Self::Future {
				use mozjpeg::{decompress::DctMethod, Decompress, ALL_MARKERS};

				if *field.content_type() == mime::IMAGE_JPEG {
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
							std::slice::from_raw_parts(
                                out.as_ptr() as *const u8,
                                sz,
                            )
						};

						Ok(Self(Box::new(<$img>::from_raw(w as u32, h as u32, out.to_vec())
										 .ok_or(Error::MozjpgDecodeError)?)))
					}
					.boxed_local()
				} else {
					async move {
						let img = DynamicImage::from_field(field).await?;
						let img = img.0.$into_img();
						Ok(Self(Box::new(img)))
					}
					.boxed_local()
				}
			}
		}
	};
);

#[cfg(feature = "mozjpeg")]
ff_img_mozjpeg!(RgbImage, image::RgbImage, [u8; 3], rgb, into_rgb8);
#[cfg(not(feature = "mozjpeg"))]
ff_img!(RgbImage, image::RgbImage, into_rgb8);

#[cfg(feature = "mozjpeg")]
ff_img_mozjpeg!(RgbaImage, image::RgbaImage, [u8; 4], rgba, into_rgba8);
#[cfg(not(feature = "mozjpeg"))]
ff_img!(RgbaImage, image::RgbaImage, into_rgba8);

#[cfg(feature = "mozjpeg")]
ff_img_mozjpeg!(GrayImage, image::GrayImage, [u8; 1], grayscale, into_luma8);
#[cfg(not(feature = "mozjpeg"))]
ff_img!(GrayImage, image::GrayImage, into_luma8);

ff_img!(GrayAlphaImage, image::GrayAlphaImage, into_luma_alpha8);
ff_img!(BgrImage, image::ImageBuffer<Bgr<u8>, Vec<u8>>, into_bgr8);
ff_img!(BgraImage, image::ImageBuffer<Bgra<u8>, Vec<u8>>, into_bgra8);
