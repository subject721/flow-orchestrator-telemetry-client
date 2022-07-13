use crate::common::metric::{Metric, MetricRawUnit, MetricUnit, MetricValue, OrderOfMagnitude};
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

struct MetricEntry {
    storage: MetricStorage,
    parent_metric: Option<String>,

}

pub struct MetricAggregator {
    metrics: HashMap<String, MetricEntry>,

    last_timestamp: u64,

    auto_metric_rules: Vec<AutoMetricRule>,

    max_history: usize,

    desired_deltat_diffs_us: u64,

    messages_received: u64,
}

pub struct MetricIterator<'a> {
    internal_it: std::collections::hash_map::Iter<'a, String, MetricEntry>,
}

impl<'a> Iterator for MetricIterator<'a> {
    type Item = &'a Metric;

    fn next(&mut self) -> Option<Self::Item> {
        let n = self.internal_it.next();

        if let Some((_name_, metric_entry)) = n {
            match &metric_entry.storage {
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

impl MetricEntry {
    fn new(storage: MetricStorage, parent_metric: Option<String>) -> MetricEntry {
        MetricEntry {
            storage,
            parent_metric,
        }
    }
}

const DEFAULT_MAX_HISTORY: usize = 128;

const DEFAULT_DELTAT: u64 = 250000;

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

    let scaling = 1.0f64 / order_of_magnitude.get_factor();

    let rate_value = ((scaling * value_diff) / time_diff_s) as i64;

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

fn metric_from_avg(value: f64, src_unit: &MetricUnit, dst_name: &str) -> Option<Metric> {
    Some(Metric::new(dst_name.to_string(), src_unit.clone(), MetricValue::Number(value)))
}

impl MetricAggregator {
    pub fn new() -> MetricAggregator {
        MetricAggregator {
            metrics: HashMap::new(),
            last_timestamp: 0u64,
            auto_metric_rules: Vec::new(),
            max_history: DEFAULT_MAX_HISTORY,
            desired_deltat_diffs_us: DEFAULT_DELTAT,
            messages_received: 0,
        }
    }

    pub fn handle_metrics(&mut self, new_timestamp: u64, metrics: &[Metric]) {
        //let last_timestamp = self.last_timestamp;

        self.last_timestamp = new_timestamp;


        self.messages_received += 1;

        for metric in metrics {
            self.handle_incoming_metric(metric, &None);
        }

        self.handle_auto_rules();
    }

    fn handle_incoming_metric(&mut self, metric: &Metric, parent_metric: &Option<String>) {
        if let Some(metric_entry) = self.metrics.get_mut(metric.get_label()) {
            let metric_storage = &mut metric_entry.storage;

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
        } else {
            let metric_unit = metric.get_unit();

            let metric_storage = match metric_unit.get_raw_unit().0 {
                MetricRawUnit::Bytes
                | MetricRawUnit::Bits
                | MetricRawUnit::Packets
                | MetricRawUnit::None => {
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
                .insert(metric.get_label().to_string(), MetricEntry::new(metric_storage, parent_metric.clone()));
        }
    }

    fn create_auto_rules(&mut self, metric_storage: &MetricStorage) {
        if let MetricStorage::History {
            current,
            history: _,
        } = metric_storage
        {
            if std::mem::discriminant(current.get_value()) == std::mem::discriminant(&MetricValue::Number(0f64)) ||
                std::mem::discriminant(current.get_value()) == std::mem::discriminant(&MetricValue::Integer(0)) {
                if current.get_unit().get_raw_unit().1 != &MetricRawUnit::Seconds {
                    if current.get_unit().get_raw_unit().0 != &MetricRawUnit::None && current.get_unit().get_raw_unit().0 != &MetricRawUnit::Seconds {
                        self.auto_metric_rules.push(AutoMetricRule {
                            src_metric_name: current.get_label().to_string(),
                            dst_metric_name: format!("{}-ps", current.get_label()),
                            rule_type: AutoMetricRuleType::TimeDifferentiate,
                        });
                    }
                } else if !current.get_label().ends_with(&"-avg") {
                    self.auto_metric_rules.push(AutoMetricRule {
                        src_metric_name: current.get_label().to_string(),
                        dst_metric_name: format!("{}-avg", current.get_label()),
                        rule_type: AutoMetricRuleType::MovingAverage { depth: 32 },
                    });
                }
            }
        }
    }

    fn handle_auto_rules(&mut self) {
        for auto_rule_index in 0..self.auto_metric_rules.len() {
            let mut generated_metric = Option::None;
            let mut parent_metric = Option::None;

            let auto_rule = self.auto_metric_rules.get(auto_rule_index).unwrap();

            if let Some(metric_entry) = self.metrics.get(&auto_rule.src_metric_name) {
                parent_metric = Some(auto_rule.src_metric_name.clone());

                match auto_rule.rule_type {
                    AutoMetricRuleType::TimeDifferentiate => {
                        if let MetricStorage::History {
                            current: current_metric,
                            history,
                        } = &metric_entry.storage
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
                    AutoMetricRuleType::MovingAverage { depth } => {
                        if let MetricStorage::History {
                            current: current_metric,
                            history,
                        } = &metric_entry.storage
                        {
                            let mut history_iter = history.iter().take(depth);

                            let mut count = 0;
                            let mut sum = 0.0f64;

                            while let Some(history_element) = history_iter.next() {
                                sum += f64::from(&history_element.1);
                                count += 1;
                            }

                            if count > 0 {
                                let avg = sum / count as f64;
                                generated_metric = metric_from_avg(avg, current_metric.get_unit(), &auto_rule.dst_metric_name);
                            }
                        }
                    }
                    _ => (),
                }
            }

            if let Some(generated_metric) = generated_metric {
                self.handle_incoming_metric(&generated_metric, &parent_metric);
            }
        }
    }

    pub fn walk_metrics(&self, cb: impl Fn(&Metric)) {
        for (_, metric_entry) in &self.metrics {
            match &metric_entry.storage {
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

    pub fn get_metric(&self, name: &str) -> Option<&Metric> {
        if let Some(metric_entry) = self.metrics.get(name) {
            match &metric_entry.storage {
                MetricStorage::History {
                    current,
                    history: _,
                } => return Some(current),
                MetricStorage::CurrentOnly(current) => return Some(current),
            }
        }

        None
    }

    pub fn get_last_timestamp(&self) -> u64 {
        self.last_timestamp
    }

    pub fn get_metric_history(
        &self,
        name: &str,
        data: &mut Vec<(f64, f64)>,
        max_len: usize,
    ) -> Option<(f64, f64)> {
        if let Some(metric_entry) = self.metrics.get(name) {
            if let MetricStorage::History {
                current,
                history,
            } = &metric_entry.storage {
                if std::mem::discriminant(current.get_value()) == std::mem::discriminant(&MetricValue::Number(0f64)) ||
                    std::mem::discriminant(current.get_value()) == std::mem::discriminant(&MetricValue::Integer(0)) {
                    let requested_len = max_len.min(history.len());

                    if data.len() != requested_len {
                        data.resize(requested_len, (0.0f64, 0.0f64));
                    }

                    let mut max_val = None;
                    let mut min_val = None;

                    for idx in 0..history.len() {
                        let current = &history[idx];

                        let current_metric_val = f64::from(&current.1);

                        if idx < requested_len {
                            data[requested_len - idx - 1] =
                                (current.0 as f64 / 1e6f64, current_metric_val);
                        }

                        max_val = Some(current_metric_val.max(max_val.unwrap_or(0.0f64)));

                        min_val =
                            Some(current_metric_val.min(min_val.unwrap_or(current_metric_val)));
                    }

                    if let (Some(max_val), Some(min_val)) = (max_val, min_val) {
                        return Some((min_val, max_val));
                    }
                }
            }
        }

        None
    }
}
