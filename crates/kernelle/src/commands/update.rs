use anyhow::{Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    tarball_url: String,
}

pub async fn execute(version: Option<&str>) -> Result<()> {
    println!("🚀 Starting kernelle update...");
    
    // Determine target version
    let target_version = match version {
        Some(v) => {
            println!("📌 Updating to version: {}", v);
            v.to_string()
        }
        None => {
            println!("🔍 Fetching latest version...");
            get_latest_version().await?
        }
    };
    
    // Create staging areas
    let staging_dir = TempDir::new().context("Failed to create staging directory")?;
    let kernelle_staging = staging_dir.path().join("kernelle_home");
    let bins_staging = staging_dir.path().join("bins");
    
    fs::create_dir_all(&kernelle_staging)?;
    fs::create_dir_all(&bins_staging)?;
    
    println!("📁 Staging in: {}", staging_dir.path().display());
    
    // Download and extract
    println!("⬇️  Downloading kernelle {}...", target_version);
    let extracted_dir = download_and_extract(&target_version, staging_dir.path()).await?;
    
    // Test build in staging environment
    println!("🔨 Building and testing in staging environment...");
    test_build_in_staging(&extracted_dir, &kernelle_staging, &bins_staging).await?;
    
    // Create snapshot of current installation
    println!("📸 Creating snapshot of current installation...");
    let snapshot_dir = create_snapshot().await?;
    
    // Attempt to install new version - if this fails, automatically rollback
    println!("⚡ Installing new version...");
    match install_new_version(&extracted_dir).await {
        Ok(()) => {
            // Verify installation
            println!("✅ Verifying installation...");
            match verify_installation().await {
                Ok(()) => {
                    // Success! Clean up staging
                    drop(staging_dir);
                    
                    println!("🎉 Update completed successfully!");
                    println!("📝 Snapshot saved at: {}", snapshot_dir.display());
                    println!("💡 Snapshot will be automatically cleaned up in 24 hours");
                    
                    Ok(())
                }
                Err(e) => {
                    println!("❌ Verification failed: {}", e);
                    println!("🔄 Automatically rolling back to previous version...");
                    
                    match perform_rollback(&snapshot_dir).await {
                        Ok(()) => {
                            println!("✅ Rollback completed successfully");
                            Err(anyhow::anyhow!("Update failed and was rolled back: {}", e))
                        }
                        Err(rollback_err) => {
                            println!("💥 CRITICAL: Rollback also failed: {}", rollback_err);
                            Err(anyhow::anyhow!(
                                "Update failed: {}. Rollback also failed: {}. Manual recovery may be needed.",
                                e,
                                rollback_err
                            ))
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Installation failed: {}", e);
            println!("🔄 Automatically rolling back to previous version...");
            
            match perform_rollback(&snapshot_dir).await {
                Ok(()) => {
                    println!("✅ Rollback completed successfully");
                    Err(anyhow::anyhow!("Update failed and was rolled back: {}", e))
                }
                Err(rollback_err) => {
                    println!("💥 CRITICAL: Rollback also failed: {}", rollback_err);
                    Err(anyhow::anyhow!(
                        "Update failed: {}. Rollback also failed: {}. Manual recovery may be needed.",
                        e,
                        rollback_err
                    ))
                }
            }
        }
    }
}

async fn get_latest_version() -> Result<String> {
    let client = reqwest::Client::new();
    let url = "https://api.github.com/repos/TravelSizedLions/kernelle/releases/latest";
    
    let response = client
        .get(url)
        .header("User-Agent", "kernelle-updater")
        .send()
        .await
        .context("Failed to fetch latest release from GitHub")?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }
    
    let release: GitHubRelease = response
        .json()
        .await
        .context("Failed to parse GitHub release response")?;
    
    println!("📌 Latest version: {}", release.tag_name);
    Ok(release.tag_name)
}

async fn download_and_extract(version: &str, staging_path: &Path) -> Result<std::path::PathBuf> {
    let client = reqwest::Client::new();
    
    // Get release info
    let release_url = if version == "latest" {
        "https://api.github.com/repos/TravelSizedLions/kernelle/releases/latest".to_string()
    } else {
        format!("https://api.github.com/repos/TravelSizedLions/kernelle/releases/tags/{}", version)
    };
    
    let response = client
        .get(&release_url)
        .header("User-Agent", "kernelle-updater")
        .send()
        .await
        .context("Failed to fetch release info from GitHub")?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "GitHub API request failed with status: {}. Version '{}' may not exist.",
            response.status(),
            version
        ));
    }
    
    let release: GitHubRelease = response
        .json()
        .await
        .context("Failed to parse GitHub release response")?;
    
    // Download tarball
    println!("⬇️  Downloading from: {}", release.tarball_url);
    let tarball_response = client
        .get(&release.tarball_url)
        .header("User-Agent", "kernelle-updater")
        .send()
        .await
        .context("Failed to download release tarball")?;
    
    if !tarball_response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download tarball: HTTP {}",
            tarball_response.status()
        ));
    }
    
    // Save tarball to staging area
    let tarball_path = staging_path.join("kernelle.tar.gz");
    let tarball_bytes = tarball_response
        .bytes()
        .await
        .context("Failed to read tarball content")?;
    
    fs::write(&tarball_path, &tarball_bytes)
        .context("Failed to write tarball to disk")?;
    
    // Extract tarball
    println!("📦 Extracting tarball...");
    let output = Command::new("tar")
        .args(&["-xzf", tarball_path.to_str().unwrap()])
        .current_dir(staging_path)
        .output()
        .context("Failed to execute tar command")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to extract tarball: {}", stderr));
    }
    
    // Find the extracted directory (GitHub creates a directory like TravelSizedLions-kernelle-abc123)
    let entries = fs::read_dir(staging_path)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.file_name().unwrap().to_str().unwrap().contains("kernelle") {
            if path != tarball_path.parent().unwrap() {
                return Ok(path);
            }
        }
    }
    
    Err(anyhow::anyhow!("Could not find extracted kernelle directory"))
}

