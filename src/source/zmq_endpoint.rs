use std::cell::Cell;
use crate::common::message::MetricCollection;
use crate::common::metric::{Metric};
use tokio::sync::mpsc::error::TrySendError;
use zeromq::{Socket, SocketRecv, ZmqError};
use crate::source::EndpointError;

const SUB_NAME_METRICS: &str = "metrics";

pub struct ZmqEndpoint {
    destination: String,
    socket: Cell<zeromq::SubSocket>,
}

impl TryFrom<&String> for ZmqEndpoint {
    type Error = ZmqError;

    fn try_from(dst: &String) -> Result<Self, Self::Error> {
        let socket = std::cell::Cell::new(zeromq::SubSocket::new());

        Ok(ZmqEndpoint {
            destination: dst.clone(),
            socket,
        })
    }
}

pub trait MessageSink {
    fn try_send(&self, msg: MetricCollection) -> Result<(), TrySendError<MetricCollection>>;
}

impl MessageSink for tokio::sync::mpsc::Sender<MetricCollection> {
    fn try_send(&self, msg: MetricCollection) -> Result<(), TrySendError<MetricCollection>> {
        self.try_send(msg)
    }
}

impl ZmqEndpoint {
    pub fn new(dst: &str) -> Result<Self, zeromq::ZmqError> {
        Self::try_from(&format!("tcp://{}", dst))
    }

    pub async fn connect(&mut self) -> Result<(), zeromq::ZmqError> {
        self.socket.get_mut().connect(&self.destination).await?;

        self.socket.get_mut().subscribe(SUB_NAME_METRICS).await?;

        Ok(())
    }

    pub async fn try_reconnect(&mut self) -> Result<(), zeromq::ZmqError> {
        let new_socket = zeromq::SubSocket::new();

        self.socket.replace(new_socket);

        self.socket.get_mut().connect(&self.destination).await
    }

    fn convert_to_metrics(json_obj: json::JsonValue) -> Result<Vec<Metric>, EndpointError> {
        let values_entry = &json_obj["values"];

        let mut local_metrics = Vec::new();

        for value_entry in values_entry.members() {
            if let Ok(metric) = Metric::try_from(value_entry) {
                local_metrics.push(metric);
            }
        }

        Ok(local_metrics)
    }

    pub async fn recv_msg(&mut self) -> Result<MetricCollection, EndpointError> {
        let msg_result = self.socket.get_mut().recv().await;

        if msg_result.is_err() {
            return Err(EndpointError::new("Receive failed"));
        }

        let msg = msg_result.unwrap();

        if msg.is_empty() {
            return Err(EndpointError::new("Empty message received"));
        }

        let pub_name: String = String::from_utf8(msg.get(0).unwrap().to_vec()).unwrap();

        return match msg.get(1) {
            Some(msg_data) => {
                let s = String::from_utf8(msg_data.to_vec());

                let json_obj = json::parse(&s.unwrap());

                match json_obj {
                    Ok(json_obj) => {
                        let timestamp_entry = &json_obj["timestamp"];

                        let timestamp = timestamp_entry.as_u64().unwrap_or(0);

                        let converted_metrics = ZmqEndpoint::convert_to_metrics(json_obj);

                        if converted_metrics.is_err() {
                            return Err(converted_metrics.err().unwrap());
                        }

                        Ok(MetricCollection::new(
                            self.destination.clone(),
                            pub_name,
                            timestamp,
                            converted_metrics.unwrap(),
                        ))
                    }
                    Err(json_err) => {
                        Err(EndpointError::new(&json_err.to_string()))
                    }
                }
            }
            None => {
                Err(EndpointError::new("no message data"))
            }
        };
    }
}
