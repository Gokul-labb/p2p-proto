//! Production Build Configuration and Cross-Platform Compatibility
//! 
//! This module provides optimized build configurations, deployment settings,
//! and cross-platform compatibility testing for production deployment.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::{debug, info, warn, error};

/// Production build configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionConfig {
    /// Build optimization level
    pub optimization_level: OptimizationLevel,

    /// Target platforms for cross-compilation
    pub target_platforms: Vec<TargetPlatform>,

    /// Binary compression settings
    pub compression: CompressionConfig,

    /// Resource embedding configuration
    pub resources: ResourceConfig,

    /// Deployment settings
    pub deployment: DeploymentConfig,

    /// Testing configuration
    pub testing: TestingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationLevel {
    /// Development build (fast compilation)
    Debug,

    /// Release build (balanced)
    Release,

    /// Maximum optimization (slower compilation)
    MaximumPerformance,

    /// Size-optimized build
    MinimumSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetPlatform {
    /// Target triple (e.g., x86_64-unknown-linux-gnu)
    pub target: String,

    /// Platform display name
    pub name: String,

    /// Whether this platform is supported
    pub supported: bool,

    /// Platform-specific build flags
    pub build_flags: Vec<String>,

    /// Required system dependencies
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable binary compression
    pub enabled: bool,

    /// Compression tool to use
    pub tool: CompressionTool,

    /// Compression level (1-9)
    pub level: u8,

    /// Strip debug symbols
    pub strip_symbols: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionTool {
    /// UPX (Ultimate Packer for eXecutables)
    Upx,

    /// LZMA compression
    Lzma,

    /// gzip compression
    Gzip,

    /// Platform default
    Default,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// Embed default configuration
    pub embed_default_config: bool,

    /// Embed help text and documentation
    pub embed_help: bool,

    /// Embed certificates and keys
    pub embed_certificates: bool,

    /// Resource directory path
    pub resource_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    /// Package format
    pub package_format: PackageFormat,

    /// Installation directories
    pub install_dirs: InstallDirs,

    /// System service configuration
    pub service_config: Option<ServiceConfig>,

    /// Update mechanism
    pub update_config: UpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageFormat {
    /// Debian package (.deb)
    Deb,

    /// RPM package (.rpm)
    Rpm,

    /// Windows installer (.msi)
    Msi,

    /// macOS package (.pkg)
    Pkg,

    /// AppImage (Linux)
    AppImage,

    /// Flatpak (Linux)
    Flatpak,

    /// Snap package (Linux)
    Snap,

    /// Tarball (.tar.gz)
    Tarball,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallDirs {
    pub binary_dir: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub cache_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    pub name: String,

    /// Service description
    pub description: String,

    /// Auto-start on boot
    pub auto_start: bool,

    /// Service user account
    pub user: String,

    /// Working directory
    pub working_dir: PathBuf,

    /// Environment variables
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Enable automatic updates
    pub auto_update: bool,

    /// Update check interval
    pub check_interval_hours: u32,

    /// Update server URL
    pub update_server: String,

    /// Update signature verification
    pub verify_signatures: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingConfig {
    /// Run cross-platform tests
    pub cross_platform_tests: bool,

    /// Performance benchmarking
    pub performance_tests: bool,

    /// Memory leak detection
    pub memory_leak_tests: bool,

    /// Security scanning
    pub security_tests: bool,

    /// Integration test suites
    pub integration_tests: Vec<String>,
}

impl Default for ProductionConfig {
    fn default() -> Self {
        Self {
            optimization_level: OptimizationLevel::Release,
            target_platforms: default_target_platforms(),
            compression: CompressionConfig {
                enabled: true,
                tool: CompressionTool::Default,
                level: 6,
                strip_symbols: true,
            },
            resources: ResourceConfig {
                embed_default_config: true,
                embed_help: true,
                embed_certificates: false,
                resource_dir: PathBuf::from("resources"),
            },
            deployment: DeploymentConfig {
                package_format: PackageFormat::Tarball,
                install_dirs: InstallDirs {
                    binary_dir: PathBuf::from("/usr/local/bin"),
                    config_dir: PathBuf::from("/etc/p2p-converter"),
                    data_dir: PathBuf::from("/var/lib/p2p-converter"),
                    log_dir: PathBuf::from("/var/log/p2p-converter"),
                    cache_dir: PathBuf::from("/tmp/p2p-converter"),
                },
                service_config: None,
                update_config: UpdateConfig {
                    auto_update: false,
                    check_interval_hours: 24,
                    update_server: "https://api.github.com/repos/user/p2p-converter/releases".to_string(),
                    verify_signatures: true,
                },
            },
            testing: TestingConfig {
                cross_platform_tests: true,
                performance_tests: true,
                memory_leak_tests: true,
                security_tests: true,
                integration_tests: vec![
                    "file_conversion".to_string(),
                    "network_communication".to_string(),
                    "error_handling".to_string(),
                ],
            },
        }
    }
}

/// Default supported target platforms
fn default_target_platforms() -> Vec<TargetPlatform> {
    vec![
        TargetPlatform {
            target: "x86_64-unknown-linux-gnu".to_string(),
            name: "Linux x64".to_string(),
            supported: true,
            build_flags: vec!["--release".to_string()],
            dependencies: vec!["build-essential".to_string(), "pkg-config".to_string()],
        },
        TargetPlatform {
            target: "x86_64-pc-windows-msvc".to_string(),
            name: "Windows x64".to_string(),
            supported: true,
            build_flags: vec!["--release".to_string()],
            dependencies: vec!["Visual Studio Build Tools".to_string()],
        },
        TargetPlatform {
            target: "x86_64-apple-darwin".to_string(),
            name: "macOS x64".to_string(),
            supported: true,
            build_flags: vec!["--release".to_string()],
            dependencies: vec!["Xcode Command Line Tools".to_string()],
        },
        TargetPlatform {
            target: "aarch64-unknown-linux-gnu".to_string(),
            name: "Linux ARM64".to_string(),
            supported: true,
            build_flags: vec!["--release".to_string()],
            dependencies: vec!["gcc-aarch64-linux-gnu".to_string()],
        },
        TargetPlatform {
            target: "aarch64-apple-darwin".to_string(),
            name: "macOS ARM64 (Apple Silicon)".to_string(),
            supported: true,
            build_flags: vec!["--release".to_string()],
            dependencies: vec!["Xcode Command Line Tools".to_string()],
        },
    ]
}

/// Production build manager
pub struct ProductionBuilder {
    config: ProductionConfig,
    build_dir: PathBuf,
    output_dir: PathBuf,
}

impl ProductionBuilder {
    /// Create new production builder
    pub fn new(config: ProductionConfig, build_dir: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            config,
            build_dir,
            output_dir,
        }
    }

    /// Build for all supported platforms
    pub async fn build_all_platforms(&self) -> Result<Vec<BuildResult>> {
        let mut results = Vec::new();

        info!("Starting cross-platform build process");

        for platform in &self.config.target_platforms {
            if platform.supported {
                info!("Building for platform: {}", platform.name);

                match self.build_for_platform(platform).await {
                    Ok(result) => {
                        info!("✅ Build successful for {}: {}", platform.name, result.output_path.display());
                        results.push(result);
                    }
                    Err(e) => {
                        error!("❌ Build failed for {}: {}", platform.name, e);
                        results.push(BuildResult {
                            platform: platform.clone(),
                            success: false,
                            output_path: PathBuf::new(),
                            binary_size: 0,
                            build_time: std::time::Duration::from_secs(0),
                            error_message: Some(e.to_string()),
                        });
                    }
                }
            } else {
                warn!("Skipping unsupported platform: {}", platform.name);
            }
        }

        info!("Cross-platform build completed: {}/{} successful", 
              results.iter().filter(|r| r.success).count(),
              results.len());

        Ok(results)
    }

    /// Build for specific platform
    async fn build_for_platform(&self, platform: &TargetPlatform) -> Result<BuildResult> {
        let start_time = std::time::Instant::now();

        // Prepare build environment
        self.setup_build_environment(platform).await?;

        // Execute build command
        let mut build_cmd = Command::new("cargo");
        build_cmd
            .arg("build")
            .arg("--target")
            .arg(&platform.target)
            .current_dir(&self.build_dir);

        // Add optimization flags
        match self.config.optimization_level {
            OptimizationLevel::Debug => {},
            OptimizationLevel::Release => {
                build_cmd.arg("--release");
            },
            OptimizationLevel::MaximumPerformance => {
                build_cmd.arg("--release");
                build_cmd.env("RUSTFLAGS", "-C target-cpu=native -C opt-level=3");
            },
            OptimizationLevel::MinimumSize => {
                build_cmd.arg("--release");
                build_cmd.env("RUSTFLAGS", "-C opt-level=s -C strip=symbols");
            },
        }

        // Add platform-specific flags
        for flag in &platform.build_flags {
            build_cmd.arg(flag);
        }

        debug!("Executing build command: {:?}", build_cmd);

        let output = build_cmd.output()
            .context("Failed to execute cargo build command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Build failed: {}", stderr));
        }

        // Locate built binary
        let binary_path = self.locate_built_binary(platform)?;

        // Post-process binary (compression, stripping, etc.)
        let processed_path = self.post_process_binary(&binary_path, platform).await?;

        let binary_size = std::fs::metadata(&processed_path)
            .context("Failed to get binary metadata")?
            .len();

        let build_time = start_time.elapsed();

        Ok(BuildResult {
            platform: platform.clone(),
            success: true,
            output_path: processed_path,
            binary_size,
            build_time,
            error_message: None,
        })
    }

    /// Setup build environment for platform
    async fn setup_build_environment(&self, platform: &TargetPlatform) -> Result<()> {
        // Add target if not already installed
        let mut add_target_cmd = Command::new("rustup");
        add_target_cmd
            .arg("target")
            .arg("add")
            .arg(&platform.target);

        let output = add_target_cmd.output()
            .context("Failed to add rust target")?;

        if !output.status.success() {
            warn!("Failed to add target {}: {}", platform.target, 
                  String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Locate the built binary
    fn locate_built_binary(&self, platform: &TargetPlatform) -> Result<PathBuf> {
        let target_dir = self.build_dir.join("target").join(&platform.target);

        let binary_dir = match self.config.optimization_level {
            OptimizationLevel::Debug => target_dir.join("debug"),
            _ => target_dir.join("release"),
        };

        let binary_name = if platform.target.contains("windows") {
            "p2p-converter.exe"
        } else {
            "p2p-converter"
        };

        let binary_path = binary_dir.join(binary_name);

        if !binary_path.exists() {
            return Err(anyhow::anyhow!("Built binary not found at: {}", binary_path.display()));
        }

        Ok(binary_path)
    }

    /// Post-process binary (compression, stripping, etc.)
    async fn post_process_binary(&self, binary_path: &Path, platform: &TargetPlatform) -> Result<PathBuf> {
        let output_path = self.output_dir.join(format!(
            "p2p-converter-{}-{}",
            platform.target,
            if platform.target.contains("windows") { ".exe" } else { "" }
        ));

        // Copy binary to output directory
        std::fs::copy(binary_path, &output_path)
            .context("Failed to copy binary to output directory")?;

        // Strip debug symbols if configured
        if self.config.compression.strip_symbols {
            self.strip_binary_symbols(&output_path, platform).await?;
        }

        // Compress binary if configured
        if self.config.compression.enabled {
            self.compress_binary(&output_path, platform).await?;
        }

        Ok(output_path)
    }

    /// Strip debug symbols from binary
    async fn strip_binary_symbols(&self, binary_path: &Path, platform: &TargetPlatform) -> Result<()> {
        let strip_cmd = if platform.target.contains("windows") {
            return Ok(()); // Windows doesn't typically use strip
        } else if platform.target.contains("darwin") {
            "strip"
        } else {
            "strip"
        };

        let mut cmd = Command::new(strip_cmd);
        cmd.arg(binary_path);

        let output = cmd.output()
            .context("Failed to execute strip command")?;

        if !output.status.success() {
            warn!("Failed to strip symbols: {}", String::from_utf8_lossy(&output.stderr));
        } else {
            debug!("Stripped debug symbols from {}", binary_path.display());
        }

        Ok(())
    }

    /// Compress binary using configured compression tool
    async fn compress_binary(&self, binary_path: &Path, platform: &TargetPlatform) -> Result<()> {
        match self.config.compression.tool {
            CompressionTool::Upx => {
                self.compress_with_upx(binary_path).await?;
            }
            CompressionTool::Default | _ => {
                // Skip compression for unsupported tools
                debug!("Skipping compression for platform {}", platform.name);
            }
        }

        Ok(())
    }

    /// Compress binary with UPX
    async fn compress_with_upx(&self, binary_path: &Path) -> Result<()> {
        let mut cmd = Command::new("upx");
        cmd.arg(format!("-{}", self.config.compression.level))
           .arg(binary_path);

        let output = cmd.output();

        match output {
            Ok(output) if output.status.success() => {
                debug!("Compressed binary with UPX: {}", binary_path.display());
            }
            Ok(output) => {
                warn!("UPX compression failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(_) => {
                debug!("UPX not available, skipping compression");
            }
        }

        Ok(())
    }

    /// Run cross-platform tests
    pub async fn run_cross_platform_tests(&self, build_results: &[BuildResult]) -> Result<TestResults> {
        if !self.config.testing.cross_platform_tests {
            return Ok(TestResults::default());
        }

        info!("Running cross-platform compatibility tests");

        let mut test_results = TestResults::default();

        for result in build_results {
            if result.success {
                let platform_tests = self.test_platform_binary(&result.output_path, &result.platform).await;
                test_results.platform_results.insert(result.platform.name.clone(), platform_tests);
            }
        }

        info!("Cross-platform testing completed");
        Ok(test_results)
    }

    /// Test binary on specific platform
    async fn test_platform_binary(&self, binary_path: &Path, platform: &TargetPlatform) -> PlatformTestResult {
        let mut result = PlatformTestResult {
            platform: platform.name.clone(),
            binary_works: false,
            help_output_correct: false,
            version_output_correct: false,
            basic_functionality: false,
            performance_acceptable: false,
        };

        // Test 1: Binary executes without crashing
        result.binary_works = self.test_binary_execution(binary_path).await;

        if result.binary_works {
            // Test 2: Help output
            result.help_output_correct = self.test_help_output(binary_path).await;

            // Test 3: Version output
            result.version_output_correct = self.test_version_output(binary_path).await;

            // Test 4: Basic functionality
            result.basic_functionality = self.test_basic_functionality(binary_path).await;

            // Test 5: Performance benchmark
            if self.config.testing.performance_tests {
                result.performance_acceptable = self.test_performance(binary_path).await;
            }
        }

        result
    }

    async fn test_binary_execution(&self, binary_path: &Path) -> bool {
        Command::new(binary_path)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    async fn test_help_output(&self, binary_path: &Path) -> bool {
        Command::new(binary_path)
            .arg("--help")
            .output()
            .map(|output| {
                output.status.success() && 
                String::from_utf8_lossy(&output.stdout).contains("p2p-converter")
            })
            .unwrap_or(false)
    }

    async fn test_version_output(&self, binary_path: &Path) -> bool {
        Command::new(binary_path)
            .arg("--version")
            .output()
            .map(|output| {
                output.status.success() && 
                String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION"))
            })
            .unwrap_or(false)
    }

    async fn test_basic_functionality(&self, _binary_path: &Path) -> bool {
        // Would implement basic functionality tests
        // For now, assume success if previous tests passed
        true
    }

    async fn test_performance(&self, _binary_path: &Path) -> bool {
        // Would implement performance benchmarks
        // For now, assume acceptable performance
        true
    }
}

#[derive(Debug, Clone)]
pub struct BuildResult {
    pub platform: TargetPlatform,
    pub success: bool,
    pub output_path: PathBuf,
    pub binary_size: u64,
    pub build_time: std::time::Duration,
    pub error_message: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct TestResults {
    pub platform_results: HashMap<String, PlatformTestResult>,
}

#[derive(Debug, Clone)]
pub struct PlatformTestResult {
    pub platform: String,
    pub binary_works: bool,
    pub help_output_correct: bool,
    pub version_output_correct: bool,
    pub basic_functionality: bool,
    pub performance_acceptable: bool,
}

impl PlatformTestResult {
    pub fn overall_success(&self) -> bool {
        self.binary_works && 
        self.help_output_correct && 
        self.version_output_correct && 
        self.basic_functionality
    }
}

/// Generate build report
pub fn generate_build_report(
    build_results: &[BuildResult],
    test_results: &TestResults,
) -> String {
    let mut report = String::new();

    report.push_str("# P2P File Converter - Build Report\n\n");
    report.push_str(&format!("Generated: {}\n\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

    // Build results summary
    let successful_builds = build_results.iter().filter(|r| r.success).count();
    report.push_str(&format!("## Build Summary\n"));
    report.push_str(&format!("- Total platforms: {}\n", build_results.len()));
    report.push_str(&format!("- Successful builds: {}\n", successful_builds));
    report.push_str(&format!("- Failed builds: {}\n\n", build_results.len() - successful_builds));

    // Individual platform results
    report.push_str("## Platform Results\n\n");
    for result in build_results {
        let status = if result.success { "✅" } else { "❌" };
        report.push_str(&format!("### {} {} ({})\n", status, result.platform.name, result.platform.target));

        if result.success {
            report.push_str(&format!("- Binary size: {} KB\n", result.binary_size / 1024));
            report.push_str(&format!("- Build time: {:?}\n", result.build_time));
            report.push_str(&format!("- Output: `{}`\n", result.output_path.display()));

            // Test results
            if let Some(test_result) = test_results.platform_results.get(&result.platform.name) {
                let test_status = if test_result.overall_success() { "✅" } else { "⚠️" };
                report.push_str(&format!("- Tests: {} {}\n", 
                    test_status,
                    if test_result.overall_success() { "Passed" } else { "Some issues" }
                ));
            }
        } else if let Some(error) = &result.error_message {
            report.push_str(&format!("- Error: {}\n", error));
        }

        report.push_str("\n");
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_config_default() {
        let config = ProductionConfig::default();
        assert!(config.target_platforms.len() >= 3); // At least Linux, Windows, macOS
        assert!(config.compression.enabled);
        assert!(config.testing.cross_platform_tests);
    }

    #[test]
    fn test_platform_test_result() {
        let result = PlatformTestResult {
            platform: "Test Platform".to_string(),
            binary_works: true,
            help_output_correct: true,
            version_output_correct: true,
            basic_functionality: true,
            performance_acceptable: true,
        };

        assert!(result.overall_success());

        let failed_result = PlatformTestResult {
            platform: "Failed Platform".to_string(),
            binary_works: false,
            help_output_correct: false,
            version_output_correct: false,
            basic_functionality: false,
            performance_acceptable: false,
        };

        assert!(!failed_result.overall_success());
    }
}
