//! Advanced Progress Bars, Metrics Collection, and User Experience
//! 
//! This module provides sophisticated progress visualization, comprehensive metrics
//! collection, and enhanced user experience features for the P2P file converter.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress, ProgressDrawTarget};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
    thread,
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncWriteExt, BufWriter},
    sync::broadcast,
    task::JoinHandle,
    time::interval,
};
use tracing::{debug, info, warn, instrument};

/// Advanced progress bar manager for concurrent operations
pub struct AdvancedProgressManager {
    multi_progress: Arc<MultiProgress>,
    active_bars: Arc<RwLock<HashMap<String, ProgressBarInfo>>>,
    config: ProgressConfig,
}

#[derive(Debug, Clone)]
pub struct ProgressConfig {
    /// Enable colored output
    pub enable_colors: bool,

    /// Update interval for progress bars
    pub update_interval: Duration,

    /// Enable detailed progress information
    pub show_detailed_info: bool,

    /// Enable ETA calculation
    pub show_eta: bool,

    /// Enable speed display
    pub show_speed: bool,

    /// Template for progress bar display
    pub progress_template: String,

    /// Maximum number of concurrent progress bars
    pub max_concurrent_bars: usize,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            enable_colors: true,
            update_interval: Duration::from_millis(100),
            show_detailed_info: true,
            show_eta: true,
            show_speed: true,
            progress_template: "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} ({eta})".to_string(),
            max_concurrent_bars: 10,
        }
    }
}

#[derive(Debug, Clone)]
struct ProgressBarInfo {
    progress_bar: ProgressBar,
    operation_type: String,
    start_time: Instant,
    last_update: Instant,
    last_position: u64,
}

impl AdvancedProgressManager {
    /// Create new progress manager
    pub fn new(config: ProgressConfig) -> Self {
        let multi_progress = MultiProgress::new();

        // Set draw target based on configuration
        let draw_target = if config.enable_colors {
            ProgressDrawTarget::stderr()
        } else {
            ProgressDrawTarget::stderr_nocolors()
        };

        multi_progress.set_draw_target(draw_target);

        Self {
            multi_progress: Arc::new(multi_progress),
            active_bars: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create a new progress bar for an operation
    #[instrument(skip(self), fields(operation_id = %operation_id))]
    pub fn create_progress_bar(
        &self,
        operation_id: &str,
        operation_type: &str,
        total_size: u64,
        message: &str,
    ) -> Result<ProgressBarHandle> {
        let mut active_bars = self.active_bars.write().unwrap();

        // Check concurrent limit
        if active_bars.len() >= self.config.max_concurrent_bars {
            return Err(anyhow::anyhow!(
                "Maximum concurrent progress bars exceeded: {}",
                self.config.max_concurrent_bars
            ));
        }

        let progress_bar = self.multi_progress.add(ProgressBar::new(total_size));

        // Set progress bar style
        let style = if self.config.enable_colors {
            ProgressStyle::default_bar()
                .template(&self.config.progress_template)?
                .progress_chars("█▉▊▋▌▍▎▏  ")
        } else {
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40} {pos:>7}/{len:7} {msg} ({eta})")?
                .progress_chars("##-")
        };

        progress_bar.set_style(style);
        progress_bar.set_message(message.to_string());

        let bar_info = ProgressBarInfo {
            progress_bar: progress_bar.clone(),
            operation_type: operation_type.to_string(),
            start_time: Instant::now(),
            last_update: Instant::now(),
            last_position: 0,
        };

        active_bars.insert(operation_id.to_string(), bar_info);

        debug!(
            "Created progress bar for operation {} (type: {}, total: {} bytes)",
            operation_id, operation_type, total_size
        );

        Ok(ProgressBarHandle {
            operation_id: operation_id.to_string(),
            progress_bar,
            manager: Arc::downgrade(&self.multi_progress),
            active_bars: Arc::downgrade(&self.active_bars),
            config: self.config.clone(),
        })
    }

    /// Get progress statistics for all operations
    pub fn get_progress_summary(&self) -> ProgressSummary {
        let active_bars = self.active_bars.read().unwrap();

        let mut by_type = HashMap::new();
        let mut total_operations = 0;
        let mut completed_operations = 0;

        for (_, bar_info) in active_bars.iter() {
            total_operations += 1;

            let counter = by_type.entry(bar_info.operation_type.clone()).or_insert(0);
            *counter += 1;

            if bar_info.progress_bar.is_finished() {
                completed_operations += 1;
            }
        }

        ProgressSummary {
            total_operations,
            completed_operations,
            active_operations: total_operations - completed_operations,
            operations_by_type: by_type,
            start_time: Instant::now(), // Would track actual start time in real implementation
        }
    }
}

/// Handle for controlling individual progress bars
pub struct ProgressBarHandle {
    operation_id: String,
    progress_bar: ProgressBar,
    manager: std::sync::Weak<MultiProgress>,
    active_bars: std::sync::Weak<RwLock<HashMap<String, ProgressBarInfo>>>,
    config: ProgressConfig,
}

impl ProgressBarHandle {
    /// Update progress with current position
    pub fn update_progress(&self, position: u64, message: Option<&str>) {
        self.progress_bar.set_position(position);

        if let Some(msg) = message {
            self.progress_bar.set_message(msg.to_string());
        }

        // Update speed calculation
        if let Some(active_bars) = self.active_bars.upgrade() {
            if let Ok(mut bars) = active_bars.write() {
                if let Some(bar_info) = bars.get_mut(&self.operation_id) {
                    let now = Instant::now();
                    let time_diff = now.duration_since(bar_info.last_update);

                    if time_diff >= self.config.update_interval {
                        let position_diff = position.saturating_sub(bar_info.last_position);
                        let speed = position_diff as f64 / time_diff.as_secs_f64();

                        if self.config.show_speed && speed > 0.0 {
                            let speed_str = format_speed(speed);
                            self.progress_bar.set_message(format!("{} ({})", 
                                message.unwrap_or(""), speed_str));
                        }

                        bar_info.last_update = now;
                        bar_info.last_position = position;
                    }
                }
            }
        }
    }

