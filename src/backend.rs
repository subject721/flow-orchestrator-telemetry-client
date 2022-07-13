
use crate::common::metric::Metric;
use crate::source::zmq_endpoint::ZmqEndpoint;
use crate::{source, MetricAggregator};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{select, task, time};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use zeromq::ZmqError;
use crate::source::prometheus_poll_endpoint::PrometheusPollEndpoint;

//type CbType = dyn Fn() + Send + 'static;

pub type MetricCallback = dyn Fn() + Send + 'static;


pub struct Backend {

    aggregator: Arc<Mutex<MetricAggregator>>,

    task_join_handle: Option<JoinHandle<()>>,

    quit_signal: Option<oneshot::Sender<()>>,

    callbacks: Arc<Mutex<Vec<Box<MetricCallback>>>>,
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

    fn update_current(&mut self, metric: &Metric);
}

impl Backend {
    pub fn new() -> Backend {
        Backend {
            aggregator: Arc::new(Mutex::new(MetricAggregator::new())),
            task_join_handle: None,
            quit_signal: None,
            callbacks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn connect<T: ToString>(&mut self, dst: T) -> Result<(), Error> {
        if self.task_join_handle.is_some() {
            return Err(Error {
                msg: String::from("already connected"),
            });
        }

        //let endpoint = source::zmq_endpoint::ZmqEndpoint::new(&dst.to_string());
        let endpoint = source::prometheus_poll_endpoint::PrometheusPollEndpoint::new(&dst.to_string());

        if endpoint.is_err() {
            return Err(Error {
                msg: format!("Could not connect: {:?}", endpoint.err().unwrap()),
            });
        } else {
            let mut endpoint = endpoint.unwrap();

            //let connect_result: Result<(), ZmqError> = endpoint.connect().await;

            //if connect_result.is_ok() {

                let aggregator = Arc::clone(&self.aggregator);

                let callbacks = Arc::clone(&self.callbacks);

                let (quit_signal, quit_signal_receiver) = oneshot::channel::<()>();

                self.quit_signal = Some(quit_signal);

                self.task_join_handle = Some(task::spawn(async move {
                    Self::receiver_handler(endpoint, quit_signal_receiver, aggregator, callbacks)
                        .await
                }));
            //} else {
            //    return Err(Error {
            //        msg: format!("{:?}", connect_result.err().unwrap()),
            //    });
            //}
        }

        Ok(())
    }

    pub fn add_callback<T : Fn() + Send + 'static>(&self, cb : T) {
        let mut callbacks_local = self.callbacks.lock().unwrap();

        callbacks_local.push(Box::new(cb));
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

    pub fn get_metric_history(
        &self,
        name: &str,
        history_data: &mut Vec<(f64, f64)>,
        max_len: usize,
    ) -> Option<(f64, f64)> {
        let aggregator_local = self.aggregator.lock().unwrap();

        aggregator_local.get_metric_history(name, history_data, max_len)
    }

    pub fn get_last_timestamp(&self) -> u64 {
        let aggregator_local = self.aggregator.lock().unwrap();

        aggregator_local.get_last_timestamp()
    }

    pub fn fetch_updates<T>(&self, mut foreign_it: T)
    where
        T: Iterator,
        T::Item: AsMut<dyn MetricAdapter>,
    {
        let aggregator_local = self.aggregator.lock().unwrap();

        while let Some(mut element) = foreign_it.next() {
            if let Some(metric) = aggregator_local.get_metric(element.as_mut().get_name()) {
                element.as_mut().update_current(metric);
            }
        }
    }

    pub async fn disconnect(&mut self) {
        if let Some(signal) = self.quit_signal.take() {
            signal.send(()).unwrap();

            if self.task_join_handle.is_some() {

                let jh = self.task_join_handle.take().unwrap();

                let result = jh.await;

                if result.is_err() {
                    // wahtever
                }
            }
        }
    }

    async fn receiver_handler(
        mut endpoint: PrometheusPollEndpoint,
        quit_signal_receiver: oneshot::Receiver<()>,
        aggregator: Arc<Mutex<MetricAggregator>>,
        callbacks: Arc<Mutex<Vec<Box<MetricCallback>>>>,
    ) {

        let mut quit_signal_receiver = quit_signal_receiver;

        let recv_timeout = Duration::from_secs(1);

        let sleep = time::sleep(recv_timeout.clone());
        tokio::pin!(sleep);

        loop {
            let mut do_reconnect = false;

            select! {
                msg = endpoint.recv_msg() => {
                    if let Ok(msg) = &msg {
                        sleep.as_mut().reset(Instant::now() + recv_timeout.clone());

                        //let s = String::from_utf8(msg.get_data().clone());


                        //let json_obj = json::parse(&s);

                        //if let Ok(json_obj) = json_obj {

                        let mut aggregator_local = aggregator.lock().unwrap();

                        aggregator_local.handle_metrics(msg.get_timestamp(), msg.get_metrics_ref().as_slice());

                        let callbacks_local = callbacks.lock().unwrap();

                        for cb in callbacks_local.deref() {
                            cb();
                        }

                        //}

                    } else if let Err(err) = &msg {
                        println!("Endpoint error: {:?}", err);

                        do_reconnect = true;
                    }
                },
                _ = (&mut quit_signal_receiver) => {
                    println!("Quit signal received");
                    break;
                },
                _ = (&mut sleep) => {
                    println!("Timeout elapsed");

                    do_reconnect = true;

                    sleep.as_mut().reset(Instant::now() + recv_timeout.clone());
                }
            }
/*
            if do_reconnect {
                let result = endpoint.try_reconnect().await;

                if let Err(err) = result {
                    println!("Could not reconnect: {:?}", err);

                    break;
                }
            }*/
        }
    }
}
