use crate::common::metric::{Metric, MetricRawUnit, MetricUnit, MetricValue, OrderOfMagnitude};
use std::collections::hash_map::Iter;
use std::collections::{HashMap, VecDeque};

pub enum AutoMetricRuleType {
    TimeDifferentiate,
    MovingAverage { depth: usize },
    ExpFalloffAverage { alpha: f32 },
}

pub struct AutoMetricRule {
    src_metric_name: String,
    dst_metric_name: String,
    rule_type: AutoMetricRuleType,
}

enum MetricStorage {
    CurrentOnly(Metric),
    History {
        current: Metric,
        history: VecDeque<(u64, MetricValue)>,
    },
}

pub struct MetricAggregator {
    metrics: HashMap<String, MetricStorage>,

    last_timestamp: u64,

    auto_metric_rules: Vec<AutoMetricRule>,

    max_history: usize,

    desired_deltat_diffs_us: u64,
}

pub struct MetricIterator<'a> {
    internal_it: std::collections::hash_map::Iter<'a, String, MetricStorage>,
}

impl<'a> Iterator for MetricIterator<'a> {
    type Item = &'a Metric;

    fn next(&mut self) -> Option<Self::Item> {
        let n = self.internal_it.next();

        if let Some((name, metric_storage)) = n {
            match metric_storage {
                MetricStorage::CurrentOnly(current) => Some(current),
                MetricStorage::History {
                    current,
                    history: _,
                } => Some(current),
            }
        } else {
            None
        }
    }
}

const DEFAULT_MAX_HISTORY: usize = 1024;

const DEFAULT_DELTAT: u64 = 1000000;

fn metric_from_time_diff(
    first: &MetricValue,
    second: &MetricValue,
    src_unit: &MetricUnit,
    dst_name: &str,
    time_diff_us: u64,
) -> Option<Metric> {
    let time_diff_s = time_diff_us as f64 * 1e-6f64;

    let value_diff = f64::from(first) - f64::from(second);

    let order_of_magnitude = match src_unit.get_raw_unit().0 {
        MetricRawUnit::Bytes => OrderOfMagnitude::Kilo,
        MetricRawUnit::Bits => OrderOfMagnitude::Mega,
        _ => OrderOfMagnitude::One,
    };

    let correction_factor = 1.0f64 / order_of_magnitude.get_factor();

    let rate_value = ((correction_factor * value_diff) / time_diff_s) as i64;

    Some(Metric::new(
        dst_name.to_string(),
        MetricUnit::new(
            src_unit.get_raw_unit().0.clone(),
            MetricRawUnit::Seconds,
            order_of_magnitude,
        ),
        MetricValue::Integer(rate_value),
    ))
}

impl MetricAggregator {
    pub fn new() -> MetricAggregator {
        MetricAggregator {
            metrics: HashMap::new(),
            last_timestamp: 0u64,
            auto_metric_rules: Vec::new(),
            max_history: DEFAULT_MAX_HISTORY,
            desired_deltat_diffs_us: DEFAULT_DELTAT,
        }
    }

    pub fn handle_metrics(&mut self, new_timestamp: u64, metrics: &[Metric]) {
        //let last_timestamp = self.last_timestamp;

        self.last_timestamp = new_timestamp;

        for metric in metrics {
            self.handle_incoming_metric(metric);
        }

        self.handle_auto_rules();
    }

    fn handle_incoming_metric(&mut self, metric: &Metric) {
        if !self.metrics.contains_key(metric.get_label()) {
            let metric_unit = metric.get_unit();

            let metric_storage = match metric_unit.get_raw_unit() {
                (MetricRawUnit::Bytes, MetricRawUnit::None)
                | (MetricRawUnit::Packets, MetricRawUnit::None) => {
                    let metric_history =
                        VecDeque::from([(self.last_timestamp, metric.get_value().clone())]);

                    MetricStorage::History {
                        current: metric.clone(),
                        history: metric_history,
                    }
                }
                _ => MetricStorage::CurrentOnly(metric.clone()),
            };

            self.create_auto_rules(&metric_storage);

            self.metrics
                .insert(metric.get_label().to_string(), metric_storage);
        } else {
            let query_result = self.metrics.get_mut(metric.get_label());

            if let Some(metric_storage) = query_result {
                match metric_storage {
                    MetricStorage::History { current, history } => {
                        *current = metric.clone();

                        crate::common::vec_shift(
                            history,
                            (self.last_timestamp, current.get_value().clone()),
                            self.max_history,
                        );
                    }
                    MetricStorage::CurrentOnly(current) => {
                        *current = metric.clone();
                    }
                }
            }
        }
    }

    fn create_auto_rules(&mut self, metric_storage: &MetricStorage) {
        if let MetricStorage::History {
            current,
            history: _,
        } = metric_storage
        {
            self.auto_metric_rules.push(AutoMetricRule {
                src_metric_name: current.get_label().to_string(),
                dst_metric_name: format!("{}-ps", current.get_label()),
                rule_type: AutoMetricRuleType::TimeDifferentiate,
            })
        }
    }

    fn handle_auto_rules(&mut self) {
        for auto_rule_index in 0..self.auto_metric_rules.len() {
            let mut generated_metric = Option::None;

            let auto_rule = self.auto_metric_rules.get(auto_rule_index).unwrap();

            if let Some(metric_storage) = self.metrics.get(&auto_rule.src_metric_name) {
                match auto_rule.rule_type {
                    AutoMetricRuleType::TimeDifferentiate => {
                        if let MetricStorage::History {
                            current: current_metric,
                            history,
                        } = metric_storage
                        {
                            let mut history_iter = history.iter();

                            let first = history_iter.next();

                            if let Some(first) = first {
                                while let Some(current) = history_iter.next() {
                                    let time_diff_us = first.0 - current.0;

                                    if time_diff_us >= self.desired_deltat_diffs_us {
                                        generated_metric = metric_from_time_diff(
                                            &first.1,
                                            &current.1,
                                            current_metric.get_unit(),
                                            &auto_rule.dst_metric_name,
                                            time_diff_us,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    _ => (),
                }
            }

            if let Some(generated_metric) = generated_metric {
                self.handle_incoming_metric(&generated_metric);
            }
        }
    }

    pub fn walk_metrics(&self, cb: impl Fn(&Metric)) {
        for (_, metric_storage) in &self.metrics {
            match metric_storage {
                MetricStorage::History {
                    current,
                    history: _,
                } => {
                    cb(current);
                }
                MetricStorage::CurrentOnly(current) => {
                    cb(current);
                }
            }
        }
    }

    pub fn metric_iter(&self) -> MetricIterator {
        MetricIterator {
            internal_it: self.metrics.iter(),
        }
    }
}