    /// Mark progress as completed
    pub fn finish(&self, message: Option<&str>) {
        if let Some(msg) = message {
            self.progress_bar.finish_with_message(msg.to_string());
        } else {
            self.progress_bar.finish();
        }

        debug!("Progress bar completed for operation {}", self.operation_id);
    }

    /// Mark progress as failed
    pub fn finish_with_error(&self, error_message: &str) {
        let styled_message = if self.config.enable_colors {
            format!("❌ {}", error_message)
        } else {
            format!("ERROR: {}", error_message)
        };

        self.progress_bar.abandon_with_message(styled_message);
        warn!("Progress bar failed for operation {}: {}", self.operation_id, error_message);
    }

    /// Get current progress percentage
    pub fn get_percentage(&self) -> f64 {
        let position = self.progress_bar.position();
        let length = self.progress_bar.length();

        if length > 0 {
            (position as f64 / length as f64) * 100.0
        } else {
            0.0
        }
    }
}

impl Drop for ProgressBarHandle {
    fn drop(&mut self) {
        // Remove from active bars when dropped
        if let Some(active_bars) = self.active_bars.upgrade() {
            if let Ok(mut bars) = active_bars.write() {
                bars.remove(&self.operation_id);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressSummary {
    pub total_operations: usize,
    pub completed_operations: usize,
    pub active_operations: usize,
    pub operations_by_type: HashMap<String, usize>,
    pub start_time: Instant,
}

/// Comprehensive metrics collection system
pub struct MetricsCollector {
    config: MetricsConfig,
    transfer_metrics: Arc<RwLock<TransferMetrics>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    error_metrics: Arc<RwLock<ErrorMetrics>>,
    collection_task: Option<JoinHandle<()>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// Collection interval
    pub collection_interval: Duration,

    /// Metrics file path
    pub metrics_file: std::path::PathBuf,

    /// Enable real-time metrics export
    pub enable_realtime_export: bool,

    /// Maximum metrics history to keep
    pub max_history_size: usize,

    /// Enable system resource monitoring
    pub monitor_system_resources: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransferMetrics {
    pub total_transfers: u64,
    pub successful_transfers: u64,
    pub failed_transfers: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub average_transfer_speed: f64,
    pub peak_transfer_speed: f64,
    pub total_transfer_time: Duration,
    pub concurrent_transfers_peak: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub cpu_usage_samples: VecDeque<f64>,
    pub memory_usage_samples: VecDeque<u64>,
    pub disk_io_samples: VecDeque<DiskIoSample>,
    pub network_io_samples: VecDeque<NetworkIoSample>,
    pub operation_latencies: HashMap<String, VecDeque<Duration>>,
    pub throughput_samples: VecDeque<ThroughputSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIoSample {
    pub timestamp: DateTime<Utc>,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_ops: u64,
    pub write_ops: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIoSample {
    pub timestamp: DateTime<Utc>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputSample {
    pub timestamp: DateTime<Utc>,
    pub operation_type: String,
    pub throughput_bps: f64,
    pub concurrent_operations: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub error_counts_by_type: HashMap<String, u64>,
    pub error_counts_by_operation: HashMap<String, u64>,
    pub recovery_success_rate: f64,
    pub mean_time_to_recovery: Duration,
    pub error_rate_by_time: VecDeque<ErrorRateSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRateSample {
    pub timestamp: DateTime<Utc>,
    pub error_count: u64,
    pub total_operations: u64,
    pub error_rate: f64,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(config: MetricsConfig) -> Self {
        Self {
            config,
            transfer_metrics: Arc::new(RwLock::new(TransferMetrics::default())),
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            error_metrics: Arc::new(RwLock::new(ErrorMetrics::default())),
            collection_task: None,
            shutdown_tx: None,
        }
    }

    /// Start metrics collection
    pub async fn start(&mut self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let config = self.config.clone();
        let transfer_metrics = Arc::clone(&self.transfer_metrics);
        let performance_metrics = Arc::clone(&self.performance_metrics);
        let error_metrics = Arc::clone(&self.error_metrics);

        let collection_task = tokio::spawn(async move {
            let mut interval = interval(config.collection_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = collect_system_metrics(
                            &config,
                            &transfer_metrics,
                            &performance_metrics,
                            &error_metrics,
                        ).await {
                            warn!("Failed to collect metrics: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Metrics collection shutdown requested");
                        break;
                    }
                }
            }
        });

        self.collection_task = Some(collection_task);
        info!("Metrics collection started");
        Ok(())
    }

    /// Stop metrics collection
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(task) = self.collection_task.take() {
            task.abort();
        }

        // Export final metrics
        self.export_metrics().await?;

        info!("Metrics collection stopped");
        Ok(())
    }

    /// Record transfer metrics
    #[instrument(skip(self))]
    pub fn record_transfer(
        &self,
        bytes_transferred: u64,
        transfer_time: Duration,
        success: bool,
    ) {
        let mut metrics = self.transfer_metrics.write().unwrap();

        metrics.total_transfers += 1;
        if success {
            metrics.successful_transfers += 1;
            metrics.total_bytes_sent += bytes_transferred;
            metrics.total_transfer_time += transfer_time;

            // Update speed metrics
            let speed = bytes_transferred as f64 / transfer_time.as_secs_f64();
            metrics.peak_transfer_speed = metrics.peak_transfer_speed.max(speed);

            // Update average speed
            let total_successful = metrics.successful_transfers;
            metrics.average_transfer_speed = 
                (metrics.average_transfer_speed * (total_successful - 1) as f64 + speed) 
                / total_successful as f64;
        } else {
            metrics.failed_transfers += 1;
        }

        debug!(
            "Recorded transfer: {} bytes, {:?}, success: {}",
            bytes_transferred, transfer_time, success
        );
    }

    /// Record error occurrence
    pub fn record_error(&self, error_type: &str, operation: &str) {
        let mut metrics = self.error_metrics.write().unwrap();

        *metrics.error_counts_by_type.entry(error_type.to_string()).or_insert(0) += 1;
        *metrics.error_counts_by_operation.entry(operation.to_string()).or_insert(0) += 1;

        debug!("Recorded error: {} in operation {}", error_type, operation);
    }

    /// Record operation latency
    pub fn record_latency(&self, operation: &str, latency: Duration) {
        let mut metrics = self.performance_metrics.write().unwrap();

        let latencies = metrics.operation_latencies
            .entry(operation.to_string())
            .or_insert_with(VecDeque::new);

        latencies.push_back(latency);

        // Keep only recent samples
        while latencies.len() > self.config.max_history_size {
            latencies.pop_front();
        }

        debug!("Recorded latency: {} -> {:?}", operation, latency);
    }

    /// Get current metrics snapshot
    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            transfer_metrics: self.transfer_metrics.read().unwrap().clone(),
            performance_metrics: self.performance_metrics.read().unwrap().clone(),
            error_metrics: self.error_metrics.read().unwrap().clone(),
            timestamp: Utc::now(),
        }
    }

    /// Export metrics to file
    async fn export_metrics(&self) -> Result<()> {
        if !self.config.enable_realtime_export {
            return Ok(());
        }

        let snapshot = self.get_metrics_snapshot();
        let json_data = serde_json::to_string_pretty(&snapshot)?;

        let mut file = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.config.metrics_file)
                .await?
        );

        file.write_all(json_data.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        debug!("Exported metrics to {}", self.config.metrics_file.display());
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub transfer_metrics: TransferMetrics,
    pub performance_metrics: PerformanceMetrics,
    pub error_metrics: ErrorMetrics,
    pub timestamp: DateTime<Utc>,
}

/// Collect system-level metrics
async fn collect_system_metrics(
    config: &MetricsConfig,
    transfer_metrics: &Arc<RwLock<TransferMetrics>>,
    performance_metrics: &Arc<RwLock<PerformanceMetrics>>,
    error_metrics: &Arc<RwLock<ErrorMetrics>>,
) -> Result<()> {
    if !config.monitor_system_resources {
        return Ok(());
    }

    let timestamp = Utc::now();

    // Collect CPU and memory usage (simplified implementation)
    let cpu_usage = get_cpu_usage().await.unwrap_or(0.0);
    let memory_usage = get_memory_usage().await.unwrap_or(0);

    {
        let mut perf_metrics = performance_metrics.write().unwrap();

        perf_metrics.cpu_usage_samples.push_back(cpu_usage);
        perf_metrics.memory_usage_samples.push_back(memory_usage);

        // Limit sample history
        while perf_metrics.cpu_usage_samples.len() > config.max_history_size {
            perf_metrics.cpu_usage_samples.pop_front();
        }

        while perf_metrics.memory_usage_samples.len() > config.max_history_size {
            perf_metrics.memory_usage_samples.pop_front();
        }
    }

    debug!("Collected system metrics: CPU {}%, Memory {} bytes", cpu_usage, memory_usage);
    Ok(())
}

/// Get current CPU usage percentage
async fn get_cpu_usage() -> Result<f64> {
    // Simplified implementation - in practice would use system APIs
    Ok(0.0)
}

/// Get current memory usage in bytes
async fn get_memory_usage() -> Result<u64> {
    // Simplified implementation - in practice would use system APIs
    Ok(0)
}

/// Format speed in human-readable format
fn format_speed(speed_bps: f64) -> String {
    if speed_bps < 1024.0 {
        format!("{:.1} B/s", speed_bps)
    } else if speed_bps < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", speed_bps / 1024.0)
    } else if speed_bps < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB/s", speed_bps / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB/s", speed_bps / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Enhanced user experience coordinator
pub struct UserExperienceManager {
    progress_manager: AdvancedProgressManager,
    metrics_collector: MetricsCollector,
    config: UxConfig,
}

#[derive(Debug, Clone)]
pub struct UxConfig {
    pub enable_progress_bars: bool,
    pub enable_metrics: bool,
    pub enable_notifications: bool,
    pub enable_sound_alerts: bool,
    pub progress_config: ProgressConfig,
    pub metrics_config: MetricsConfig,
}

impl UserExperienceManager {
    pub fn new(config: UxConfig) -> Self {
        let progress_manager = AdvancedProgressManager::new(config.progress_config.clone());
        let metrics_collector = MetricsCollector::new(config.metrics_config.clone());

        Self {
            progress_manager,
            metrics_collector,
            config,
        }
    }

    /// Start all UX systems
    pub async fn start(&mut self) -> Result<()> {
        if self.config.enable_metrics {
            self.metrics_collector.start().await?;
        }

        info!("User experience systems started");
        Ok(())
    }

    /// Stop all UX systems
    pub async fn stop(&mut self) -> Result<()> {
        if self.config.enable_metrics {
            self.metrics_collector.stop().await?;
        }

        info!("User experience systems stopped");
        Ok(())
    }

    /// Create progress tracking for an operation
    pub fn track_operation(
        &self,
        operation_id: &str,
        operation_type: &str,
        total_size: u64,
        message: &str,
    ) -> Result<Option<ProgressBarHandle>> {
        if self.config.enable_progress_bars {
            Ok(Some(self.progress_manager.create_progress_bar(
                operation_id, operation_type, total_size, message
            )?))
        } else {
            Ok(None)
        }
    }

    /// Record metrics for an operation
    pub fn record_operation_metrics(
        &self,
        bytes_transferred: u64,
        transfer_time: Duration,
        success: bool,
    ) {
        if self.config.enable_metrics {
            self.metrics_collector.record_transfer(bytes_transferred, transfer_time, success);
        }
    }

    /// Get comprehensive status report
    pub fn get_status_report(&self) -> StatusReport {
        let progress_summary = self.progress_manager.get_progress_summary();
        let metrics_snapshot = if self.config.enable_metrics {
            Some(self.metrics_collector.get_metrics_snapshot())
        } else {
            None
        };

        StatusReport {
            progress_summary,
            metrics_snapshot,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatusReport {
    pub progress_summary: ProgressSummary,
    pub metrics_snapshot: Option<MetricsSnapshot>,
    pub timestamp: DateTime<Utc>,
}

impl StatusReport {
    /// Format status report as human-readable text
    pub fn format_summary(&self) -> String {
        let mut report = String::new();

        report.push_str(&format!(
            "📊 Status Report ({}\n",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        report.push_str(&format!(
            "📋 Operations: {} active, {} completed, {} total\n",
            self.progress_summary.active_operations,
            self.progress_summary.completed_operations,
            self.progress_summary.total_operations
        ));

        if let Some(metrics) = &self.metrics_snapshot {
            let success_rate = if metrics.transfer_metrics.total_transfers > 0 {
                (metrics.transfer_metrics.successful_transfers as f64 / 
                 metrics.transfer_metrics.total_transfers as f64) * 100.0
            } else {
                0.0
            };

            report.push_str(&format!(
                "📈 Transfers: {} total, {:.1}% success rate\n",
                metrics.transfer_metrics.total_transfers,
                success_rate
            ));

            report.push_str(&format!(
                "⚡ Performance: {:.1} MB/s avg, {:.1} MB/s peak\n",
                metrics.transfer_metrics.average_transfer_speed / 1024.0 / 1024.0,
                metrics.transfer_metrics.peak_transfer_speed / 1024.0 / 1024.0
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_manager() {
        let config = ProgressConfig::default();
        let manager = AdvancedProgressManager::new(config);

        let handle = manager.create_progress_bar(
            "test_op",
            "file_transfer",
            1000,
            "Testing progress"
        ).unwrap();

        handle.update_progress(250, Some("25% complete"));
        assert_eq!(handle.get_percentage(), 25.0);

        handle.finish(Some("Completed successfully"));

        let summary = manager.get_progress_summary();
        assert_eq!(summary.total_operations, 1);
    }

    #[tokio::test]
    async fn test_metrics_collector() {
        let config = MetricsConfig {
            enabled: true,
            collection_interval: Duration::from_millis(100),
            metrics_file: std::path::PathBuf::from("test_metrics.json"),
            enable_realtime_export: false,
            max_history_size: 100,
            monitor_system_resources: false,
        };

        let collector = MetricsCollector::new(config);

        // Record some test metrics
        collector.record_transfer(1024, Duration::from_secs(1), true);
        collector.record_error("network_error", "file_transfer");
        collector.record_latency("conversion", Duration::from_millis(500));

        let snapshot = collector.get_metrics_snapshot();
        assert_eq!(snapshot.transfer_metrics.total_transfers, 1);
        assert_eq!(snapshot.transfer_metrics.successful_transfers, 1);
        assert!(snapshot.error_metrics.error_counts_by_type.contains_key("network_error"));
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(format_speed(512.0), "512.0 B/s");
        assert_eq!(format_speed(1536.0), "1.5 KB/s");
        assert_eq!(format_speed(2 * 1024.0 * 1024.0), "2.0 MB/s");
        assert_eq!(format_speed(3 * 1024.0 * 1024.0 * 1024.0), "3.0 GB/s");
    }
}
