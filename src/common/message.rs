
#[allow(unused_variables)]
pub struct MetricMessage {
    src: String,
    subscription: String,
    data: Vec<u8>,
}

impl MetricMessage {
    pub fn new(src: String, subscription: String, data: Vec<u8>) -> MetricMessage {
        MetricMessage {
            src,
            subscription,
            data,
        }
    }

    pub fn get_data(&self) -> &Vec<u8> {
        &self.data
    }
}
