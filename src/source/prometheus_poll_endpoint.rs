use std::error::Error;
use std::str::FromStr;
use std::time;
use std::time::Duration;
use reqwest::Response;
use crate::common::message::MetricCollection;
use crate::common::metric::{Metric, MetricRawUnit, MetricUnit, MetricValue, OrderOfMagnitude};
use crate::source::{EndpointError, MetricEndpoint};



pub struct PrometheusPollEndpoint {
    dst: String,
    client: reqwest::Client
}

impl MetricEndpoint for PrometheusPollEndpoint {

}

impl TryFrom<&str> for PrometheusPollEndpoint {
    type Error = EndpointError;

    fn try_from(dst: &str) -> Result<Self, Self::Error> {
        Ok(PrometheusPollEndpoint {
            dst: dst.to_string(),
            client: reqwest::Client::new()
        })
    }
}

impl PrometheusPollEndpoint {
    pub fn new(dst: &str) -> Result<Self, EndpointError> {
        PrometheusPollEndpoint::try_from(dst)
    }

    pub async fn recv_msg(&mut self) -> Result<MetricCollection, EndpointError> {
        let req = self.client.get(&self.dst).build();

        if req.is_err() {
            return Err(EndpointError::new(&req.unwrap_err().to_string()))
        }

        let response = self.client.execute(req.unwrap()).await;

        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    return Err(EndpointError::new(&format!("Request to {} failed", resp.url())));
                }

                let body_data = resp.text().await;

                std::thread::sleep(Duration::from_millis(250));

                self.build_metric_msg(body_data.unwrap_or(String::new()))
            },
            Err(e) => {
                Err(EndpointError::new(&e.to_string()))
            }
        }
    }

    fn get_metric_unit(line_data: &str) -> MetricUnit {
        let line_data_lc = line_data.to_ascii_lowercase();

        let mut unit_num = MetricRawUnit::None;

        if line_data_lc.contains("packets") {
            unit_num = MetricRawUnit::Packets;
        } else if line_data_lc.contains("bytes") {
            unit_num = MetricRawUnit::Bytes;
        } else if line_data_lc.contains("bits") {
            unit_num = MetricRawUnit::Bits;
        }

        MetricUnit::new(unit_num, MetricRawUnit::None, OrderOfMagnitude::One)
    }

    fn build_metric(line_data: &str, unit: MetricUnit) -> Option<Metric> {
        let mut name : Option<&str> = None;
        let mut value_str : Option<&str> = None;

        for (idx, element) in line_data.split_whitespace().enumerate() {
            if idx == 0 {
                name = Some(element);
            } else if idx == 1 {
                value_str = Some(element);
                break;
            }
        }

        if name.is_none() || value_str.is_none() {
            return None
        }

        if let Ok(v) = i64::from_str(value_str.unwrap()) {
            return Some(Metric::new(name.unwrap().to_string(), unit, MetricValue::Integer(v)))
        } else if let Ok(v) = f64::from_str(value_str.unwrap()) {
            return Some(Metric::new(name.unwrap().to_string(), unit, MetricValue::Number(v)))
        } else {
            return Some(Metric::new(name.unwrap().to_string(), unit, MetricValue::String(value_str.unwrap().to_string())))
        }
    }



    fn build_metric_msg(&self, body_data: String) -> Result<MetricCollection, EndpointError> {

        let mut lines = body_data.lines();

        let mut metric_unit =  None;

        let mut metrics = Vec::new();
        let timestamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_micros() as u64;

        //println!("Beginning parsing of prometheus data at : {}", timestamp);

        while let Some(line) = lines.next() {
            //println!("Processing line: {}", line);

            if line.starts_with("#TYPE") {
                metric_unit = Some(Self::get_metric_unit(line));
            } else if line.starts_with("#HELP") {
                continue;
            } else if !line.starts_with("#") {
                let metric = Self::build_metric(line, metric_unit.take().unwrap_or_else(||{
                    MetricUnit::new(MetricRawUnit::None, MetricRawUnit::None, OrderOfMagnitude::One)
                }));

                if let Some(metric) = metric {
                    //println!("Parsed metric: {:?}", metric);

                    metrics.push(metric);
                }
            }
        }

        Ok(MetricCollection::new(self.dst.clone(), String::new(), timestamp, metrics))
    }
}