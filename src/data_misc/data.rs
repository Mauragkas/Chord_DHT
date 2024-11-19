use super::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Data {
    pub key: String,
    pub value: String,
}

pub trait NewData {
    fn new(key: &str, value: &str) -> Self;
}

impl NewData for Data {
    fn new(key: &str, value: &str) -> Self {
        Data {
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}
