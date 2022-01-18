use crate::aggregator::aggregator::MetricIterator;
use crate::common::metric::Metric;
use crate::source::endpoint::Endpoint;
use crate::{source, MetricAggregator};
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::select;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use zeromq::ZmqError;

pub struct Backend {
    rt: tokio::runtime::Runtime,

    aggregator: Arc<Mutex<MetricAggregator>>,

    task_join_handle: Option<JoinHandle<()>>,

    quit_signal: Option<oneshot::Sender<()>>,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub msg: String,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "msg: {}", self.msg)
    }
}

impl std::error::Error for Error {}


pub trait MetricAdapter {
    // Name of the metric that's encapsulated
    fn get_name(&self) -> &String;

    fn update_current(&mut self, metric : &Metric);
}

impl Backend {
    pub fn new() -> Backend {
        Backend {
            rt: tokio::runtime::Builder::new_multi_thread().enable_io().enable_time().worker_threads(1).build().unwrap()/*Runtime::new().unwrap()*/,
            aggregator: Arc::new(Mutex::new(MetricAggregator::new())),
            task_join_handle: None,
            quit_signal: None,
        }
    }

    pub fn connect<T: ToString>(&mut self, dst: T) -> Result<(), Error> {
        if self.task_join_handle.is_some() {
            return Err(Error {
                msg: String::from("already connected"),
            });
        }

        let endpoint = source::endpoint::Endpoint::new(&dst.to_string());

        if endpoint.is_err() {
            return Err(Error {
                msg: format!("Could not connect: {:?}", endpoint.err().unwrap()),
            });
        } else {
            let mut endpoint = endpoint.unwrap();

            let connect_result: Result<Endpoint, ZmqError> = self.rt.block_on(async move {
                endpoint.connect().await?;

                Ok(endpoint)
            });

            if connect_result.is_ok() {
                let endpoint = connect_result.unwrap();

                let aggregator = Arc::clone(&self.aggregator);

                let (quit_signal, quit_signal_receiver) = oneshot::channel::<()>();

                self.quit_signal = Some(quit_signal);

                self.task_join_handle = Some(self.rt.spawn(async move {
                    Backend::receiver_handler(endpoint, quit_signal_receiver, aggregator).await
                }));
            } else {
                return Err(Error {
                    msg: format!("{:?}", connect_result.err().unwrap()),
                });
            }
        }

        Ok(())
    }

    pub fn visit_metrics(&self, cb: impl Fn(&Metric)) {
        let aggregator = self.aggregator.lock().unwrap();

        aggregator.walk_metrics(cb);
    }

    pub fn map_metrics<T, F>(&self, cb: F) -> Vec<T>
    where
        F: Fn(&Metric) -> T,
    {
        let mut v = Vec::new();

        let aggregator_local = self.aggregator.lock().unwrap();

        for m in aggregator_local.metric_iter() {
            v.push(cb(m));
        }

        v
    }

    pub fn get_metric_history(&self, name : &str, history_data : &mut Vec<(f64, f64)>) -> Option<(f64, f64)> {
        let aggregator_local = self.aggregator.lock().unwrap();

        aggregator_local.get_metric_history(name, history_data)
    }

    pub fn get_last_timestamp(&self) -> u64 {
        let aggregator_local = self.aggregator.lock().unwrap();

        aggregator_local.get_last_timestamp()
    }

    pub fn fetch_updates<T>(&self, mut foreign_it : T)
    where
        T: Iterator,
        T::Item: AsMut<dyn MetricAdapter>
    {
        let aggregator_local = self.aggregator.lock().unwrap();

        while let Some(mut element) = foreign_it.next() {

            if let Some(metric) = aggregator_local.get_metric(element.as_mut().get_name()) {
                element.as_mut().update_current(metric);
            }
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(signal) = self.quit_signal.take() {
            signal.send(()).unwrap();

            if self.task_join_handle.is_some() {
                self.rt.block_on(self.task_join_handle.take().unwrap()).unwrap();
            }
        }
    }

    async fn receiver_handler(
        mut endpoint: Endpoint,
        quit_signal_receiver: oneshot::Receiver<()>,
        aggregator: Arc<Mutex<MetricAggregator>>,
    ) {
        let mut local_metrics = Vec::new();

        let mut quit_signal_receiver = quit_signal_receiver;

        loop {
            select! {
                msg = endpoint.recv_msg() => {
                    if let Ok(msg) = msg {
                        let s = String::from_utf8(msg.get_data().clone());

                        if let Ok(s) = s {

                        let json_obj = json::parse(&s);

                        if let Ok(json_obj) = json_obj {
                            let timestamp_entry = &json_obj["timestamp"];
                            let values_entry = &json_obj["values"];

                            let timestamp = timestamp_entry.as_u64().unwrap_or(0);

                            for value_entry in values_entry.members() {
                                if let Ok(metric) = Metric::try_from(value_entry) {
                                    local_metrics.push(metric);
                                }
                            }

                            let mut aggregator_local = aggregator.lock().unwrap();

                            aggregator_local.handle_metrics(timestamp, local_metrics.as_slice());

                            local_metrics.clear();
                        }
                    }
                    }
                },
                _ = (&mut quit_signal_receiver) => {
                    break;
                }
            }
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.disconnect()
    }
}