async fn test_build_in_staging(
    source_dir: &Path,
    kernelle_home: &Path,
    install_dir: &Path,
) -> Result<()> {
    let install_script = source_dir.join("scripts").join("install.sh");
    
    if !install_script.exists() {
        return Err(anyhow::anyhow!("install.sh not found in extracted archive"));
    }
    
    // Run install.sh with staging environment variables
    let output = Command::new("bash")
        .arg(&install_script)
        .arg("--non-interactive")
        .env("KERNELLE_HOME", kernelle_home)
        .env("INSTALL_DIR", install_dir)
        .output()
        .context("Failed to run install.sh in staging")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Staging installation failed: {}", stderr));
    }
    
    // Quick smoke test of staged binaries
    let kernelle_bin = install_dir.join("kernelle");
    if !kernelle_bin.exists() {
        return Err(anyhow::anyhow!("kernelle binary not found in staging installation"));
    }
    
    let version_output = Command::new(&kernelle_bin)
        .arg("--version")
        .env("KERNELLE_HOME", kernelle_home)
        .output()
        .context("Failed to test staged kernelle binary")?;
    
    if !version_output.status.success() {
        return Err(anyhow::anyhow!("Staged kernelle binary failed version check"));
    }
    
    println!("✅ Staging installation successful");
    Ok(())
}

async fn create_snapshot() -> Result<std::path::PathBuf> {
    let kernelle_home = env::var("KERNELLE_HOME")
        .unwrap_or_else(|_| format!("{}/.kernelle", env::var("HOME").unwrap_or_default()));
    let install_dir = env::var("INSTALL_DIR")
        .unwrap_or_else(|_| format!("{}/.cargo/bin", env::var("HOME").unwrap_or_default()));
    
    let snapshot_base = Path::new(&kernelle_home).join("snapshots");
    fs::create_dir_all(&snapshot_base).context("Failed to create snapshots directory")?;
    
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let snapshot_dir = snapshot_base.join(format!("pre_update_{}", timestamp));
    fs::create_dir_all(&snapshot_dir)?;
    
    // Snapshot kernelle home directory
    let kernelle_snapshot = snapshot_dir.join("kernelle_home");
    copy_dir_recursive(&kernelle_home, &kernelle_snapshot)?;
    
    // Snapshot binaries
    let bins_snapshot = snapshot_dir.join("bins");
    fs::create_dir_all(&bins_snapshot)?;
    
    let binaries = ["kernelle", "jerrod", "blizz", "violet", "adam", "sentinel"];
    for binary in &binaries {
        let src = Path::new(&install_dir).join(binary);
        if src.exists() {
            let dst = bins_snapshot.join(binary);
            fs::copy(&src, &dst).context(format!("Failed to backup {}", binary))?;
        }
    }
    
    Ok(snapshot_dir)
}

