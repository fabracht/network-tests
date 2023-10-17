use std::net::SocketAddr;

use network_commons::TestResult;
use serde::{Deserialize, Serialize};

use super::NETWORK_PRECISION;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct NetworkStatistics {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub avg_rtt: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub min_rtt: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub max_rtt: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub std_dev_rtt: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub median_rtt: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub low_percentile_rtt: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub high_percentile_rtt: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub avg_forward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub min_forward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub max_forward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub std_dev_forward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub median_forward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub low_percentile_forward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub high_percentile_forward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub avg_backward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub min_backward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub max_backward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub std_dev_backward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub median_backward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub low_percentile_backward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub high_percentile_backward_owd: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub avg_process_time: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub min_process_time: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub max_process_time: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub std_dev_process_time: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub median_process_time: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub low_percentile_process_time: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub high_percentile_process_time: Option<f64>,
    pub forward_loss: u32,
    pub backward_loss: u32,
    pub total_loss: u32,
    pub total_packets: usize,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "round_option_f64_with_precision"
    )]
    pub gamlr_offset: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionResult {
    pub address: SocketAddr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_statistics: Option<NetworkStatistics>,
}

fn round_f64_with_precision<S>(num: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let precision = NETWORK_PRECISION; // Change this value to set the number of digits after the decimal point
    let factor = 10f64.powi(precision);
    let rounded = (num * factor).round() / factor;
    serializer.serialize_f64(rounded)
}

fn round_option_f64_with_precision<S>(num: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let num = num.unwrap_or_default();
    round_f64_with_precision(&num, serializer)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TwampResult {
    pub session_results: Vec<SessionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TestResult for TwampResult {}
