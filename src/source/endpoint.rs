use std::io::Error;
use crate::common::message::MetricMessage;
use std::net::IpAddr;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task;
use zeromq::{Socket, SocketRecv, ZmqError};

const SUB_NAME_METRICS: &str = "metrics";

pub struct Endpoint {
    destination: String,
    socket: zeromq::SubSocket,
}

impl TryFrom<&String> for Endpoint {
    type Error = ZmqError;

    fn try_from(dst: &String) -> Result<Self, Self::Error> {
        let socket = zeromq::SubSocket::new();

        Ok(Endpoint {
            destination: dst.clone(),
            socket,
        })
    }
}

pub trait MessageSink {
    fn try_send(&self, msg: MetricMessage) -> Result<(), TrySendError<MetricMessage>>;
}

impl MessageSink for tokio::sync::mpsc::Sender<MetricMessage> {
    fn try_send(&self, msg: MetricMessage) -> Result<(), TrySendError<MetricMessage>> {
        self.try_send(msg)
    }
}

impl Endpoint {
    pub fn new(dst: &str) -> Result<Self, zeromq::ZmqError> {
        Self::try_from(&format!("tcp://{}", dst))
    }

    pub async fn connect(&mut self) -> Result<(), zeromq::ZmqError> {
        self.socket.connect(&self.destination).await?;

        self.socket.subscribe(SUB_NAME_METRICS).await?;

        Ok(())
    }

    pub async fn recv_msg(&mut self) -> Result<MetricMessage, ZmqError> {
        let msg = self.socket.recv().await?;

        let pub_name: String = String::from_utf8(msg.get(0).unwrap().to_vec()).unwrap();

        return match msg.get(1) {
            Some(msg_data) => {
                Ok(MetricMessage::new(
                self.destination.clone(),
                pub_name,
                msg_data.to_vec(),
                ))
            },
            None => {
                Err(ZmqError::Other(&"no message data"))
            }
        }
    }

}
