use crate::common::metric::{Metric};

#[allow(unused_variables)]



pub struct MetricCollection {
    src: String,
    subscription: String,
    timestamp: u64,
    metrics: Vec<Metric>,
}

impl MetricCollection {
    pub fn new(src: String, subscription: String, timestamp: u64, metrics: Vec<Metric>) -> MetricCollection {
        MetricCollection {
            src,
            subscription,
            timestamp,
            metrics,
        }
    }

    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn get_metrics_ref(&self) -> &Vec<Metric> {
        &self.metrics
    }

    pub fn get_metrics(self) -> Vec<Metric> {
        self.metrics
    }
}
