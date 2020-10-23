use awmpde::FromActixMultipart;

#[derive(FromActixMultipart)]
struct Help {
    _animals: Vec<String>,
}
