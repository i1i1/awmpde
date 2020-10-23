use awmpde::FromActixMultipart;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
enum State {
    Ready,
    Set,
    Go,
}

#[derive(FromActixMultipart, Debug)]
struct Help {
    #[serde_json]
    _state: State,
}