async fn install_new_version(source_dir: &Path) -> Result<()> {
    let install_script = source_dir.join("scripts").join("install.sh");
    
    let output = Command::new("bash")
        .arg(&install_script)
        .arg("--non-interactive")
        .output()
        .context("Failed to run install.sh for new version")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Installation failed: {}", stderr));
    }
    
    Ok(())
}

async fn verify_installation() -> Result<()> {
    // Test that kernelle works
    let output = Command::new("kernelle")
        .arg("--version")
        .output()
        .context("Failed to test kernelle after installation")?;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("kernelle failed version check after installation"));
    }
    
    println!("✅ Installation verified");
    Ok(())
}

fn copy_dir_recursive<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    
    if !src.exists() {
        return Ok(()); // Nothing to copy
    }
    
    fs::create_dir_all(dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}

async fn perform_rollback(snapshot_path: &Path) -> Result<()> {
    println!("🔄 Rolling back from snapshot: {}", snapshot_path.display());
    
    let kernelle_home = env::var("KERNELLE_HOME")
        .unwrap_or_else(|_| format!("{}/.kernelle", env::var("HOME").unwrap_or_default()));
    let install_dir = env::var("INSTALL_DIR")
        .unwrap_or_else(|_| format!("{}/.cargo/bin", env::var("HOME").unwrap_or_default()));
    
    if !snapshot_path.exists() {
        return Err(anyhow::anyhow!("Snapshot directory not found: {}", snapshot_path.display()));
    }
    
    // Restore kernelle home (excluding the snapshots directory itself)
    let kernelle_backup = snapshot_path.join("kernelle_home");
    if kernelle_backup.exists() {
        // Create a temporary backup of current snapshots
        let temp_snapshots = tempfile::tempdir()?;
        let snapshots_dir = Path::new(&kernelle_home).join("snapshots");
        if snapshots_dir.exists() {
            copy_dir_recursive(&snapshots_dir, temp_snapshots.path().join("snapshots"))?;
        }
        
        // Clear current kernelle home
        if Path::new(&kernelle_home).exists() {
            fs::remove_dir_all(&kernelle_home)?;
        }
        
        // Restore from backup
        copy_dir_recursive(&kernelle_backup, &kernelle_home)?;
        
        // Restore the snapshots directory
        if Path::new(&kernelle_home).join("snapshots").exists() {
            fs::remove_dir_all(Path::new(&kernelle_home).join("snapshots"))?;
        }
        copy_dir_recursive(temp_snapshots.path().join("snapshots"), &snapshots_dir)?;
        
        println!("✅ Restored kernelle home directory");
    }
    
    // Restore binaries
    let bins_backup = snapshot_path.join("bins");
    if bins_backup.exists() {
        let binaries = ["kernelle", "jerrod", "blizz", "violet", "adam", "sentinel"];
        for binary in &binaries {
            let backup_bin = bins_backup.join(binary);
            let install_bin = Path::new(&install_dir).join(binary);
            
            if backup_bin.exists() {
                if install_bin.exists() {
                    fs::remove_file(&install_bin)?;
                }
                fs::copy(&backup_bin, &install_bin)
                    .context(format!("Failed to restore {}", binary))?;
                println!("✅ Restored {}", binary);
            }
        }
    }
    
    // Verify rollback
    println!("🔍 Verifying rollback...");
    verify_installation().await?;
    
    println!("✅ Rollback completed successfully!");
    
    Ok(())
}
