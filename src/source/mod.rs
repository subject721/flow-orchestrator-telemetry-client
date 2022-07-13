use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub mod zmq_endpoint;
pub mod prometheus_poll_endpoint;

#[derive(Debug, Clone)]
pub struct EndpointError {
    pub msg: String
}

pub trait MetricEndpoint {

}


impl Display for EndpointError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "msg: {}", self.msg)
    }
}

impl Error for EndpointError {

}

impl EndpointError {
    pub fn new(msg: &str) -> EndpointError {
        EndpointError {msg: msg.to_string()}
    }
}
